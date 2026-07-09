//! Data structures shared between the SSR data layer and the UI. These cross
//! the server-function boundary, so they must be (de)serializable and available
//! in both the `ssr` and `hydrate` builds.

use serde::{Deserialize, Serialize};

/// One row on the search page: a company plus a summary of its filings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompanySummary {
    pub id: i64,
    pub name: String,
    pub country: Option<String>,
    pub lei: Option<String>,
    pub filing_count: i64,
    pub first_year: Option<String>,
    pub last_year: Option<String>,
}

/// One filing in a company's timeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilingRow {
    pub reporting_date: Option<String>,
    pub country: Option<String>,
    pub filing_url: Option<String>,
    pub xbrl_json_url: Option<String>,
    pub validation_message_count: i64,
}

/// One IFRS concept row in the financials table, with a formatted cell per year.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConceptRow {
    pub concept: String,
    pub label: String,
    /// Aligned to [`CompanyDetail::years`]; `None` where the concept is absent.
    pub cells: Vec<Option<String>>,
}

/// One company's column in the cross-country comparator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareColumn {
    pub id: i64,
    pub name: String,
    pub country: Option<String>,
    pub currency: Option<String>,
    /// Formatted values aligned to [`CompareTable::labels`]; `None` where absent.
    pub cells: Vec<Option<String>>,
}

/// The comparator table: IFRS concept rows × selected companies for one year.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareTable {
    pub fy: String,
    /// Fiscal years available across the selected companies (for the picker).
    pub years: Vec<String>,
    pub labels: Vec<String>,
    pub columns: Vec<CompareColumn>,
    /// Common currency figures were converted to, or `None` when shown native.
    pub base: Option<String>,
}

/// One country's row in the coverage map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoverageRow {
    pub country: String,
    pub country_name: String,
    pub entities: i64,
    pub entities_with_filings: i64,
    pub filings: i64,
    /// Whether the filings.xbrl.org index covers this jurisdiction at all.
    pub indexed: bool,
}

/// Country coverage across the seeded universe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoverageSummary {
    pub rows: Vec<CoverageRow>,
    pub countries: i64,
    pub covered: i64,
    pub gaps: i64,
    pub entities: i64,
    pub filings: i64,
}

/// One country's aggregated data-quality figures.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityByCountry {
    pub country: String,
    pub country_name: String,
    pub filings: i64,
    pub errors: i64,
    pub warnings: i64,
    pub inconsistencies: i64,
    pub clean: i64,
}

/// Validation-message quality across all filings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualitySummary {
    pub by_country: Vec<QualityByCountry>,
    pub filings: i64,
    pub errors: i64,
    pub warnings: i64,
    pub inconsistencies: i64,
    pub clean: i64,
}

/// Everything the company detail page needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompanyDetail {
    pub id: i64,
    pub name: String,
    pub country: Option<String>,
    pub lei: Option<String>,
    pub currency: Option<String>,
    /// Reporting years, most recent first — the financials table columns.
    pub years: Vec<String>,
    pub rows: Vec<ConceptRow>,
    pub filings: Vec<FilingRow>,
}
