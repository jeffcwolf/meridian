use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::Header;
use crate::pages::company::CompanyPage;
use crate::pages::compare::ComparePage;
use crate::pages::coverage::CoveragePage;
use crate::pages::quality::QualityPage;
use crate::pages::search::SearchPage;

/// The HTML document shell rendered on the server for every route.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
                <AutoReload options=options.clone() />
                <HydrationScripts options=options.clone() />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/meridian.css" />
        <Title text="Meridian — ESEF filing explorer" />

        <Router>
            <Header />
            <main class="container">
                <Routes fallback=|| view! { <NotFound /> }>
                    <Route path=path!("/") view=SearchPage />
                    <Route path=path!("/company/:id") view=CompanyPage />
                    <Route path=path!("/compare") view=ComparePage />
                    <Route path=path!("/coverage") view=CoveragePage />
                    <Route path=path!("/quality") view=QualityPage />
                </Routes>
            </main>
            <footer class="site-footer">
                <p>
                    "Data: "
                    <a href="https://filings.xbrl.org">"filings.xbrl.org"</a>
                    " ESEF filing index (entity, filing and XBRL-JSON data)"
                </p>
            </footer>
        </Router>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! {
        <div class="empty">
            <h2>"Page not found"</h2>
            <p><a href="/">"Back to search"</a></p>
        </div>
    }
}
