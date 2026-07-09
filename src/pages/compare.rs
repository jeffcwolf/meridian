use leptos::prelude::*;
use leptos_router::hooks::use_location;

use crate::model::{CompanySummary, CompareTable};
use crate::pages::search::search_companies;

/// Server function backing the comparator. Returns `None` for fewer than two
/// companies (nothing to compare yet).
#[server(CompareData)]
pub async fn compare_data(
    ids: Vec<i64>,
    fy: Option<String>,
) -> Result<Option<CompareTable>, ServerFnError> {
    if ids.len() < 2 {
        return Ok(None);
    }
    crate::data::compare(&ids, fy.as_deref())
        .map(Some)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Repeated `id=` params come in on the query string; parse them ourselves
/// (ParamsMap keeps only one value per key).
fn parse_ids(search: &str) -> Vec<i64> {
    search
        .trim_start_matches('?')
        .split('&')
        .filter_map(|kv| kv.strip_prefix("id="))
        .filter_map(|v| v.parse::<i64>().ok())
        .collect()
}

fn parse_fy(search: &str) -> Option<String> {
    search
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix("fy="))
        .map(|v| v.to_string())
        .filter(|s| !s.is_empty())
}

#[component]
pub fn ComparePage() -> impl IntoView {
    let location = use_location();
    let ids = move || parse_ids(&location.search.get());
    let fy = move || parse_fy(&location.search.get());

    let all = Resource::new_blocking(|| (), |_| async { search_companies(None).await });
    let table = Resource::new_blocking(
        move || (ids(), fy()),
        |(ids, fy)| async move { compare_data(ids, fy).await },
    );

    view! {
        <section class="page-intro">
            <h1>"Cross-country comparator"</h1>
            <p class="muted">
                "Because every ESEF filing uses the IFRS taxonomy, the same concept tags line
                 up across countries and languages. Pick 2–5 companies and a year to compare."
            </p>
        </section>

        <Suspense fallback=move || {
            view! { <p class="muted loading">"Loading…"</p> }
        }>
            {move || {
                let companies = match all.get() {
                    Some(Ok(c)) => c,
                    Some(Err(e)) => {
                        return view! { <p class="error">{e.to_string()}</p> }.into_any();
                    }
                    None => return ().into_any(),
                };
                let result = table.get().and_then(Result::ok).flatten();
                let years = result.as_ref().map(|t| t.years.clone()).unwrap_or_default();
                let current_fy = result.as_ref().map(|t| t.fy.clone());
                view! {
                    <CompareForm
                        companies=companies
                        selected=ids()
                        years=years
                        fy=current_fy
                    />
                    {result.map(|t| view! { <CompareTableView table=t /> })}
                    {(ids().len() < 2)
                        .then(|| {
                            view! {
                                <p class="muted">"Select at least two companies, then compare."</p>
                            }
                        })}
                }
                    .into_any()
            }}
        </Suspense>
    }
}

#[component]
fn CompareForm(
    companies: Vec<CompanySummary>,
    selected: Vec<i64>,
    years: Vec<String>,
    fy: Option<String>,
) -> impl IntoView {
    let checks = companies
        .into_iter()
        .filter(|c| c.filing_count > 0)
        .map(|c| {
            let checked = selected.contains(&c.id);
            let country = c.country.unwrap_or_default();
            view! {
                <label class="check">
                    <input type="checkbox" name="id" value=c.id.to_string() checked=checked />
                    <span>{c.name}</span>
                    <span class="muted">{country}</span>
                </label>
            }
        })
        .collect_view();

    let year_select = (!years.is_empty()).then(|| {
        let options = years
            .into_iter()
            .map(|y| {
                let is_sel = fy.as_deref() == Some(y.as_str());
                view! { <option value=y.clone() selected=is_sel>{y.clone()}</option> }
            })
            .collect_view();
        view! {
            <label class="year-picker">
                "Fiscal year "
                <select name="fy">{options}</select>
            </label>
        }
    });

    view! {
        <form class="compare-form" method="GET" action="/compare">
            <div class="checks">{checks}</div>
            <div class="compare-controls">
                {year_select}
                <button class="btn" type="submit">"Compare"</button>
            </div>
        </form>
    }
}

#[component]
fn CompareTableView(table: CompareTable) -> impl IntoView {
    let CompareTable {
        fy,
        labels,
        columns,
        ..
    } = table;

    let headers = columns
        .iter()
        .map(|c| {
            let sub = format!(
                "{} · {}",
                c.country.clone().unwrap_or_else(|| "—".into()),
                c.currency.clone().unwrap_or_else(|| "—".into())
            );
            view! {
                <th scope="col" class="num">
                    <a href=format!("/company/{}", c.id)>{c.name.clone()}</a>
                    <span class="cmp-sub">{sub}</span>
                </th>
            }
        })
        .collect_view();

    let body = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let cells = columns
                .iter()
                .map(|c| {
                    let v = c
                        .cells
                        .get(i)
                        .cloned()
                        .flatten()
                        .unwrap_or_else(|| "—".into());
                    view! { <td class="num">{v}</td> }
                })
                .collect_view();
            view! {
                <tr>
                    <th scope="row">{label.clone()}</th>
                    {cells}
                </tr>
            }
        })
        .collect_view();

    view! {
        <h2>"Comparison — fiscal year " {fy}</h2>
        <div class="table-wrap">
            <table class="data-table compare-table">
                <thead>
                    <tr>
                        <th scope="col">"Concept"</th>
                        {headers}
                    </tr>
                </thead>
                <tbody>{body}</tbody>
            </table>
            <p class="caption">
                "Native currency, figures in millions. Cross-currency FX conversion to a common
                 base (via ECB reference rates) is a planned next step."
            </p>
        </div>
    }
}
