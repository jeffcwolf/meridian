//! Integration tests for the SQLite read layer and the CSV/JSON export
//! handlers, exercised against a seeded temporary database.
//!
//! The read functions open the database named by `MERIDIAN_DB` (see
//! [`super::open_db`]), so the suite seeds one read-only fixture database once
//! per test process and points `MERIDIAN_DB` at it.

use std::sync::Once;

use axum::body::to_bytes;
use axum::extract::{Path, RawQuery};
use axum::http::StatusCode;

use super::{
    company_export, compare, coverage, extension_summary, list_companies, load_company,
    quality_summary,
};

const SCHEMA_AND_FIXTURES: &str = r#"
CREATE TABLE entities (
    id INTEGER PRIMARY KEY, name TEXT NOT NULL, lei TEXT UNIQUE, country TEXT
);
CREATE TABLE filings (
    id INTEGER PRIMARY KEY, entity_id INTEGER NOT NULL, reporting_date TEXT,
    filing_url TEXT, xbrl_json_url TEXT, country TEXT,
    validation_message_count INTEGER DEFAULT 0, error_count INTEGER DEFAULT 0,
    warning_count INTEGER DEFAULT 0, inconsistency_count INTEGER DEFAULT 0
);
CREATE TABLE financial_facts (
    id INTEGER PRIMARY KEY, entity_id INTEGER NOT NULL, reporting_date TEXT,
    concept TEXT NOT NULL, value TEXT, currency TEXT
);
CREATE TABLE extension_facts (
    id INTEGER PRIMARY KEY, entity_id INTEGER NOT NULL, reporting_date TEXT,
    concept TEXT NOT NULL, prefix TEXT, value TEXT, currency TEXT
);
CREATE TABLE fx_rates (
    currency TEXT NOT NULL, year TEXT NOT NULL, rate_per_eur REAL NOT NULL,
    PRIMARY KEY (currency, year)
);

INSERT INTO entities (id, name, lei, country) VALUES
    (1, 'Siemens AG', 'LEI-SIE', 'DE'),
    (2, 'SAP SE', 'LEI-SAP', 'DE'),
    (3, 'Novo Nordisk', 'LEI-NOVO', 'DK');

INSERT INTO filings
    (entity_id, reporting_date, filing_url, xbrl_json_url, country,
     validation_message_count, error_count, warning_count, inconsistency_count)
VALUES
    (1, '2023-09-30', 'https://x/sie', 'https://x/sie.json', 'DE', 0, 0, 0, 0),
    (2, '2023-12-31', 'https://x/sap', 'https://x/sap.json', 'DE', 2, 0, 2, 0),
    (3, '2023-12-31', 'https://x/novo', 'https://x/novo.json', 'DK', 1, 1, 0, 0);

INSERT INTO financial_facts (entity_id, reporting_date, concept, value, currency) VALUES
    (1, '2023-09-30', 'ifrs-full:Revenue', '77769000000', 'EUR'),
    (1, '2023-09-30', 'ifrs-full:Assets', '138000000000', 'EUR'),
    (2, '2023-12-31', 'ifrs-full:Revenue', '31207000000', 'EUR'),
    (3, '2023-12-31', 'ifrs-full:Revenue', '232261000000', 'DKK');

INSERT INTO extension_facts (entity_id, reporting_date, concept, prefix, value, currency)
VALUES (1, '2023-09-30', 'sie:CustomTag', 'sie', '123', 'EUR');

INSERT INTO fx_rates (currency, year, rate_per_eur) VALUES ('DKK', '2023', 7.4508);
"#;

static SEED: Once = Once::new();

/// Seed a fresh temporary database once per test process and point the data
/// layer at it. Tests only read, so the fixture is shared across parallel tests.
fn ensure_seeded() {
    SEED.call_once(|| {
        let path = std::env::temp_dir().join("meridian-read-layer-itest.db");
        let _ = std::fs::remove_file(&path);
        let conn = rusqlite::Connection::open(&path).expect("open temp test database");
        conn.execute_batch(SCHEMA_AND_FIXTURES)
            .expect("seed temp test database");
        std::env::set_var("MERIDIAN_DB", &path);
    });
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    String::from_utf8(bytes.to_vec()).expect("utf-8 body")
}

#[test]
fn list_companies_returns_every_seeded_company() {
    ensure_seeded();
    let companies = list_companies(None).unwrap();
    assert_eq!(companies.len(), 3);
    assert!(companies.iter().any(|c| c.name == "Siemens AG"));
}

