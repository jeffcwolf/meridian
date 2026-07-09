use leptos::prelude::*;
use leptos_meta::Title;

use crate::components::Stat;
use crate::model::ExtensionSummary;

/// Server function backing the extension-tag tracker.
#[server(ExtensionData)]
pub async fn extension_data() -> Result<ExtensionSummary, ServerFnError> {
    crate::data::extension_summary().map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn ExtensionsPage() -> impl IntoView {
    let data = Resource::new_blocking(|| (), |_| async { extension_data().await });

    view! {
        <Title text="Extension tags · Meridian" />
        <section class="page-intro">
            <h1>"Extension tags"</h1>
            <p class="muted">
                "When an issuer's reporting does not fit a standard IFRS element it defines a
                 company-specific extension. Tracking where and how often that happens shows where
                 the taxonomy falls short — and where comparability quietly erodes."
            </p>
        </section>

        <Suspense fallback=move || {
            view! { <p class="muted loading">"Loading…"</p> }
        }>
            {move || {
                data.get()
                    .map(|res| match res {
                        Ok(e) => view! { <ExtensionView e=e /> }.into_any(),
                        Err(e) => view! { <p class="error">{e.to_string()}</p> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

#[component]
fn ExtensionView(e: ExtensionSummary) -> impl IntoView {
    let ExtensionSummary {
        by_company,
        by_concept,
        companies_using,
        distinct_concepts,
        total_facts,
    } = e;

    if total_facts == 0 {
        return view! {
            <p class="muted">
                "No extension facts yet. Run "
                <code>"parse_xbrl_json.py"</code>
                " to populate them."
            </p>
        }
        .into_any();
    }

    let company_rows = by_company
        .into_iter()
        .map(|c| {
            let samples = c
                .samples
                .into_iter()
                .map(|s| view! { <code class="ext-tag">{s}</code> })
                .collect_view();
            view! {
                <tr>
                    <td class="col-name">
                        <a href=format!("/company/{}", c.id)>{c.name}</a>
                        <span class="lei">{c.country.unwrap_or_default()}</span>
                    </td>
                    <td class="num">{c.count}</td>
                    <td class="ext-samples">{samples}</td>
                </tr>
            }
        })
        .collect_view();

    let concept_rows = by_concept
        .into_iter()
        .map(|k| {
            view! {
                <tr>
                    <td><code class="ext-tag">{k.concept}</code></td>
                    <td>{k.prefix}</td>
                    <td class="num">{k.companies}</td>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="stat-grid">
            <Stat label="Issuers using extensions" value=companies_using.to_string() warn=true />
            <Stat label="Distinct extension tags" value=distinct_concepts.to_string() />
            <Stat label="Extension facts" value=total_facts.to_string() />
        </div>

        <h2>"By issuer"</h2>
        <div class="table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th scope="col">"Company"</th>
                        <th scope="col" class="num">"Extension tags"</th>
                        <th scope="col">"Examples"</th>
                    </tr>
                </thead>
                <tbody>{company_rows}</tbody>
            </table>
        </div>

        <h2>"Most common extensions"</h2>
        <div class="table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th scope="col">"Extension concept"</th>
                        <th scope="col">"Prefix"</th>
                        <th scope="col" class="num">"Issuers"</th>
                    </tr>
                </thead>
                <tbody>{concept_rows}</tbody>
            </table>
            <p class="caption">
                "Anchoring (which standard concept each extension maps to) lives in the extension
                 taxonomy, not the XBRL-JSON — a natural next enrichment."
            </p>
        </div>
    }
    .into_any()
}
