# Meridian

*A cross-border ESEF filing explorer. One taxonomy, 27 countries, every listed company.*

Meridian is a search-and-browse interface over European listed-company filings
submitted under the ESEF (European Single Electronic Format) mandate. It pulls
data from the [filings.xbrl.org](https://filings.xbrl.org) API, parses the
XBRL-JSON extracts, and presents IFRS-tagged financial data so cross-country
comparison is trivial. See [`SPEC.md`](SPEC.md) for the full product spec.

## Architecture

```
filings.xbrl.org ─▶ Python scripts ─▶ data/meridian.db ─▶ Rust/Axum ─▶ Leptos
```

- **Python data scripts** (`scripts/`, uv-managed) fetch and parse data offline
  into a SQLite cache. See [`scripts/README.md`](scripts/README.md).
- **Rust web app** (Leptos SSR + hydration on Axum) reads that cache and renders
  the UI. It never calls external APIs at request time.
- **Custom CSS** in `style/main.css` — no frameworks.

## What's built so far

The first end-to-end slice of the [SPEC](SPEC.md) build sequence:

1. `fetch_filings.py` — match seed issuers to their filer in the filings.xbrl.org index → entity + filing metadata
2. `parse_xbrl_json.py` — XBRL-JSON extracts → headline IFRS facts
3. `fetch_fx_rates.py` — ECB annual reference rates → `fx_rates` (for currency conversion)
4. A Leptos + Axum app with three pages:
   - **Search** — every seeded company with country, filing count, and years
   - **Company detail** — IFRS financial highlights across years + a filing timeline
   - **Compare** — 2–5 companies side by side for a chosen year, convertible to a common currency (EUR/USD/GBP) at ECB rates

## Running it

Prerequisites: a recent Rust toolchain, the wasm target
(`rustup target add wasm32-unknown-unknown`), and [`uv`](https://docs.astral.sh/uv/).
Install **cargo-leptos 0.3+** (`cargo install cargo-leptos`) — the pinned Leptos
0.8 stack needs `wasm-bindgen 0.2.126`, which older cargo-leptos (0.2.x) does not
bundle.

```bash
# 1. Populate the data cache.
cd scripts
uv sync
uv run python src/fetch_filings.py      # real data (needs network) …
uv run python src/parse_xbrl_json.py    # … then parse financials …
uv run python src/fetch_fx_rates.py     # … then FX rates (optional)
# or, with no network:
uv run python src/seed_demo.py          # offline demo data
cd ..

# 2. Run the web app.
cargo leptos watch
# open http://127.0.0.1:3000
```

The app reads `data/meridian.db` (override with the `MERIDIAN_DB` env var).

## Layout

```
src/
├── main.rs          # Axum server entry point (ssr)
├── lib.rs           # module wiring + wasm hydrate entry
├── app.rs           # Leptos app root, shell, router
├── model.rs         # shared serializable data types
├── query.rs         # shared query-string parsing
├── export.rs        # CSV/JSON export handlers (ssr)
├── data/            # rusqlite reads: reads · compare · dashboards · format (ssr)
├── components/      # shared UI (header, stat tile)
└── pages/           # one module per page (each owns its server fn)
scripts/src/         # Python data pipeline
style/main.css       # hand-written CSS
data/                # SQLite cache (gitignored)
```

## Code standards

The code is written and reviewed against a three-layer methodology — structure
(Ousterhout's *A Philosophy of Software Design*), expression (*The Art of
Readable Code*), then Rust idiom. The standards, and how they're applied here,
are documented in [`docs/standards/`](docs/standards/README.md).
