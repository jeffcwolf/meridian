//! Data structures shared between the SSR data layer and the UI. These cross
//! the server-function boundary, so they must be (de)serializable and available
//! in both the `ssr` and `hydrate` builds.

use serde::{Deserialize, Serialize};

/// One row on the search page: a company plus a summary of its filings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompanySummary {
    /// Entity row id in the local cache (used to build detail/compare links).
    pub id: i64,
    /// Issuer's legal name.
    pub name: String,
    /// Domicile as an ISO 3166-1 alpha-2 code, when known.
    pub country: Option<String>,
    /// Legal Entity Identifier, when the issuer has one on record.
    pub lei: Option<String>,
    /// Number of filings held for this company.
    pub filing_count: i64,
    /// Earliest reporting year covered, as `YYYY`.
    pub first_year: Option<String>,
    /// Latest reporting year covered, as `YYYY`.
    pub last_year: Option<String>,
}

/// One filing in a company's timeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilingRow {
    /// Reporting-period end date (`YYYY-MM-DD`), the filing's fiscal year end.
    pub reporting_date: Option<String>,
    /// Filing jurisdiction as an ISO 3166-1 alpha-2 code.
    pub country: Option<String>,
    /// Landing page for the filing on filings.xbrl.org.
    pub filing_url: Option<String>,
    /// Direct link to the xBRL-JSON (OIM) extract, when available.
    pub xbrl_json_url: Option<String>,
    /// Total validation messages recorded against the filing by the index.
    pub validation_message_count: i64,
}

/// One IFRS concept row in the financials table, with a formatted cell per year.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConceptRow {
    /// IFRS taxonomy tag, stored as-is (e.g. `ifrs-full:Revenue`).
    pub concept: String,
    /// Human-readable label shown in the row header.
    pub label: String,
    /// Aligned to [`CompanyDetail::years`]; `None` where the concept is absent.
    pub cells: Vec<Option<String>>,
}

/// One company's column in the cross-country comparator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareColumn {
    /// Entity row id, for the column's link back to the company page.
    pub id: i64,
    /// Issuer's legal name (the column header).
    pub name: String,
    /// Domicile as an ISO 3166-1 alpha-2 code, when known.
    pub country: Option<String>,
    /// Presentation currency of the figures, as an ISO 4217 code.
    pub currency: Option<String>,
    /// Formatted values aligned to [`CompareTable::labels`]; `None` where absent.
    pub cells: Vec<Option<String>>,
}

/// The comparator table: IFRS concept rows × selected companies for one year.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompareTable {
    /// The fiscal year the table is showing, as `YYYY`.
    pub fy: String,
    /// Fiscal years available across the selected companies (for the picker).
    pub years: Vec<String>,
    /// Row labels, one per IFRS concept, in display order.
    pub labels: Vec<String>,
    /// One column per selected company, in the order requested.
    pub columns: Vec<CompareColumn>,
    /// Common currency figures were converted to, or `None` when shown native.
    pub base: Option<String>,
}

/// One country's row in the coverage map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoverageRow {
    /// ISO 3166-1 alpha-2 code of the jurisdiction.
    pub country: String,
    /// Full country name for display.
    pub country_name: String,
    /// Entities domiciled here in the seeded universe.
    pub entities: i64,
    /// Of those, how many have at least one filing cached.
    pub entities_with_filings: i64,
    /// Total filings cached for the jurisdiction.
    pub filings: i64,
    /// Whether the filings.xbrl.org index covers this jurisdiction at all.
    pub indexed: bool,
}

/// Country coverage across the seeded universe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoverageSummary {
    /// Per-country breakdown, one entry per jurisdiction.
    pub rows: Vec<CoverageRow>,
    /// Distinct countries represented.
    pub countries: i64,
    /// Countries with at least one cached filing.
    pub covered: i64,
    /// Countries present in the universe but with no cached filings.
    pub gaps: i64,
    /// Total entities across all countries.
    pub entities: i64,
    /// Total filings across all countries.
    pub filings: i64,
}

/// One country's aggregated data-quality figures.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityByCountry {
    /// ISO 3166-1 alpha-2 code of the jurisdiction.
    pub country: String,
    /// Full country name for display.
    pub country_name: String,
    /// Filings assessed for this country.
    pub filings: i64,
    /// Filings carrying at least one validation error.
    pub errors: i64,
    /// Filings carrying at least one validation warning.
    pub warnings: i64,
    /// Filings carrying at least one reported inconsistency.
    pub inconsistencies: i64,
    /// Filings with no validation messages of any severity.
    pub clean: i64,
}

/// Validation-message quality across all filings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualitySummary {
    /// Per-country breakdown, one entry per jurisdiction.
    pub by_country: Vec<QualityByCountry>,
    /// Total filings assessed.
    pub filings: i64,
    /// Filings carrying at least one validation error.
    pub errors: i64,
    /// Filings carrying at least one validation warning.
    pub warnings: i64,
    /// Filings carrying at least one reported inconsistency.
    pub inconsistencies: i64,
    /// Filings with no validation messages of any severity.
    pub clean: i64,
}

/// One company's extension usage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtensionByCompany {
    /// Entity row id, for the link back to the company page.
    pub id: i64,
    /// Issuer's legal name.
    pub name: String,
    /// Domicile as an ISO 3166-1 alpha-2 code, when known.
    pub country: Option<String>,
    /// Distinct company-specific extension concepts this issuer defined.
    pub count: i64,
    /// A few example extension tags, for illustration.
    pub samples: Vec<String>,
}

/// One extension concept and how many companies use it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtensionConcept {
    /// The company-specific extension tag (e.g. `ext:PatrimonioNetto`).
    pub concept: String,
    /// Namespace prefix of the extension (e.g. `ext`).
    pub prefix: String,
    /// How many companies use this concept.
    pub companies: i64,
}

/// Extension-tag usage across the universe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtensionSummary {
    /// Extension usage grouped by issuer.
    pub by_company: Vec<ExtensionByCompany>,
    /// Extension usage grouped by concept.
    pub by_concept: Vec<ExtensionConcept>,
    /// Distinct companies using at least one extension.
    pub companies_using: i64,
    /// Distinct extension concepts seen across all companies.
    pub distinct_concepts: i64,
    /// Total extension facts cached.
    pub total_facts: i64,
}

/// Everything the company detail page needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompanyDetail {
    /// Entity row id in the local cache.
    pub id: i64,
    /// Issuer's legal name.
    pub name: String,
    /// Domicile as an ISO 3166-1 alpha-2 code, when known.
    pub country: Option<String>,
    /// Legal Entity Identifier, when the issuer has one on record.
    pub lei: Option<String>,
    /// Presentation currency of the figures, as an ISO 4217 code.
    pub currency: Option<String>,
    /// Reporting years, most recent first — the financials table columns.
    pub years: Vec<String>,
    /// IFRS concept rows, each with one formatted cell per year.
    pub rows: Vec<ConceptRow>,
    /// The company's filing timeline, most recent first.
    pub filings: Vec<FilingRow>,
}
