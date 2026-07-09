//! Server-side reads from the SQLite cache produced by the Python pipeline.
//! Compiled only in the `ssr` build (rusqlite is not available in wasm).

use std::collections::{BTreeSet, HashMap};

use rusqlite::{Connection, OptionalExtension};

use crate::model::{
    CompanyDetail, CompanySummary, CompareColumn, CompareTable, ConceptRow, FilingRow,
};

/// Headline IFRS rows in display order: a label plus the accepted concept tags
/// (primary first). Issuers tag the same line differently, so each row coalesces
/// across its aliases, preferring the primary tag.
const CONCEPTS: [(&str, &[&str]); 5] = [
    (
        "Revenue",
        &[
            "ifrs-full:Revenue",
            "ifrs-full:RevenueFromContractsWithCustomers",
            "ifrs-full:RevenueFromSaleOfGoods",
        ],
    ),
    ("Profit (loss) for the period", &["ifrs-full:ProfitLoss"]),
    ("Total assets", &["ifrs-full:Assets"]),
    (
        "Total equity",
        &[
            "ifrs-full:Equity",
            "ifrs-full:EquityAttributableToOwnersOfParent",
        ],
    ),
    (
        "Cash flow from operating activities",
        &[
            "ifrs-full:CashFlowsFromUsedInOperatingActivities",
            "ifrs-full:CashFlowsFromUsedInOperatingActivitiesContinuingOperations",
        ],
    ),
];

/// SQL expression mapping a `reporting_date` to its fiscal year — the year of
/// the date six months before period end. This keeps 52/53-week retail filers
/// (whose year-end drifts across Dec/Jan) and January year-ends aligned, and
/// avoids two of one issuer's year-ends landing in the same column.
const FISCAL_YEAR: &str = "strftime('%Y', reporting_date, '-182 days')";

fn db_path() -> String {
    std::env::var("MERIDIAN_DB").unwrap_or_else(|_| "data/meridian.db".to_string())
}

pub fn open_db() -> rusqlite::Result<Connection> {
    Connection::open(db_path())
}

/// Companies for the search page, optionally filtered by a case-insensitive
/// substring of name, country, or LEI.
pub fn list_companies(query: Option<&str>) -> rusqlite::Result<Vec<CompanySummary>> {
    let conn = open_db()?;
    let like = query.map(|q| format!("%{}%", q.to_lowercase()));
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.country, e.lei,
                COUNT(f.id) AS filing_count,
                MIN(strftime('%Y', f.reporting_date, '-182 days')) AS first_year,
                MAX(strftime('%Y', f.reporting_date, '-182 days')) AS last_year
         FROM entities e
         LEFT JOIN filings f ON f.entity_id = e.id
         WHERE (?1 IS NULL
                OR lower(e.name) LIKE ?1
                OR lower(IFNULL(e.country, '')) LIKE ?1
                OR lower(IFNULL(e.lei, '')) LIKE ?1)
         GROUP BY e.id
         ORDER BY e.name",
    )?;
    let rows = stmt.query_map([like.as_deref()], |r| {
        Ok(CompanySummary {
            id: r.get(0)?,
            name: r.get(1)?,
            country: r.get(2)?,
            lei: r.get(3)?,
            filing_count: r.get(4)?,
            first_year: r.get(5)?,
            last_year: r.get(6)?,
        })
    })?;
    rows.collect()
}

/// A company's raw (unformatted) figures, shared by the detail and compare
/// paths. Each row is aligned to [`CONCEPTS`], each cell to `years`, and holds
/// the amount in minor-free full units plus its ISO currency.
struct RawCompany {
    id: i64,
    name: String,
    country: Option<String>,
    lei: Option<String>,
    currency: Option<String>,
    years: Vec<String>,
    rows: Vec<Vec<Option<(i128, String)>>>,
}

