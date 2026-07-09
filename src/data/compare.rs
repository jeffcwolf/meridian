//! The cross-country comparator: align several companies for one fiscal year,
//! optionally converting every column to a common currency at ECB annual rates.

use std::collections::{BTreeSet, HashMap};

use rusqlite::Connection;

use super::format::fmt_millions;
use super::open_db;
use super::reads::{raw_company, RawCompany, CONCEPTS};
use crate::model::{CompareColumn, CompareTable};

/// Align several companies' IFRS figures for one fiscal year, one column each.
/// Unknown ids are skipped. `fy` defaults to the most recent year available.
/// `base` (an ISO code such as `"EUR"`) converts every column to that currency
/// at ECB annual-average rates; `None`/`"native"` keeps each issuer's own.
pub(crate) fn compare(
    ids: &[i64],
    fy: Option<&str>,
    base: Option<&str>,
) -> rusqlite::Result<CompareTable> {
    let conn = open_db()?;
    let fx = load_fx(&conn)?;

    let companies: Vec<RawCompany> = ids
        .iter()
        .filter_map(|&id| raw_company(&conn, id).transpose())
        .collect::<rusqlite::Result<_>>()?;

    let fy = resolve_year(&companies, fy);
    let years = available_years(&companies);
    let base = base
        .filter(|b| !b.is_empty() && !b.eq_ignore_ascii_case("native"))
        .map(str::to_uppercase);

    let mut columns: Vec<CompareColumn> = companies
        .into_iter()
        .map(|company| column_for(company, &fy, base.as_deref(), &fx))
        .collect();
    let labels = drop_empty_rows(&mut columns);

    Ok(CompareTable {
        fy,
        years,
        labels,
        columns,
        base,
    })
}

/// The fiscal years present across the selected companies, most recent first.
fn available_years(companies: &[RawCompany]) -> Vec<String> {
    let mut years: BTreeSet<String> = BTreeSet::new();
    for company in companies {
        years.extend(company.years.iter().cloned());
    }
    years.into_iter().rev().collect()
}

/// The requested year if it exists across the selection, else the most recent.
fn resolve_year(companies: &[RawCompany], requested: Option<&str>) -> String {
    let years = available_years(companies);
    requested
        .map(str::to_string)
        .filter(|y| years.contains(y))
        .or_else(|| years.first().cloned())
        .unwrap_or_default()
}

/// Build one company's column for `fy`, converting to `base` where possible.
fn column_for(
    company: RawCompany,
    fy: &str,
    base: Option<&str>,
    fx: &HashMap<(String, String), f64>,
) -> CompareColumn {
    let year_index = company.years.iter().position(|y| y == fy);
    let cells = company
        .rows
        .iter()
        .map(|row| {
            let cell = year_index.and_then(|i| row.get(i).cloned().flatten());
            cell.and_then(|(amount, currency)| match base {
                Some(target) => convert(amount, &currency, target, fy, fx).map(fmt_millions),
                None => Some(fmt_millions(amount)),
            })
        })
        .collect();
    CompareColumn {
        currency: base.map(str::to_string).or(company.currency),
        id: company.id,
        name: company.name,
        country: company.country,
        cells,
    }
}

/// Drop concept rows that are empty for every column (e.g. the bank income lines
/// when comparing non-banks, or Revenue when comparing banks). Returns the labels
/// of the rows that were kept, in order.
fn drop_empty_rows(columns: &mut [CompareColumn]) -> Vec<String> {
    let kept: Vec<usize> = (0..CONCEPTS.len())
        .filter(|&i| {
            columns
                .iter()
                .any(|c| matches!(c.cells.get(i), Some(Some(_))))
        })
        .collect();
    for column in columns.iter_mut() {
        column.cells = kept.iter().map(|&i| column.cells[i].clone()).collect();
    }
    kept.iter().map(|&i| CONCEPTS[i].0.to_string()).collect()
}

/// Load all FX rates as `(currency, year) -> units per EUR`.
fn load_fx(conn: &Connection) -> rusqlite::Result<HashMap<(String, String), f64>> {
    let mut stmt = conn.prepare("SELECT currency, year, rate_per_eur FROM fx_rates")?;
    let mut rates = HashMap::new();
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, f64>(2)?,
        ))
    })?;
    for row in rows {
        let (currency, year, rate) = row?;
        rates.insert((currency, year), rate);
    }
    Ok(rates)
}

fn rate_per_eur(fx: &HashMap<(String, String), f64>, currency: &str, year: &str) -> Option<f64> {
    if currency.eq_ignore_ascii_case("EUR") {
        Some(1.0)
    } else {
        fx.get(&(currency.to_string(), year.to_string())).copied()
    }
}

/// Convert `amount` from currency `from` to `to` for a given fiscal year, via
/// EUR. Returns `None` if a needed rate is missing.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rates() -> HashMap<(String, String), f64> {
        // 1 EUR = 7.4508 DKK = 1.0813 USD in 2023.
        HashMap::from([
            (("DKK".to_string(), "2023".to_string()), 7.4508),
            (("USD".to_string(), "2023".to_string()), 1.0813),
        ])
    }

    #[test]
    fn same_currency_is_unchanged() {
        assert_eq!(convert(1_000, "EUR", "EUR", "2023", &rates()), Some(1_000));
    }

    #[test]
    fn foreign_to_eur_divides_by_the_rate() {
        // 232,261,000,000 DKK / 7.4508 ≈ 31,172,000,000 EUR.
        let eur = convert(232_261_000_000, "DKK", "EUR", "2023", &rates()).unwrap();
        assert_eq!(fmt_millions(eur), "31,172");
    }

    #[test]
    fn missing_rate_yields_none() {
        assert_eq!(convert(1_000, "NOK", "EUR", "2023", &rates()), None);
    }
}
