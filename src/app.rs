use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::Header;
use crate::pages::about::AboutPage;
use crate::pages::company::CompanyPage;
use crate::pages::compare::ComparePage;
use crate::pages::coverage::CoveragePage;
use crate::pages::extensions::ExtensionsPage;
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
                    <Route path=path!("/about") view=AboutPage />
                    <Route path=path!("/company/:id") view=CompanyPage />
                    <Route path=path!("/compare") view=ComparePage />
                    <Route path=path!("/coverage") view=CoveragePage />
                    <Route path=path!("/quality") view=QualityPage />
                    <Route path=path!("/extensions") view=ExtensionsPage />
                </Routes>
            </main>
            <footer class="site-footer">
                <div class="footer-inner">
                    <p class="footer-source">
                        "Data: "
                        <a href="https://filings.xbrl.org">"filings.xbrl.org"</a>
                        " ESEF index · ECB reference rates · "
                        <a href="/about">"About this project"</a>
                    </p>
                    <div class="creator">
                        <span class="creator-name">"Created by Jeffrey C. Wolf"</span>
                        <a
                            class="contact"
                            href="mailto:publicemailalias.kite252@passmail.net"
                            aria-label="Email the creator"
                        >
                            <span class="mail-icon" aria-hidden="true">"✉"</span>
                            <span>"Contact me"</span>
                        </a>
                    </div>
                </div>
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
