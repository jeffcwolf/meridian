# CLAUDE.md — Meridian

## Project Overview

Meridian is a cross-border ESEF filing explorer that lets investors search, browse, and compare European listed company filings across jurisdictions. See `SPEC.md` for the full product specification.

## Architecture

Hybrid Rust + Python.

**Rust web application (Leptos + Axum):**
- Leptos for the frontend (SSR + client-side hydration)
- Axum for the backend API and static file serving
- Custom CSS — no Tailwind, no CSS frameworks. All styles hand-written in `style/`
- The Rust app reads from cached data in `data/`. It does not call external APIs directly at runtime

**Python data scripts (uv-managed, in `pipeline/`):**
- Fetch, parse, and cache data from external sources
- Output to `data/` as SQLite databases or JSON files
- Run offline / on a schedule, not at request time
- Managed by uv — dependencies declared in `pipeline/pyproject.toml`

**Data flow:**
```
External APIs → Python scripts → data/ (SQLite/JSON) → Rust/Axum reads → Leptos renders
```

## Tech Stack

### Rust
- **Leptos** — reactive frontend framework (SSR mode with Axum integration)
- **Axum** — HTTP server, API routes, static file serving
- **sqlx** or **rusqlite** — reading from SQLite caches
- **serde** / **serde_json** — serialisation
- **tokio** — async runtime

### Python (in `pipeline/`)
- **xbrl-filings-api** — Python wrapper for filings.xbrl.org API
- **requests** or **httpx** — HTTP calls to GLEIF, ECB (for FX rates)
- **sqlite3** or **sqlalchemy** — writing to SQLite cache
- Managed by **uv**. Run scripts with `cd pipeline && uv run python <script>.py`

### CSS
- Custom hand-written CSS in `style/main.css`
- No frameworks, no preprocessors unless explicitly added later
- Clean, table-heavy design appropriate for financial data

## Project Structure

```
meridian/
├── Cargo.toml
├── CLAUDE.md
├── SPEC.md
├── README.md
├── src/
│   ├── main.rs              # Axum server entry point
│   ├── app.rs               # Leptos app root component
│   ├── components/          # Leptos UI components
│   ├── pages/               # Page-level components (search, company, comparator)
│   └── data/                # Rust modules for reading cached data
├── style/
│   └── main.css
├── pipeline/
│   ├── pyproject.toml
│   ├── .python-version
│   └── src/
│       ├── fetch_filings.py     # filings.xbrl.org → SQLite
│       ├── fetch_entities.py    # GLEIF LEI lookup → SQLite
│       ├── fetch_fx_rates.py    # ECB FX rates → SQLite
│       └── parse_xbrl_json.py   # Parse XBRL-JSON extracts into structured tables
└── data/
    └── .gitkeep
```

## Key External APIs

| API | Base URL | Auth | Used by |
|-----|----------|------|---------|
| filings.xbrl.org | `https://filings.xbrl.org/api/filings` | None (public, rate-limited) | `fetch_filings.py` |
| GLEIF LEI lookup | `https://api.gleif.org/api/v1/lei-records` | None (public) | `fetch_entities.py` |
| ECB Data Portal (FX rates) | `https://data-api.ecb.europa.eu` | None (public) | `fetch_fx_rates.py` |

## Conventions

- Rust code follows standard `rustfmt` formatting
- Python scripts follow PEP 8
- Component filenames in snake_case
- All financial amounts stored as integers in minor units (cents) or as strings to avoid floating-point issues
- IFRS concept tags stored as-is from the taxonomy (e.g., `ifrs-full:Revenue`)
- Every data table in the UI shows its source and coverage metadata

## Running the Project

```bash
# Fetch/update data cache
cd pipeline && uv run python src/fetch_filings.py && cd ..

# Run the web app
cargo leptos watch
```

## Build Priority

Refer to SPEC.md Build Sequence. Start with the Python data scripts (they populate the cache the Rust app needs), then build the Rust/Leptos UI on top.
