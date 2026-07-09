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
                            class="repo-link"
                            href="https://github.com/jeffcwolf/meridian"
                            aria-label="View source on GitHub"
                        >
                            <svg
                                class="repo-icon"
                                viewBox="0 0 16 16"
                                width="16"
                                height="16"
                                aria-hidden="true"
                            >
                                <path
                                    fill="currentColor"
                                    d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A8.013 8.013 0 0 1 0 8c0-4.42 3.58-8 8-8Z"
                                />
                            </svg>
                            <span>"View source"</span>
                        </a>
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
