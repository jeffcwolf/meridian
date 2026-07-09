use leptos::prelude::*;

/// A single headline statistic tile (used on the coverage and quality pages).
#[component]
pub fn Stat(label: &'static str, value: String, #[prop(optional)] warn: bool) -> impl IntoView {
    let class = if warn { "stat stat-warn" } else { "stat" };
    view! {
        <div class=class>
            <span class="stat-value">{value}</span>
            <span class="stat-label">{label}</span>
        </div>
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
                </nav>
            </div>
        </header>
    }
}
