//! The coverage map: how many entities and filings are cached per jurisdiction,
//! and which jurisdictions the index omits.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::{resourced, Stat};
use crate::model::CoverageSummary;

/// Server function backing the coverage map.
#[server(CoverageData)]
pub async fn coverage_data() -> Result<CoverageSummary, ServerFnError> {
    crate::data::coverage().map_err(|e| ServerFnError::new(e.to_string()))
}

/// Coverage page: renders the per-country coverage table and summary tiles.
#[component]
pub fn CoveragePage() -> impl IntoView {
    let data = Resource::new_blocking(|| (), |_| async { coverage_data().await });

    view! {
        <Title text="Coverage · Meridian" />
        <section class="page-intro">
            <h1>"Country coverage"</h1>
            <p class="muted">
                "filings.xbrl.org aggregates from national OAMs, and several jurisdictions —
                 most notably Germany — do not publish to the public index. Surfacing that gap is
                 itself informative: it is exactly where cross-border comparability breaks down."
            </p>
        </section>

        {resourced(data, "Loading…", |c| view! { <CoverageView c=c /> })}
    }
}

#[component]
fn CoverageView(c: CoverageSummary) -> impl IntoView {
    let CoverageSummary {
        rows,
        countries,
        covered,
        gaps,
        entities,
        filings,
    } = c;
    let max_filings = rows.iter().map(|r| r.filings).max().unwrap_or(0).max(1);

    let body = rows
        .into_iter()
        .map(|r| {
            let pct = (r.filings as f64 / max_filings as f64 * 100.0).round() as i64;
            let status = if r.indexed {
                view! { <span class="badge badge-ok">"Indexed"</span> }.into_any()
            } else {
                view! { <span class="badge badge-warn">"Gap"</span> }.into_any()
            };
            let row_class = if r.indexed { "" } else { "row-empty" };
            view! {
                <tr class=row_class>
                    <td class="col-name">
                        {r.country_name}
                        <span class="lei">{r.country}</span>
                    </td>
                    <td class="num">{r.entities}</td>
                    <td class="num">{r.entities_with_filings}</td>
                    <td class="num">{r.filings}</td>
                    <td class="bar-cell">
                        <div class="bar">
                            <div class="bar-fill" style=format!("width:{pct}%")></div>
                        </div>
                    </td>
                    <td>{status}</td>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="stat-grid">
            <Stat label="Countries" value=countries.to_string() />
            <Stat label="Covered" value=covered.to_string() />
            <Stat label="Coverage gaps" value=gaps.to_string() warn=true />
            <Stat label="Issuers" value=entities.to_string() />
            <Stat label="Filings" value=filings.to_string() />
        </div>

        <div class="table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th scope="col">"Country"</th>
                        <th scope="col" class="num">"Issuers"</th>
                        <th scope="col" class="num">"With filings"</th>
                        <th scope="col" class="num">"Filings"</th>
                        <th scope="col">"Volume"</th>
                        <th scope="col">"Status"</th>
                    </tr>
                </thead>
                <tbody>{body}</tbody>
            </table>
            <p class="caption">
                "Source: filings.xbrl.org ESEF index · a gap means the jurisdiction is not
                 discoverable in the public index (the company may still file under the mandate)."
            </p>
        </div>
    }
}
