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
    base: Option<String>,
) -> Result<Option<CompareTable>, ServerFnError> {
    if ids.len() < 2 {
        return Ok(None);
    }
    crate::data::compare(&ids, fy.as_deref(), base.as_deref())
        .map(Some)
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Currency options offered in the comparator's base-currency picker.
const BASE_OPTIONS: [(&str, &str); 4] = [
    ("", "Native"),
    ("EUR", "EUR (€)"),
    ("USD", "USD ($)"),
    ("GBP", "GBP (£)"),
];

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

fn parse_param(search: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    search
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix(&prefix))
        .map(|v| v.to_string())
        .filter(|s| !s.is_empty())
}

#[component]
pub fn ComparePage() -> impl IntoView {
    let location = use_location();
    let ids = move || parse_ids(&location.search.get());
    let fy = move || parse_param(&location.search.get(), "fy");
    let base = move || parse_param(&location.search.get(), "base");
    let search = move || location.search.get().trim_start_matches('?').to_string();

    let all = Resource::new_blocking(|| (), |_| async { search_companies(None).await });
    let table = Resource::new_blocking(
        move || (ids(), fy(), base()),
        |(ids, fy, base)| async move { compare_data(ids, fy, base).await },
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
                let export_links = result.as_ref().map(|_| {
                    let qs = search();
                    view! {
                        <p class="export-links">
                            "Export comparison "
                            <a href=format!("/export/compare/csv?{qs}")>"CSV"</a>
                            <a href=format!("/export/compare/json?{qs}")>"JSON"</a>
                        </p>
                    }
                });
                view! {
                    <CompareForm
                        companies=companies
                        selected=ids()
                        years=years
                        fy=current_fy
                        base=base()
                    />
                    {export_links}
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
    base: Option<String>,
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

    let has_years = !years.is_empty();
    let year_select = has_years.then(|| {
        let options = years
            .into_iter()
            .map(|y| {
                let is_sel = fy.as_deref() == Some(y.as_str());
                view! { <option value=y.clone() selected=is_sel>{y.clone()}</option> }
            })
            .collect_view();
        view! {
            <label class="picker">
                "Fiscal year "
                <select name="fy">{options}</select>
            </label>
        }
    });

    let base_up = base.map(|b| b.to_uppercase());
    let currency_select = has_years.then(|| {
        let options = BASE_OPTIONS
            .iter()
            .map(|(code, label)| {
                let is_sel = base_up.as_deref().unwrap_or("") == *code;
                view! { <option value=*code selected=is_sel>{*label}</option> }
            })
            .collect_view();
        view! {
            <label class="picker">
                "Currency "
                <select name="base">{options}</select>
            </label>
        }
    });

    view! {
        <form class="compare-form" method="GET" action="/compare">
            <div class="checks">{checks}</div>
            <div class="compare-controls">
                {year_select}
                {currency_select}
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
        base,
        ..
    } = table;

    let caption = match &base {
        Some(b) => format!("Converted to {b} at ECB annual-average rates · figures in millions"),
        None => "Native currency · figures in millions".to_string(),
    };

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
            <p class="caption">{caption}</p>
        </div>
    }
}
