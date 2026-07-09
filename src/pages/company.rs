use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;

use crate::model::CompanyDetail;

/// Server function backing the company detail page.
#[server(FetchCompany)]
pub async fn company_detail(id: i64) -> Result<Option<CompanyDetail>, ServerFnError> {
    crate::data::get_company(id).map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn CompanyPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || {
        params
            .get()
            .get("id")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(-1)
    };

    let detail = Resource::new_blocking(id, |id| async move { company_detail(id).await });

    view! {
        <Suspense fallback=move || {
            view! { <p class="muted loading">"Loading company…"</p> }
        }>
            {move || {
                detail
                    .get()
                    .map(|result| match result {
                        Ok(Some(company)) => view! { <CompanyView company=company /> }.into_any(),
                        Ok(None) => {
                            view! {
                                <div class="empty">
                                    <h2>"Company not found"</h2>
                                    <p><a href="/">"Back to search"</a></p>
                                </div>
                            }
                                .into_any()
                        }
                        Err(e) => {
                            view! { <p class="error">"Could not load company: " {e.to_string()}</p> }
                                .into_any()
                        }
                    })
            }}
        </Suspense>
    }
}

#[component]
fn CompanyView(company: CompanyDetail) -> impl IntoView {
    let CompanyDetail {
        id,
        name,
        country,
        lei,
        currency,
        years,
        rows,
        filings,
        ..
    } = company;

    let page_title = format!("{name} · Meridian");
    let ccy = currency.clone().unwrap_or_else(|| "—".into());
    let has_filings = !filings.is_empty();
    let has_financials =
        !years.is_empty() && rows.iter().any(|r| r.cells.iter().any(Option::is_some));

    let year_headers = years
        .iter()
        .map(|y| view! { <th scope="col" class="num">{y.clone()}</th> })
        .collect_view();

    let concept_rows = rows
        .into_iter()
        .map(|row| {
            let cells = row
                .cells
                .into_iter()
                .map(|cell| {
                    view! { <td class="num">{cell.unwrap_or_else(|| "—".into())}</td> }
                })
                .collect_view();
            view! {
                <tr>
                    <th scope="row" class="concept">
                        <span class="concept-label">{row.label}</span>
                        <code class="concept-tag">{row.concept}</code>
                    </th>
                    {cells}
                </tr>
            }
        })
        .collect_view();

    let timeline = filings
        .into_iter()
        .map(|f| {
            let date = f.reporting_date.unwrap_or_else(|| "Unknown date".into());
            let country = f.country.unwrap_or_default();
            let report = f.filing_url.map(|url| {
                view! { <a class="link" href=url target="_blank" rel="noopener">"iXBRL report"</a> }
            });
            let json = f.xbrl_json_url.map(|url| {
                view! { <a class="link" href=url target="_blank" rel="noopener">"XBRL-JSON"</a> }
            });
            let count = f.validation_message_count;
            let badge_class = if count == 0 {
                "badge badge-ok"
            } else {
                "badge badge-warn"
            };
            view! {
                <li class="filing">
                    <span class="filing-date">{date}</span>
                    <span class="filing-country">{country}</span>
                    <span class="filing-links">{report} {json}</span>
                    <span class=badge_class>{count}" validation messages"</span>
                </li>
            }
        })
        .collect_view();

    view! {
        <article class="company">
            <Title text=page_title />
            <header class="company-head">
                <h1>{name}</h1>
                <div class="company-meta">
                    <span class="chip">{country.unwrap_or_else(|| "—".into())}</span>
                    <code class="lei">{lei.unwrap_or_else(|| "LEI unknown".into())}</code>
                    <span class="export-links">
                        "Export "
                        <a href=format!("/export/company/{id}/csv")>"CSV"</a>
                        <a href=format!("/export/company/{id}/json")>"JSON"</a>
                    </span>
                </div>
            </header>

            {if !has_filings {
                view! {
                    <section class="coverage-gap">
                        <h2>"No discoverable filings"</h2>
                        <p>
                            "filings.xbrl.org has no ESEF filings indexed for this issuer. "
                            "Several jurisdictions — most notably Germany — do not publish "
                            "their ESEF filings to the public index, so no financial data is "
                            "available here even though the company files under the mandate. "
                            "Surfacing that gap is itself part of what Meridian is for."
                        </p>
                    </section>
                }
                    .into_any()
            } else {
                view! {
                    <section class="financials">
                        <h2>"IFRS financial highlights"</h2>
                        {if has_financials {
                            view! {
                                <div class="table-wrap">
                                    <table class="data-table financials-table">
                                        <thead>
                                            <tr>
                                                <th scope="col">"Concept"</th>
                                                {year_headers}
                                            </tr>
                                        </thead>
                                        <tbody>{concept_rows}</tbody>
                                    </table>
                                    <p class="caption">
                                        "Figures in " {ccy}
                                        " millions · Source: filings.xbrl.org XBRL-JSON extracts"
                                    </p>
                                </div>
                            }
                                .into_any()
                        } else {
                            view! {
                                <p class="muted">
                                    "No parsed financial facts yet. Run "
                                    <code>"parse_xbrl_json.py"</code>
                                    " to populate them."
                                </p>
                            }
                                .into_any()
                        }}
                    </section>
                    <section class="timeline-section">
                        <h2>"Filing timeline"</h2>
                        <ul class="timeline">{timeline}</ul>
                    </section>
                }
                    .into_any()
            }}
        </article>
    }
}
