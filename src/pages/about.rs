//! The About page: a portfolio narrative explaining what Meridian is, why it
//! exists, and how it is built.

use leptos::prelude::*;
use leptos_meta::Title;

/// The portfolio narrative: what Meridian is, why it exists, how it is built,
/// and what it demonstrates. Written for a first-time visitor (e.g. a recruiter).
#[component]
pub fn AboutPage() -> impl IntoView {
    view! {
        <Title text="About · Meridian" />
        <article class="prose">
            <section class="page-intro">
                <h1>"About Meridian"</h1>
                <p class="lead">
                    "A cross-border ESEF filing explorer. One taxonomy, many countries, real
                     financial data — built to make European listed-company filings genuinely
                     comparable across jurisdictions."
                </p>
            </section>

            <h2>"The problem"</h2>
            <p>
                "Since 2020, EU-listed companies must publish their annual reports in ESEF — the
                 European Single Electronic Format — tagging the numbers with the IFRS taxonomy so
                 they are machine-readable and, in principle, comparable across borders and
                 languages. In practice that promise is unrealised: the data is fragmented across
                 national mechanisms, tagged inconsistently, and no investor-facing tool makes it
                 easy to line companies up side by side. Meridian is a working demonstration of what
                 that tool looks like."
            </p>

            <h2>"What you can do here"</h2>
            <ul class="feature-list">
                <li>
                    <a href="/">"Search"</a>
                    " issuers by name, country or LEI, and open any one to see its IFRS financial
                     highlights across years plus its full filing timeline."
                </li>
                <li>
                    <a href="/compare">"Compare"</a>
                    " 2–5 companies from different countries side by side for a chosen fiscal year,
                     converted to a common currency at ECB reference rates — the feature ESEF was
                     meant to enable."
                </li>
                <li>
                    <a href="/coverage">"Coverage"</a>
                    " maps which jurisdictions the public index actually covers, and quantifies the
                     gaps (Germany, notably, does not publish to it) — where comparability breaks."
                </li>
                <li>
                    <a href="/quality">"Data quality"</a>
                    " aggregates the validation messages run on every filing, by severity and
                     country, as a proxy for filing quality."
                </li>
                <li>
                    <a href="/extensions">"Extension tags"</a>
                    " tracks where issuers depart from standard IFRS with company-specific tags — a
                     signal for where the taxonomy falls short."
                </li>
                <li>
                    "Any table can be "
                    <strong>"exported"</strong>
                    " to CSV or JSON, with concept tags, currencies and source filing URLs."
                </li>
            </ul>

            <h2>"How it's built"</h2>
            <p>
                "A hybrid Rust + Python system with a clean separation between data preparation and
                 serving:"
            </p>
            <div class="pipeline">
                "filings.xbrl.org → Python (uv) → SQLite → Rust / Axum → Leptos"
            </div>
            <ul>
                <li>
                    <strong>"Data pipeline (Python, uv-managed)."</strong>
                    " Resolves a curated universe of issuers against the actual filer set on
                     filings.xbrl.org, parses the XBRL-JSON extracts into headline IFRS facts
                     (handling fiscal-year alignment, concept aliases, bank-specific lines and
                     custom extensions), and pulls ECB annual FX rates — all cached to SQLite."
                </li>
                <li>
                    <strong>"Web application (Rust)."</strong>
                    " Leptos for the UI (server-side rendered with client hydration) on an Axum
                     server, reading the SQLite cache via rusqlite. Hand-written CSS, no frameworks.
                     The app never calls external APIs at request time — it serves the cache."
                </li>
            </ul>

            <h2>"Data sources"</h2>
            <ul>
                <li>
                    <a href="https://filings.xbrl.org" target="_blank" rel="noopener">
                        "filings.xbrl.org"
                    </a> " — the ESEF filing index and machine-readable XBRL-JSON extracts."
                </li>
                <li>
                    <a href="https://data.ecb.europa.eu" target="_blank" rel="noopener">
                        "European Central Bank"
                    </a> " — annual reference exchange rates for currency conversion."
                </li>
            </ul>

            <h2>"What it demonstrates"</h2>
            <ul>
                <li>"Fluency with the IFRS taxonomy and the ESEF mandate — not just US-GAAP."</li>
                <li>"Cross-jurisdictional data handling: currencies, fiscal calendars, sectors."</li>
                <li>"Practical understanding of XBRL-JSON, LEIs and validation semantics."</li>
                <li>"Data-quality and coverage awareness applied to real European filings."</li>
                <li>"Full-stack delivery in Rust (Leptos + Axum) and Python, end to end."</li>
            </ul>

            <p class="note">
                "The seeded universe is a curated set of large-cap issuers — enough to exercise
                 every feature against real filings; the pipeline extends to any issuer in the
                 index. Figures are drawn directly from companies' own ESEF submissions."
            </p>
        </article>
    }
}
