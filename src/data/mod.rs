//! Server-side reads from the SQLite cache produced by the Python pipeline.
//! Compiled only in the `ssr` build (rusqlite is not available in wasm).

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension};

use crate::model::{CompanyDetail, CompanySummary, ConceptRow, FilingRow};

/// Headline IFRS rows in display order: a label plus the accepted concept tags
/// (primary first). Issuers tag the same line differently, so each row coalesces
/// across its aliases, preferring the primary tag.
const CONCEPTS: [(&str, &[&str]); 5] = [
    (
        "Revenue",
        &[
            "ifrs-full:Revenue",
            "ifrs-full:RevenueFromContractsWithCustomers",
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
        &["ifrs-full:CashFlowsFromUsedInOperatingActivities"],
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

/// Full detail for one company: entity info, a pivoted IFRS financials table,
/// and the filing timeline. Returns `None` if the id is unknown.
pub fn get_company(id: i64) -> rusqlite::Result<Option<CompanyDetail>> {
    let conn = open_db()?;

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

    // Filing timeline (most recent first).
    let mut fstmt = conn.prepare(
        "SELECT reporting_date, country, filing_url, xbrl_json_url, validation_message_count
         FROM filings WHERE entity_id = ?1 ORDER BY reporting_date DESC",
    )?;
    let filings: Vec<FilingRow> = fstmt
        .query_map([id], |r| {
            Ok(FilingRow {
                reporting_date: r.get(0)?,
                country: r.get(1)?,
                filing_url: r.get(2)?,
                xbrl_json_url: r.get(3)?,
                validation_message_count: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // Distinct fiscal years present in the filings, most recent first.
    let mut ystmt = conn.prepare(&format!(
        "SELECT DISTINCT {FISCAL_YEAR} AS fy FROM filings
         WHERE entity_id = ?1 AND reporting_date IS NOT NULL
         ORDER BY fy DESC"
    ))?;
    let years: Vec<String> = ystmt
        .query_map([id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;

    // (concept, fiscal_year) -> value; currency is taken from the first fact seen.
    let mut xstmt = conn.prepare(&format!(
        "SELECT concept, {FISCAL_YEAR} AS fy, value, currency
         FROM financial_facts WHERE entity_id = ?1"
    ))?;
    let mut facts: HashMap<(String, String), String> = HashMap::new();
    let mut currency: Option<String> = None;
    let rows = xstmt.query_map([id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })?;
    for row in rows {
        let (concept, fy, value, ccy) = row?;
        if currency.is_none() {
            currency = ccy;
        }
        if let (Some(fy), Some(v)) = (fy, value) {
            facts.insert((concept, fy), v);
        }
    }

    // Build each row, coalescing across the concept's aliases (primary first).
    let rows = CONCEPTS
        .iter()
        .map(|(label, concepts)| {
            let cells = years
                .iter()
                .map(|y| {
                    concepts
                        .iter()
                        .find_map(|c| facts.get(&((*c).to_string(), y.clone())))
                        .map(|v| fmt_millions(v))
                })
                .collect();
            ConceptRow {
                concept: concepts[0].to_string(),
                label: label.to_string(),
                cells,
            }
        })
        .collect();

    Ok(Some(CompanyDetail {
        id,
        name,
        country,
        lei,
        currency,
        years,
        rows,
        filings,
    }))
}

/// Format a full-currency-unit integer string as a thousands-grouped figure in
/// millions, e.g. `"77769000000"` -> `"77,769"`.
fn fmt_millions(raw: &str) -> String {
    let negative = raw.trim_start().starts_with('-');
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    let millions = digits.parse::<i128>().unwrap_or(0) / 1_000_000;
    let grouped = group_thousands(millions);
    if negative && millions != 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
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
