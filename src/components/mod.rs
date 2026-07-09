use leptos::prelude::*;

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
                </nav>
            </div>
        </header>
    }
}
