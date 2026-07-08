# Meridian — Product Spec

*A cross-border ESEF filing explorer. One taxonomy, 27 countries, every listed company.*

---

## What Meridian Is

Meridian is a search-and-browse interface over European listed company filings submitted under the ESEF (European Single Electronic Format) mandate. It pulls data from the filings.xbrl.org API, parses the XBRL-JSON extracts, and presents IFRS-tagged financial data in a way that makes cross-country comparison trivial.

The core proposition: ESEF was designed to make European financial statements comparable across borders, but no investor-facing tool actually delivers on that promise. Meridian does.

---

## Who It's For

- Analysts at EU-focused funds who need to compare companies across jurisdictions without paying for a Bloomberg terminal.
- Anyone interviewing at or working for European financial institutions (ECB, BaFin, ESMA, national central banks) who wants to demonstrate fluency with ESEF data.
- Researchers studying cross-border financial reporting quality under the ESEF mandate.

---

## Data Source

**Primary:** filings.xbrl.org JSON API (`https://filings.xbrl.org/api/filings`)

- JSON-API standard with filtering, sorting, pagination
- Entity metadata (name, LEI, country)
- Filing metadata (reporting date, filing system, country, links)
- XBRL-JSON extracts (structured financial data per filing)
- Validation messages (data quality checks run by XBRL International)

**Python wrapper:** `xbrl-filings-api` (PyPI) with SQLite integration.

**Supplementary:** GLEIF API for LEI resolution and entity lookup.

---

## Features

### 1. Company Search

Full-text search by company name, LEI, or country code. Results show entity name, country, number of available filings, and years covered. Autocomplete where practical.

### 2. Filing Timeline

For a selected company, display all available ESEF filings in chronological order. Each filing card shows:

- Reporting date
- Filing country and OAM (Officially Appointed Mechanism)
- Link to original iXBRL report
- Link to XBRL-JSON extract
- Count of validation messages (errors/warnings)

### 3. Financial Statement Viewer

Parse the XBRL-JSON for a selected filing and render the three primary IFRS financial statements:

- Statement of Financial Position (balance sheet)
- Statement of Profit or Loss and Other Comprehensive Income
- Statement of Cash Flows

Display using canonical IFRS taxonomy labels. Each line item shows the IFRS concept tag alongside the human-readable label. Amounts formatted with appropriate currency and scale.

### 4. Cross-Country Comparator

The signature feature. Select 2–5 companies from different EU countries. Choose a reporting year. Meridian pulls the XBRL-JSON for each and aligns the IFRS concepts into a side-by-side comparison table.

Because all ESEF filings use the IFRS taxonomy, the same concept tags (e.g., `ifrs-full:Revenue`, `ifrs-full:Assets`) appear across filings regardless of country or language. Meridian exploits this to enable comparison that would otherwise require manual extraction.

Columns: one per company. Rows: IFRS line items. Currency conversion to a common base (EUR) using ECB reference rates where needed.

### 5. Data Quality Dashboard

Surface the validation messages from filings.xbrl.org for any filing. Categorise by severity (error, warning, inconsistency). Show:

- Calculation inconsistencies (XBRL calc linkbase failures)
- Missing mandatory tags
- Extension tag usage (non-standard IFRS extensions)
- Filing-level quality summary

Aggregate across companies to show which countries or sectors have higher/lower data quality.

### 6. Country Coverage Map

Visual map of the EU showing which countries have filings available in the filings.xbrl.org index, how many entities per country, and coverage gaps. The filings.xbrl.org documentation notes that several countries do not make ESEF filings discoverable — surfacing this gap is itself informative.

### 7. Extension Tag Tracker

When companies use custom XBRL extensions rather than standard IFRS taxonomy elements, flag these visibly. Show what they anchored the extension to in the taxonomy. Track extension usage across companies to identify where the IFRS taxonomy falls short.

### 8. Export

Export any comparison table or financial statement view to CSV or JSON. Include IFRS concept tags, values, currencies, and source filing URLs in the export.

---

## Technical Stack

- **Frontend:** Leptos (Rust reactive framework, SSR + hydration)
- **Backend:** Axum (HTTP server, API routes)
- **Styling:** Custom hand-written CSS — no frameworks
- **Data layer:** SQLite (populated by Python scripts), read via rusqlite or sqlx
- **Data scripts:** Python (uv-managed), using `xbrl-filings-api` for filings.xbrl.org, `ecbdata` for ECB FX rates, `requests` for GLEIF
- **APIs consumed:** filings.xbrl.org (primary), ECB Data Portal (FX rates), GLEIF (LEI lookup)
- **No authentication required** for any API (all public, rate-limited)

---

## What It Demonstrates (Portfolio Value)

- IFRS taxonomy knowledge (not just US-GAAP)
- Cross-jurisdictional data handling
- Practical understanding of how ESEF works technically
- Data quality awareness applied to European filings
- Ability to work with JSON-API standard and XBRL-JSON format
- Direct relevance to ESMA, ECB, BaFin, and national regulator mandates

---

## Build Sequence

1. API integration: connect to filings.xbrl.org, retrieve and cache filing metadata
2. Entity search and filing timeline views
3. XBRL-JSON parser for IFRS financial statements
4. Single-company financial statement viewer
5. Cross-country comparator (core feature)
6. Validation message / data quality dashboard
7. Country coverage map
8. Export functionality
