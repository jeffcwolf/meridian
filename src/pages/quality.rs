//! The data-quality dashboard: validation-message counts (errors, warnings,
//! inconsistencies) across filings, by country.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::{resource_view, Stat};
use crate::model::QualitySummary;

/// Server function backing the data-quality dashboard.
#[server(FetchQuality)]
pub async fn fetch_quality() -> Result<QualitySummary, ServerFnError> {
    crate::data::quality_summary().map_err(|e| ServerFnError::new(e.to_string()))
}

/// Data-quality page: renders validation-message counts by country.
#[component]
pub fn QualityPage() -> impl IntoView {
    let data = Resource::new_blocking(|| (), |_| async { fetch_quality().await });

    view! {
        <Title text="Data quality · Meridian" />
        <section class="page-intro">
            <h1>"Data quality"</h1>
            <p class="muted">
                "Validation messages run by XBRL International on every filing, categorised by
                 severity and aggregated by country — a proxy for filing quality across the index."
            </p>
        </section>

        {resource_view(data, "Loading…", |q| view! { <QualityView q=q /> })}
    }
}

#[component]
fn QualityView(q: QualitySummary) -> impl IntoView {
    let QualitySummary {
        by_country,
        filings,
        errors,
        warnings,
        inconsistencies,
        clean,
    } = q;
    let clean_pct = if filings > 0 {
        (clean as f64 / filings as f64 * 100.0).round() as i64
    } else {
        0
    };

    let body = by_country
        .into_iter()
        .map(|c| {
            view! {
                <tr>
                    <td class="col-name">
                        {c.country_name}
                        <span class="lei">{c.country}</span>
                    </td>
                    <td class="num">{c.filings}</td>
                    <td class="num sev-error">{c.errors}</td>
                    <td class="num sev-warn">{c.warnings}</td>
                    <td class="num">{c.inconsistencies}</td>
                    <td class="num">{c.clean}</td>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="stat-grid">
            <Stat label="Filings" value=filings.to_string() />
            <Stat label="Errors" value=errors.to_string() warn=true />
            <Stat label="Warnings" value=warnings.to_string() />
            <Stat label="Inconsistencies" value=inconsistencies.to_string() />
            <Stat label="Clean filings" value=format!("{clean_pct}%") />
        </div>

        <div class="table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th scope="col">"Country"</th>
                        <th scope="col" class="num">"Filings"</th>
                        <th scope="col" class="num">"Errors"</th>
                        <th scope="col" class="num">"Warnings"</th>
                        <th scope="col" class="num">"Inconsistencies"</th>
                        <th scope="col" class="num">"Clean"</th>
                    </tr>
                </thead>
                <tbody>{body}</tbody>
            </table>
            <p class="caption">
                "Source: filings.xbrl.org validation messages · clean = filings with no messages"
            </p>
        </div>
    }
}
