//! Reads over companies and their financial facts: summaries for the search
//! page, the pivoted figures behind the detail view, and the full-precision
//! export. The comparator ([`super::compare`]) builds on [`raw_company`].

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension};

use super::format::{fmt_millions, parse_amount};
use super::open_db;
use crate::model::{CompanyDetail, CompanySummary, ConceptRow, FilingRow};

/// Headline IFRS rows in display order: a label plus the accepted concept tags
/// (primary first). Issuers tag the same line differently, so each row coalesces
/// across its aliases, preferring the primary tag.
pub(crate) const CONCEPTS: [(&str, &[&str]); 7] = [
    (
        "Revenue",
        &[
            "ifrs-full:Revenue",
            "ifrs-full:RevenueFromContractsWithCustomers",
            "ifrs-full:RevenueFromSaleOfGoods",
        ],
    ),
    // Bank income lines — banks have no single "Revenue". Rows that are empty
    // for every company in a view are dropped, so these only appear for banks.
    ("Interest income", &["ifrs-full:RevenueFromInterest"]),
    (
        "Fee & commission income",
        &[
            "ifrs-full:RevenueFromFeeAndCommissionIncome",
            "ifrs-full:FeeAndCommissionIncome",
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

/// SQL expression mapping a reporting-date column to its fiscal year — the year
/// of the date six months before period end. This keeps 52/53-week retail filers
/// (whose year-end drifts across Dec/Jan) and January year-ends aligned, and
/// avoids two of one issuer's year-ends landing in the same column.
fn fiscal_year_sql(column: &str) -> String {
    format!("strftime('%Y', {column}, '-182 days')")
}

/// Companies for the search page, optionally filtered by a case-insensitive
/// substring of name, country, or LEI.
pub(crate) fn list_companies(query: Option<&str>) -> rusqlite::Result<Vec<CompanySummary>> {
    let conn = open_db()?;
    let like = query.map(|q| format!("%{}%", q.to_lowercase()));
    let fiscal_year = fiscal_year_sql("f.reporting_date");
    let mut stmt = conn.prepare(&format!(
        "SELECT e.id, e.name, e.country, e.lei,
                COUNT(f.id) AS filing_count,
                MIN({fiscal_year}) AS first_year,
                MAX({fiscal_year}) AS last_year
         FROM entities e
         LEFT JOIN filings f ON f.entity_id = e.id
         WHERE (?1 IS NULL
                OR lower(e.name) LIKE ?1
                OR lower(IFNULL(e.country, '')) LIKE ?1
                OR lower(IFNULL(e.lei, '')) LIKE ?1)
         GROUP BY e.id
         ORDER BY e.name"
    ))?;
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

/// A company's raw (unformatted) figures — the shared foundation for the detail
/// view and the comparator. Each row is aligned to [`CONCEPTS`], each cell to
/// `years`, and holds the amount in full currency units plus its ISO currency.
pub(crate) struct RawCompany {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) country: Option<String>,
    pub(crate) lei: Option<String>,
    pub(crate) currency: Option<String>,
    pub(crate) years: Vec<String>,
    pub(crate) rows: Vec<Vec<Option<(i128, String)>>>,
}

/// Load one company's facts pivoted by concept (with alias coalescing) and
/// fiscal year, most recent year first. Returns `None` if the id is unknown.
pub(crate) fn raw_company(conn: &Connection, id: i64) -> rusqlite::Result<Option<RawCompany>> {
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

    let fiscal_year = fiscal_year_sql("reporting_date");
    let mut ystmt = conn.prepare(&format!(
        "SELECT DISTINCT {fiscal_year} AS fy FROM filings
         WHERE entity_id = ?1 AND reporting_date IS NOT NULL
         ORDER BY fy DESC"
    ))?;
    let years: Vec<String> = ystmt
        .query_map([id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;

    // (concept, fiscal_year) -> (amount, currency)
    let mut xstmt = conn.prepare(&format!(
        "SELECT concept, {fiscal_year} AS fy, value, currency
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
pub(crate) fn load_company(id: i64) -> rusqlite::Result<Option<CompanyDetail>> {
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
        // Drop rows with no data at all (e.g. Revenue for a bank, or the bank
        // income lines for a non-bank).
        .filter(|row| row.cells.iter().any(Option::is_some))
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

/// One exported fact: the raw tag and value with its source filing URL.
#[derive(serde::Serialize)]
pub(crate) struct ExportFact {
    pub concept: String,
    pub fiscal_year: String,
    pub value: String,
    pub currency: Option<String>,
    pub source_url: Option<String>,
}

/// A company's full parsed facts for export (raw values, not display-formatted).
#[derive(serde::Serialize)]
pub(crate) struct CompanyExport {
    pub id: i64,
    pub name: String,
    pub lei: Option<String>,
    pub country: Option<String>,
    pub facts: Vec<ExportFact>,
}

/// All parsed facts for one company, with source URLs, for CSV/JSON export.
pub(crate) fn company_export(id: i64) -> rusqlite::Result<Option<CompanyExport>> {
    let conn = open_db()?;
    let meta = conn
        .query_row(
            "SELECT name, lei, country FROM entities WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((name, lei, country)) = meta else {
        return Ok(None);
    };

    let fiscal_year = fiscal_year_sql("x.reporting_date");
    let mut stmt = conn.prepare(&format!(
        "SELECT x.concept, {fiscal_year} AS fy, x.value, x.currency, f.xbrl_json_url
         FROM financial_facts x
         LEFT JOIN filings f
           ON f.entity_id = x.entity_id AND f.reporting_date = x.reporting_date
         WHERE x.entity_id = ?1
         ORDER BY fy DESC, x.concept"
    ))?;
    let facts = stmt
        .query_map([id], |r| {
            Ok(ExportFact {
                concept: r.get(0)?,
                fiscal_year: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                value: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                currency: r.get(3)?,
                source_url: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(Some(CompanyExport {
        id,
        name,
        lei,
        country,
        facts,
    }))
}
