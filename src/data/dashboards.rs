//! Aggregate reads backing the coverage, data-quality and extension dashboards.
//! Each groups the cache a different way — by country coverage, by validation
//! severity, and by extension tag.

use super::open_db;
use crate::model::{
    CoverageRow, CoverageSummary, ExtensionByCompany, ExtensionConcept, ExtensionSummary,
    QualityByCountry, QualitySummary,
};

/// ISO 3166-1 alpha-2 -> country name for the jurisdictions we surface.
fn country_name(code: &str) -> String {
    match code {
        "AT" => "Austria",
        "BE" => "Belgium",
        "CH" => "Switzerland",
        "CZ" => "Czechia",
        "DE" => "Germany",
        "DK" => "Denmark",
        "ES" => "Spain",
        "FI" => "Finland",
        "FR" => "France",
        "GB" => "United Kingdom",
        "GR" => "Greece",
        "IE" => "Ireland",
        "IT" => "Italy",
        "LU" => "Luxembourg",
        "NL" => "Netherlands",
        "NO" => "Norway",
        "PL" => "Poland",
        "PT" => "Portugal",
        "SE" => "Sweden",
        other => other,
    }
    .to_string()
}

/// Per-country coverage across the seeded universe — how many issuers, how many
/// actually have discoverable filings, and where the index has gaps.
pub(crate) fn coverage() -> rusqlite::Result<CoverageSummary> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT e.country,
                COUNT(DISTINCT e.id) AS entities,
                COUNT(DISTINCT CASE WHEN f.id IS NOT NULL THEN e.id END) AS with_filings,
                COUNT(f.id) AS filings
         FROM entities e
         LEFT JOIN filings f ON f.entity_id = e.id
         GROUP BY e.country
         ORDER BY entities DESC, e.country",
    )?;
    let rows: Vec<CoverageRow> = stmt
        .query_map([], |r| {
            let country: String = r.get::<_, Option<String>>(0)?.unwrap_or_default();
            let with_filings: i64 = r.get(2)?;
            Ok(CoverageRow {
                country_name: country_name(&country),
                indexed: with_filings > 0,
                country,
                entities: r.get(1)?,
                entities_with_filings: with_filings,
                filings: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let countries = rows.len() as i64;
    let covered = rows.iter().filter(|r| r.indexed).count() as i64;
    Ok(CoverageSummary {
        countries,
        covered,
        gaps: countries - covered,
        entities: rows.iter().map(|r| r.entities).sum(),
        filings: rows.iter().map(|r| r.filings).sum(),
        rows,
    })
}

/// Validation-message quality aggregated by country, plus overall totals.
pub(crate) fn quality_summary() -> rusqlite::Result<QualitySummary> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT f.country,
                COUNT(f.id) AS filings,
                COALESCE(SUM(f.error_count), 0) AS errors,
                COALESCE(SUM(f.warning_count), 0) AS warnings,
                COALESCE(SUM(f.inconsistency_count), 0) AS inconsistencies,
                COALESCE(SUM(CASE WHEN f.validation_message_count = 0 THEN 1 ELSE 0 END), 0) AS clean
         FROM filings f
         GROUP BY f.country
         ORDER BY errors DESC, warnings DESC, f.country",
    )?;
    let by_country: Vec<QualityByCountry> = stmt
        .query_map([], |r| {
            let country: String = r.get::<_, Option<String>>(0)?.unwrap_or_default();
            Ok(QualityByCountry {
                country_name: country_name(&country),
                country,
                filings: r.get(1)?,
                errors: r.get(2)?,
                warnings: r.get(3)?,
                inconsistencies: r.get(4)?,
                clean: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(QualitySummary {
        filings: by_country.iter().map(|c| c.filings).sum(),
        errors: by_country.iter().map(|c| c.errors).sum(),
        warnings: by_country.iter().map(|c| c.warnings).sum(),
        inconsistencies: by_country.iter().map(|c| c.inconsistencies).sum(),
        clean: by_country.iter().map(|c| c.clean).sum(),
        by_country,
    })
}

/// Extension-tag usage: which issuers define company-specific tags, and which
/// extensions are most common (a signal for where the IFRS taxonomy falls short).
pub(crate) fn extension_summary() -> rusqlite::Result<ExtensionSummary> {
    let conn = open_db()?;

    let mut cstmt = conn.prepare(
        "SELECT e.id, e.name, e.country, COUNT(DISTINCT x.concept) AS n
         FROM entities e JOIN extension_facts x ON x.entity_id = e.id
         GROUP BY e.id
         ORDER BY n DESC, e.name",
    )?;
    let mut by_company: Vec<ExtensionByCompany> = cstmt
        .query_map([], |r| {
            Ok(ExtensionByCompany {
                id: r.get(0)?,
                name: r.get(1)?,
                country: r.get(2)?,
                count: r.get(3)?,
                samples: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut sstmt = conn.prepare(
        "SELECT DISTINCT concept FROM extension_facts
         WHERE entity_id = ?1 ORDER BY concept LIMIT 6",
    )?;
    for company in &mut by_company {
        company.samples = sstmt
            .query_map([company.id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;
    }

    let mut kstmt = conn.prepare(
        "SELECT concept, prefix, COUNT(DISTINCT entity_id) AS n
         FROM extension_facts
         GROUP BY concept
         ORDER BY n DESC, concept
         LIMIT 40",
    )?;
    let by_concept: Vec<ExtensionConcept> = kstmt
        .query_map([], |r| {
            Ok(ExtensionConcept {
                concept: r.get(0)?,
                prefix: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                companies: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let companies_using = by_company.len() as i64;
    let distinct_concepts = conn.query_row(
        "SELECT COUNT(DISTINCT concept) FROM extension_facts",
        [],
        |r| r.get(0),
    )?;
    let total_facts = conn.query_row("SELECT COUNT(*) FROM extension_facts", [], |r| r.get(0))?;

    Ok(ExtensionSummary {
        by_company,
        by_concept,
        companies_using,
        distinct_concepts,
        total_facts,
    })
}
