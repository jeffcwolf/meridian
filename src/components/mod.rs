//! Small reusable UI components shared across pages: the header, the
//! headline-statistic tile, and a `resourced` loading/error wrapper.

use leptos::prelude::*;

/// A single headline statistic tile (used on the coverage and quality pages).
#[component]
pub fn Stat(
    /// Caption shown beneath the figure.
    label: &'static str,
    /// Pre-formatted figure to display.
    value: String,
    /// Render in the warning style (used for non-zero error counts).
    #[prop(optional)]
    warn: bool,
) -> impl IntoView {
    let class = if warn { "stat stat-warn" } else { "stat" };
    view! {
        <div class=class>
            <span class="stat-value">{value}</span>
            <span class="stat-label">{label}</span>
        </div>
    }
}

/// Render a blocking [`Resource`] with the shared loading and error chrome: a
/// placeholder carrying `loading` while the request is in flight, and a styled
/// error paragraph if the server function fails. `success` renders the resolved
/// value. Pages whose fetch has a distinct shape (an `Option` payload, a "not
/// found" case) render their own `Suspense` instead.
pub(crate) fn resourced<T, IV, F>(
    resource: Resource<Result<T, ServerFnError>>,
    loading: &'static str,
    success: F,
) -> impl IntoView
where
    T: Clone + Send + Sync + 'static,
    IV: IntoView + 'static,
    F: Fn(T) -> IV + Copy + Send + Sync + 'static,
{
    view! {
        <Suspense fallback=move || view! { <p class="muted loading">{loading}</p> }>
            {move || {
                resource
                    .get()
                    .map(move |result| match result {
                        Ok(value) => success(value).into_any(),
                        Err(e) => view! { <p class="error">{e.to_string()}</p> }.into_any(),
                    })
            }}
        </Suspense>
    }
}

/// Top navigation bar shown on every page.
#[component]
pub fn Header() -> impl IntoView {
    view! {
        <header class="site-header">
            <div class="container header-inner">
                <a class="brand" href="/">
                    <span class="brand-mark">"◆"</span>
                    <span class="brand-name">"Meridian"</span>
                    <span class="brand-tag">"ESEF filing explorer"</span>
                </a>
                <nav class="site-nav">
                    <a href="/">"Search"</a>
                    <a href="/compare">"Compare"</a>
                    <a href="/coverage">"Coverage"</a>
                    <a href="/quality">"Quality"</a>
                    <a href="/extensions">"Extensions"</a>
                    <a href="/about">"About"</a>
                </nav>
            </div>
        </header>
    }
}
