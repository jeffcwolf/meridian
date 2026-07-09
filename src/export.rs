//! CSV / JSON export endpoints (plain Axum handlers, outside the Leptos render
//! path). Company exports carry raw values and source filing URLs; comparison
//! exports carry the figures as shown (converted, in millions).

use axum::extract::{Path, RawQuery};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::data;

fn download(body: String, content_type: &'static str, filename: &str) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response()
}

/// Quote a CSV field if it contains a comma, quote, or newline.
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn parse_ids(query: &str) -> Vec<i64> {
    query
        .split('&')
        .filter_map(|kv| kv.strip_prefix("id="))
        .filter_map(|v| v.parse::<i64>().ok())
        .collect()
}

fn parse_param(query: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    query
        .split('&')
        .find_map(|kv| kv.strip_prefix(&prefix))
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

/// `GET /export/company/{id}/{format}` — one company's facts as CSV or JSON.
pub async fn company_export(Path((id, format)): Path<(i64, String)>) -> Response {
    let export = match data::company_export(id) {
        Ok(Some(e)) => e,
        Ok(None) => return (StatusCode::NOT_FOUND, "Company not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    match format.as_str() {
        "json" => match serde_json::to_string_pretty(&export) {
            Ok(body) => download(
                body,
                "application/json",
                &format!("meridian-company-{id}.json"),
            ),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        "csv" => {
            let mut out = String::from(
                "company,lei,country,concept_tag,fiscal_year,value,currency,source_url\n",
            );
            for f in &export.facts {
                out.push_str(&format!(
                    "{},{},{},{},{},{},{},{}\n",
                    csv_field(&export.name),
                    csv_field(export.lei.as_deref().unwrap_or("")),
                    csv_field(export.country.as_deref().unwrap_or("")),
                    csv_field(&f.concept),
                    csv_field(&f.fiscal_year),
                    csv_field(&f.value),
                    csv_field(f.currency.as_deref().unwrap_or("")),
                    csv_field(f.source_url.as_deref().unwrap_or("")),
                ));
            }
            download(out, "text/csv", &format!("meridian-company-{id}.csv"))
        }
        _ => (StatusCode::NOT_FOUND, "Unknown format").into_response(),
    }
}

/// `GET /export/compare/{format}?id=..&id=..&fy=..&base=..` — a comparison.
pub async fn compare_export(Path(format): Path<String>, RawQuery(query): RawQuery) -> Response {
    let query = query.unwrap_or_default();
    let ids = parse_ids(&query);
    if ids.len() < 2 {
        return (StatusCode::BAD_REQUEST, "Select at least two companies").into_response();
    }
    let fy = parse_param(&query, "fy");
    let base = parse_param(&query, "base");
    let table = match data::compare(&ids, fy.as_deref(), base.as_deref()) {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    match format.as_str() {
        "json" => match serde_json::to_string_pretty(&table) {
            Ok(body) => download(body, "application/json", "meridian-comparison.json"),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        "csv" => {
            let mut out =
                String::from("company,country,currency,concept,fiscal_year,value_millions\n");
            for col in &table.columns {
                for (i, label) in table.labels.iter().enumerate() {
                    let value = col.cells.get(i).cloned().flatten().unwrap_or_default();
                    out.push_str(&format!(
                        "{},{},{},{},{},{}\n",
                        csv_field(&col.name),
                        csv_field(col.country.as_deref().unwrap_or("")),
                        csv_field(col.currency.as_deref().unwrap_or("")),
                        csv_field(label),
                        csv_field(&table.fy),
                        csv_field(&value),
                    ));
                }
            }
            download(out, "text/csv", "meridian-comparison.csv")
        }
        _ => (StatusCode::NOT_FOUND, "Unknown format").into_response(),
    }
}
