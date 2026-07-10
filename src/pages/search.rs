//! The search page: a company-name search over the cached universe, backed by
//! the [`search_companies`] server function.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_query_map;

use crate::model::CompanySummary;

/// Server function backing the search page. Runs the SQLite read on the server;
/// on the client it is a network call.
#[server(SearchCompanies)]
pub async fn search_companies(
    /// Free-text query; matches company name substrings. `None` lists everything.
    q: Option<String>,
) -> Result<Vec<CompanySummary>, ServerFnError> {
    let q = q.and_then(|s| {
        let t = s.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    });
    crate::data::list_companies(q.as_deref()).map_err(|e| ServerFnError::new(e.to_string()))
}

/// Search page: a text box plus the results table of matching companies.
#[component]
pub fn SearchPage() -> impl IntoView {
    let query = use_query_map();
    // Current text in the box (owned String, empty when absent).
    let current_q = move || query.with(|m| m.get("q").map(|s| s.to_string()).unwrap_or_default());
    // The active filter, or None when blank.
    let active_q = move || {
        let s = current_q();
        if s.trim().is_empty() {
            None
        } else {
            Some(s)
        }
    };

    // Blocking so results are rendered into the HTML on the server (works
    // without client hydration and is search-engine friendly).
    let companies = Resource::new_blocking(active_q, |q| async move { search_companies(q).await });

    view! {
        <Title text="Meridian — European ESEF filing explorer" />
        <section class="page-intro">
            <h1>"European ESEF filings, made comparable"</h1>
            <p class="lead">
                "ESEF tags every EU-listed annual report with the IFRS taxonomy so the numbers are
                 machine-readable across borders. Meridian pulls that data, parses it, and lets you
                 search, compare and explore it — the comparability ESEF
                  promised, delivered for a curated set of ~40 large-cap issuers across 13
                  countries. "
                <a href="/about">"How it works →"</a>
            </p>
            <form class="search-form" method="GET" action="/">
                <input
                    type="search"
                    name="q"
                    class="search-input"
                    placeholder="Company name, country code, or LEI…"
                    value=current_q
                    autocomplete="off"
                />
                <button type="submit" class="btn">"Search"</button>
            </form>
        </section>

        <nav class="feature-cards" aria-label="Features">
            <a class="feature-card" href="/compare">
                <span class="feature-title">"Compare"</span>
                <span class="feature-desc">
                    "2–5 companies side by side, in one currency at ECB rates"
                </span>
            </a>
            <a class="feature-card" href="/coverage">
                <span class="feature-title">"Coverage"</span>
                <span class="feature-desc">"Which countries the ESEF index covers — and doesn't"</span>
            </a>
            <a class="feature-card" href="/quality">
                <span class="feature-title">"Data quality"</span>
                <span class="feature-desc">"Validation messages by severity and country"</span>
            </a>
            <a class="feature-card" href="/extensions">
                <span class="feature-title">"Extensions"</span>
                <span class="feature-desc">"Where issuers depart from standard IFRS tags"</span>
            </a>
        </nav>

        <section class="page-intro why-built">
            <h2>"Why I built this"</h2>
            <p class="lead">
                "I've mostly worked with U.S. financial data before, where a large, unified system makes it easy to compare companies on the same footing, and I wanted to see how that holds up in the more fragmented, varied EU landscape. I also wanted to actually build something on the filings.xbrl.org API rather than just expose it. Because one core IFRS taxonomy runs across the EU, many concepts are standardised, so you should be able to compare financial statements meaningfully across borders, e.g, a Spanish IBEX company against a French CAC 40 one, while also surfacing data-quality and validation issues. Those ideas became the features you see in Meridian. Building it, and seeing that some jurisdictions (Germany, for one) aren't even present, was a concrete way for me to feel that fragmentation firsthand."
            </p>
        </section>

        
        <Suspense fallback=move || {
            view! { <p class="muted loading">"Loading companies…"</p> }
        }>
            {move || {
                companies
                    .get()
                    .map(|result| match result {
                        Ok(list) => view! { <CompanyTable companies=list /> }.into_any(),
                        Err(e) => {
                            view! { <p class="error">"Could not load companies: " {e.to_string()}</p> }
                                .into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn CompanyTable(companies: Vec<CompanySummary>) -> impl IntoView {
    if companies.is_empty() {
        return view! {
            <div class="empty">
                <p>"No companies match your search."</p>
            </div>
        }
        .into_any();
    }

    let count = companies.len();
    let empty_count = companies.iter().filter(|c| c.filing_count == 0).count();
    let rows = companies
        .into_iter()
        .map(|c| {
            let href = format!("/company/{}", c.id);
            let empty = c.filing_count == 0;
            let years_cell = if empty {
                view! { <span class="no-filings">"No discoverable filings"</span> }.into_any()
            } else {
                let years = match (c.first_year.as_deref(), c.last_year.as_deref()) {
                    (Some(a), Some(b)) if a == b => a.to_string(),
                    (Some(a), Some(b)) => format!("{a}–{b}"),
                    _ => "—".to_string(),
                };
                view! { {years} }.into_any()
            };
            let row_class = if empty { "row-empty" } else { "" };
            view! {
                <tr class=row_class>
                    <td class="col-name">
                        <a href=href>{c.name}</a>
                        <span class="lei">{c.lei.unwrap_or_default()}</span>
                    </td>
                    <td class="col-country">{c.country.unwrap_or_else(|| "—".into())}</td>
                    <td class="num">{c.filing_count}</td>
                    <td class="col-years">{years_cell}</td>
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="table-wrap">
            <table class="data-table">
                <thead>
                    <tr>
                        <th scope="col">"Company"</th>
                        <th scope="col">"Country"</th>
                        <th scope="col" class="num">"Filings"</th>
                        <th scope="col">"Years"</th>
                    </tr>
                </thead>
                <tbody>{rows}</tbody>
            </table>
            <p class="caption">
                {count}" companies · Source: filings.xbrl.org ESEF index"
                {(empty_count > 0)
                    .then(|| {
                        format!(
                            " · {empty_count} in jurisdictions the index does not cover (e.g. Germany)"
                        )
                    })}
            </p>
        </div>
    }
    .into_any()
}