#[test]
fn list_companies_filters_by_case_insensitive_substring() {
    ensure_seeded();
    let companies = list_companies(Some("sap")).unwrap();
    assert_eq!(companies.len(), 1);
    assert_eq!(companies[0].name, "SAP SE");
}

#[test]
fn load_company_returns_detail_for_a_known_id() {
    ensure_seeded();
    let detail = load_company(1).unwrap().expect("company 1 exists");
    assert_eq!(detail.name, "Siemens AG");
    assert_eq!(detail.currency.as_deref(), Some("EUR"));
    assert!(detail.years.contains(&"2023".to_string()));
    assert!(detail.rows.iter().any(|r| r.label == "Revenue"));
}

#[test]
fn load_company_returns_none_for_an_unknown_id() {
    ensure_seeded();
    assert!(load_company(9999).unwrap().is_none());
}

#[test]
fn company_export_returns_raw_facts_with_currencies() {
    ensure_seeded();
    let export = company_export(1).unwrap().expect("company 1 exists");
    assert_eq!(export.name, "Siemens AG");
    assert!(export
        .facts
        .iter()
        .any(|f| f.concept == "ifrs-full:Revenue" && f.value == "77769000000"));
}

#[test]
fn compare_aligns_the_requested_companies_into_columns() {
    ensure_seeded();
    let table = compare(&[1, 2], None, None).unwrap();
    assert_eq!(table.columns.len(), 2);
    assert_eq!(table.fy, "2023");
    assert!(table.base.is_none());
}

#[test]
fn compare_converts_every_column_to_the_base_currency() {
    ensure_seeded();
    let table = compare(&[1, 3], None, Some("EUR")).unwrap();
    assert_eq!(table.base.as_deref(), Some("EUR"));
    let revenue = table
        .labels
        .iter()
        .position(|l| l == "Revenue")
        .expect("Revenue row present");
    let siemens = table.columns.iter().find(|c| c.id == 1).unwrap();
    let novo = table.columns.iter().find(|c| c.id == 3).unwrap();
    assert_eq!(siemens.cells[revenue].as_deref(), Some("77,769"));
    // 232,261,000,000 DKK / 7.4508 ≈ 31,172 million EUR.
    assert_eq!(novo.cells[revenue].as_deref(), Some("31,172"));
    assert_eq!(novo.currency.as_deref(), Some("EUR"));
}

#[test]
fn coverage_summarises_countries_entities_and_gaps() {
    ensure_seeded();
    let summary = coverage().unwrap();
    assert_eq!(summary.countries, 2); // DE, DK
    assert_eq!(summary.entities, 3);
    assert_eq!(summary.filings, 3);
    assert_eq!(summary.covered, 2);
    assert_eq!(summary.gaps, 0);
}

#[test]
fn quality_summary_aggregates_validation_severities() {
    ensure_seeded();
    let summary = quality_summary().unwrap();
    assert_eq!(summary.filings, 3);
    assert_eq!(summary.errors, 1); // Novo
    assert_eq!(summary.warnings, 2); // SAP
    assert_eq!(summary.clean, 1); // Siemens has no validation messages
}

#[test]
fn extension_summary_reports_company_specific_tags() {
    ensure_seeded();
    let summary = extension_summary().unwrap();
    assert_eq!(summary.total_facts, 1);
    assert_eq!(summary.companies_using, 1);
    assert_eq!(summary.distinct_concepts, 1);
    assert_eq!(summary.by_company[0].name, "Siemens AG");
}

#[tokio::test]
async fn company_export_handler_returns_csv_with_company_data() {
    ensure_seeded();
    let response = crate::export::company_export(Path((1_i64, "csv".to_string()))).await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body_text(response).await;
    assert!(text.starts_with("company,lei,country,"));
    assert!(text.contains("Siemens AG"));
}

#[tokio::test]
async fn compare_export_handler_rejects_fewer_than_two_companies() {
    ensure_seeded();
    let response =
        crate::export::compare_export(Path("csv".to_string()), RawQuery(Some("id=1".to_string())))
            .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn compare_export_handler_returns_csv_for_two_companies() {
    ensure_seeded();
    let response = crate::export::compare_export(
        Path("csv".to_string()),
        RawQuery(Some("id=1&id=2".to_string())),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body_text(response).await;
    assert!(text.contains("Siemens AG"));
    assert!(text.contains("SAP SE"));
}
