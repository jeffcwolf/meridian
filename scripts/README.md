# Meridian data pipeline

Python scripts (uv-managed) that populate `../data/meridian.db`, the SQLite
cache the Rust/Leptos app reads. They fetch and parse ESEF filings from public
sources and run offline / on a schedule — never at request time.

```
filings.xbrl.org ─┐
GLEIF ────────────┼─▶ Python scripts ─▶ data/meridian.db ─▶ Rust/Axum ─▶ Leptos
```

## Setup

```bash
cd scripts
uv sync
```

## Scripts

| Script                | What it does                                                                 | Network |
|-----------------------|------------------------------------------------------------------------------|---------|
| `src/db.py`           | Schema + upsert helpers (shared). `uv run python src/db.py` just inits it.    | none    |
| `src/fetch_filings.py`| Resolve a seed list of issuers to LEIs via GLEIF, then pull their filing metadata from filings.xbrl.org into `entities` + `filings`. | GLEIF, filings.xbrl.org |
| `src/parse_xbrl_json.py`| Download each filing's XBRL-JSON extract and parse headline IFRS concepts into `financial_facts`. | filings.xbrl.org |
| `src/seed_demo.py`    | Offline fixture: writes illustrative demo data so the web app can be run without any network access. | none |

## Usage

Real data (needs outbound access to `api.gleif.org` and `filings.xbrl.org`):

```bash
uv run python src/fetch_filings.py      # entities + filings
uv run python src/parse_xbrl_json.py    # financial_facts
```

Offline demo data (no network — for previewing the UI):

```bash
uv run python src/seed_demo.py
```

Both paths are idempotent; re-running refreshes the same rows.

## Schema (`data/meridian.db`)

- **entities** — `id, name, lei (unique), country`
- **filings** — `id, entity_id, reporting_date, filing_url, xbrl_json_url, country, validation_message_count`
- **financial_facts** — `id, entity_id, reporting_date, concept, value, currency`

Conventions: amounts stored as strings (no float rounding), IFRS concepts kept
as-is (`ifrs-full:Revenue`), currencies as ISO-4217 codes (`EUR`).

## Seed list

`fetch_filings.py` seeds 10 well-known issuers across 7 countries (DE, FR, ES,
DK, FI, IT, NL). LEIs are resolved from name + country at runtime; pin exact
LEIs in `LEI_OVERRIDES` if GLEIF full-text picks the wrong entity, and edit the
`SEED` list to change the universe.

## Parsed concepts

`ifrs-full:Revenue`, `ifrs-full:ProfitLoss`, `ifrs-full:Assets`,
`ifrs-full:Equity`, `ifrs-full:CashFlowsFromOperatingActivities`.

For each concept the parser keeps the current-year, top-line consolidated value:
the fact carrying only core xBRL-JSON dimensions (no segment/member axes) with
the latest period end.