fn raw_company(conn: &Connection, id: i64) -> rusqlite::Result<Option<RawCompany>> {
    let entity = conn
        .query_row(
            "SELECT id, name, country, lei FROM entities WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((id, name, country, lei)) = entity else {
        return Ok(None);
    };

    let mut ystmt = conn.prepare(&format!(
        "SELECT DISTINCT {FISCAL_YEAR} AS fy FROM filings
         WHERE entity_id = ?1 AND reporting_date IS NOT NULL
         ORDER BY fy DESC"
    ))?;
    let years: Vec<String> = ystmt
        .query_map([id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;

    // (concept, fiscal_year) -> (amount, currency)
    let mut xstmt = conn.prepare(&format!(
        "SELECT concept, {FISCAL_YEAR} AS fy, value, currency
         FROM financial_facts WHERE entity_id = ?1"
    ))?;
    let mut facts: HashMap<(String, String), (i128, String)> = HashMap::new();
    let mut currency: Option<String> = None;
    let query = xstmt.query_map([id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    for row in query {
        let (concept, fy, value, ccy) = row?;
        if currency.is_none() && ccy.is_some() {
            currency = ccy.clone();
        }
        if let (Some(fy), Some(amount)) = (fy, value.as_deref().and_then(parse_amount)) {
            facts.insert((concept, fy), (amount, ccy.unwrap_or_default()));
        }
    }

    let rows = CONCEPTS
        .iter()
        .map(|(_, concepts)| {
            years
                .iter()
                .map(|y| {
                    concepts
                        .iter()
                        .find_map(|c| facts.get(&((*c).to_string(), y.clone())).cloned())
                })
                .collect()
        })
        .collect();

    Ok(Some(RawCompany {
        id,
        name,
        country,
        lei,
        currency,
        years,
        rows,
    }))
}

/// Full detail for one company: entity info, a pivoted IFRS financials table,
/// and the filing timeline. Returns `None` if the id is unknown.
pub fn get_company(id: i64) -> rusqlite::Result<Option<CompanyDetail>> {
    let conn = open_db()?;
    let Some(raw) = raw_company(&conn, id)? else {
        return Ok(None);
    };

    // Filing timeline (most recent first).
    let mut fstmt = conn.prepare(
        "SELECT reporting_date, country, filing_url, xbrl_json_url, validation_message_count
         FROM filings WHERE entity_id = ?1 ORDER BY reporting_date DESC",
    )?;
    let filings: Vec<FilingRow> = fstmt
        .query_map([raw.id], |r| {
            Ok(FilingRow {
                reporting_date: r.get(0)?,
                country: r.get(1)?,
                filing_url: r.get(2)?,
                xbrl_json_url: r.get(3)?,
                validation_message_count: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let rows = CONCEPTS
        .iter()
        .zip(raw.rows.iter())
        .map(|((label, concepts), raw_row)| ConceptRow {
            concept: concepts[0].to_string(),
            label: label.to_string(),
            cells: raw_row
                .iter()
                .map(|cell| cell.as_ref().map(|(n, _)| fmt_millions(*n)))
                .collect(),
        })
        .collect();

    Ok(Some(CompanyDetail {
        id: raw.id,
        name: raw.name,
        country: raw.country,
        lei: raw.lei,
        currency: raw.currency,
        years: raw.years,
        rows,
        filings,
    }))
}

/// Align several companies' IFRS figures for one fiscal year, one column each.
/// Unknown ids are skipped. `fy` defaults to the most recent year available.
/// `base` (an ISO code such as `"EUR"`) converts every column to that currency
/// at ECB annual-average rates; `None`/`"native"` keeps each issuer's own.
pub fn compare(
    ids: &[i64],
    fy: Option<&str>,
    base: Option<&str>,
) -> rusqlite::Result<CompareTable> {
    let conn = open_db()?;
    let fx = load_fx(&conn)?;

    let raws: Vec<RawCompany> = ids
        .iter()
        .filter_map(|&id| raw_company(&conn, id).transpose())
        .collect::<rusqlite::Result<_>>()?;

    let mut year_set: BTreeSet<String> = BTreeSet::new();
    for r in &raws {
        year_set.extend(r.years.iter().cloned());
    }
    let years: Vec<String> = year_set.into_iter().rev().collect();

    let fy = fy
        .map(str::to_string)
        .filter(|s| years.contains(s))
        .or_else(|| years.first().cloned())
        .unwrap_or_default();

    let base = base
        .filter(|b| !b.is_empty() && !b.eq_ignore_ascii_case("native"))
        .map(str::to_uppercase);

    let labels: Vec<String> = CONCEPTS.iter().map(|(l, _)| l.to_string()).collect();

    let columns = raws
        .into_iter()
        .map(|r| {
            let idx = r.years.iter().position(|y| *y == fy);
            let cells = r
                .rows
                .iter()
                .map(|row| {
                    let cell = idx.and_then(|i| row.get(i).cloned().flatten());
                    cell.and_then(|(amount, cur)| match &base {
                        Some(b) => convert(amount, &cur, b, &fy, &fx).map(fmt_millions),
                        None => Some(fmt_millions(amount)),
                    })
                })
                .collect();
            CompareColumn {
                currency: base.clone().or_else(|| r.currency.clone()),
                id: r.id,
                name: r.name,
                country: r.country,
                cells,
            }
        })
        .collect();

    Ok(CompareTable {
        fy,
        years,
        labels,
        columns,
        base,
    })
}

/// Load all FX rates as `(currency, year) -> units per EUR`.
fn load_fx(conn: &Connection) -> rusqlite::Result<HashMap<(String, String), f64>> {
    let mut stmt = conn.prepare("SELECT currency, year, rate_per_eur FROM fx_rates")?;
    let mut map = HashMap::new();
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, f64>(2)?,
        ))
    })?;
    for row in rows {
        let (currency, year, rate) = row?;
        map.insert((currency, year), rate);
    }
    Ok(map)
}

fn rate_per_eur(fx: &HashMap<(String, String), f64>, currency: &str, year: &str) -> Option<f64> {
    if currency.eq_ignore_ascii_case("EUR") {
        Some(1.0)
    } else {
        fx.get(&(currency.to_string(), year.to_string())).copied()
    }
}

/// Convert `amount` in `from` to `to` for a given fiscal year via EUR.
/// Returns `None` if a needed rate is missing.
fn convert(
    amount: i128,
    from: &str,
    to: &str,
    year: &str,
    fx: &HashMap<(String, String), f64>,
) -> Option<i128> {
    if from.eq_ignore_ascii_case(to) {
        return Some(amount);
    }
    let from_rate = rate_per_eur(fx, from, year)?;
    let to_rate = rate_per_eur(fx, to, year)?;
    if from_rate == 0.0 {
        return None;
    }
    Some((amount as f64 * to_rate / from_rate).round() as i128)
}

/// Format a full-currency-unit amount as a thousands-grouped figure in millions,
/// e.g. `77_769_000_000` -> `"77,769"`.
fn fmt_millions(n: i128) -> String {
    let millions = n / 1_000_000;
    let grouped = group_thousands(millions.abs());
    if millions < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// Parse a raw XBRL-JSON numeric string (which may carry a decimal part or sign)
/// to an integer amount, truncating any fractional part.
fn parse_amount(raw: &str) -> Option<i128> {
    let raw = raw.trim();
    let negative = raw.starts_with('-');
    let body = raw.trim_start_matches(['-', '+']);
    let int_part = body.split('.').next().unwrap_or("");
    let digits: String = int_part.chars().filter(|c| c.is_ascii_digit()).collect();
    let n: i128 = digits.parse().ok()?;
    Some(if negative { -n } else { n })
}

fn group_thousands(n: i128) -> String {
    let s = n.abs().to_string();
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}
