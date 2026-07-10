"""Shared SQLite helpers for the Meridian data pipeline.

The Rust/Leptos app reads the database produced here. All three tables are
created by :func:`init_db`, which is safe to call repeatedly (idempotent).

Conventions (see CLAUDE.md):
- Financial amounts are stored as strings to avoid floating-point issues.
- IFRS concept tags are stored as-is from the taxonomy (e.g. ``ifrs-full:Revenue``).
- Currencies are stored as ISO 4217 codes (e.g. ``EUR``).
"""

from __future__ import annotations

import sqlite3
from pathlib import Path

# pipeline/src/db.py -> pipeline/src -> pipeline -> repo root -> data/meridian.db
REPO_ROOT = Path(__file__).resolve().parent.parent.parent
DATA_DIR = REPO_ROOT / "data"
DB_PATH = DATA_DIR / "meridian.db"

SCHEMA = """
CREATE TABLE IF NOT EXISTS entities (
    id      INTEGER PRIMARY KEY,
    name    TEXT NOT NULL,
    lei     TEXT UNIQUE,
    country TEXT
);

CREATE TABLE IF NOT EXISTS filings (
    id                       INTEGER PRIMARY KEY,
    entity_id                INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    reporting_date           TEXT,
    filing_url               TEXT,
    xbrl_json_url            TEXT,
    country                  TEXT,
    validation_message_count INTEGER DEFAULT 0,
    error_count              INTEGER DEFAULT 0,
    warning_count            INTEGER DEFAULT 0,
    inconsistency_count      INTEGER DEFAULT 0,
    UNIQUE(entity_id, reporting_date)
);

CREATE TABLE IF NOT EXISTS financial_facts (
    id             INTEGER PRIMARY KEY,
    entity_id      INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    reporting_date TEXT,
    concept        TEXT NOT NULL,
    value          TEXT,
    currency       TEXT,
    UNIQUE(entity_id, reporting_date, concept)
);

CREATE TABLE IF NOT EXISTS extension_facts (
    id             INTEGER PRIMARY KEY,
    entity_id      INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    reporting_date TEXT,
    concept        TEXT NOT NULL,   -- company-specific extension tag, e.g. "ext:PatrimonioNetto"
    prefix         TEXT,            -- the extension namespace prefix, e.g. "ext"
    value          TEXT,
    currency       TEXT,
    UNIQUE(entity_id, reporting_date, concept)
);

CREATE TABLE IF NOT EXISTS fx_rates (
    currency     TEXT NOT NULL,
    year         TEXT NOT NULL,
    rate_per_eur REAL NOT NULL,   -- units of `currency` per 1 EUR (ECB annual average)
    PRIMARY KEY (currency, year)
);

CREATE INDEX IF NOT EXISTS idx_filings_entity ON filings(entity_id);
CREATE INDEX IF NOT EXISTS idx_facts_entity ON financial_facts(entity_id);
"""


def connect() -> sqlite3.Connection:
    """Open (creating the data directory if needed) the Meridian database."""
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(DB_PATH)
    conn.execute("PRAGMA foreign_keys = ON")
    conn.row_factory = sqlite3.Row
    return conn


def init_db(conn: sqlite3.Connection) -> None:
    """Create all tables and indexes if they do not already exist."""
    conn.executescript(SCHEMA)
    # Lightweight migration: add severity columns to a pre-existing filings table.
    existing = {row["name"] for row in conn.execute("PRAGMA table_info(filings)")}
    for column in ("error_count", "warning_count", "inconsistency_count"):
        if column not in existing:
            conn.execute(f"ALTER TABLE filings ADD COLUMN {column} INTEGER DEFAULT 0")
    conn.commit()


def upsert_entity(
    conn: sqlite3.Connection, name: str, lei: str | None, country: str | None
) -> int:
    """Insert or update an entity keyed by LEI; return its row id."""
    if lei:
        conn.execute(
            """
            INSERT INTO entities (name, lei, country)
            VALUES (?, ?, ?)
            ON CONFLICT(lei) DO UPDATE SET
                name = excluded.name,
                country = excluded.country
            """,
            (name, lei, country),
        )
        row = conn.execute("SELECT id FROM entities WHERE lei = ?", (lei,)).fetchone()
    else:
        # No LEI (e.g. an issuer whose jurisdiction the index does not cover):
        # key on (name, country) so re-runs stay idempotent.
        row = conn.execute(
            "SELECT id FROM entities WHERE name = ? AND lei IS NULL", (name,)
        ).fetchone()
        if row is None:
            conn.execute(
                "INSERT INTO entities (name, lei, country) VALUES (?, NULL, ?)",
                (name, country),
            )
            row = conn.execute(
                "SELECT id FROM entities WHERE name = ? AND lei IS NULL", (name,)
            ).fetchone()
        else:
            conn.execute(
                "UPDATE entities SET country = ? WHERE id = ?", (country, row["id"])
            )
    conn.commit()
    return int(row["id"])


def upsert_filing(
    conn: sqlite3.Connection,
    entity_id: int,
    reporting_date: str | None,
    filing_url: str | None,
    xbrl_json_url: str | None,
    country: str | None,
    validation_message_count: int,
    error_count: int = 0,
    warning_count: int = 0,
    inconsistency_count: int = 0,
) -> None:
    """Insert or update a filing keyed by (entity_id, reporting_date)."""
    conn.execute(
        """
        INSERT INTO filings (
            entity_id, reporting_date, filing_url, xbrl_json_url,
            country, validation_message_count,
            error_count, warning_count, inconsistency_count
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(entity_id, reporting_date) DO UPDATE SET
            filing_url = excluded.filing_url,
            xbrl_json_url = excluded.xbrl_json_url,
            country = excluded.country,
            validation_message_count = excluded.validation_message_count,
            error_count = excluded.error_count,
            warning_count = excluded.warning_count,
            inconsistency_count = excluded.inconsistency_count
        """,
        (
            entity_id,
            reporting_date,
            filing_url,
            xbrl_json_url,
            country,
            validation_message_count,
            error_count,
            warning_count,
            inconsistency_count,
        ),
    )
    conn.commit()


def upsert_fact(
    conn: sqlite3.Connection,
    entity_id: int,
    reporting_date: str | None,
    concept: str,
    value: str | None,
    currency: str | None,
) -> None:
    """Insert or update a financial fact keyed by (entity, date, concept)."""
    conn.execute(
        """
        INSERT INTO financial_facts (entity_id, reporting_date, concept, value, currency)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(entity_id, reporting_date, concept) DO UPDATE SET
            value = excluded.value,
            currency = excluded.currency
        """,
        (entity_id, reporting_date, concept, value, currency),
    )
    conn.commit()


def upsert_extension(
    conn: sqlite3.Connection,
    entity_id: int,
    reporting_date: str | None,
    concept: str,
    prefix: str,
    value: str | None,
    currency: str | None,
) -> None:
    """Insert or update a company-specific extension fact."""
    conn.execute(
        """
        INSERT INTO extension_facts
            (entity_id, reporting_date, concept, prefix, value, currency)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(entity_id, reporting_date, concept) DO UPDATE SET
            prefix = excluded.prefix,
            value = excluded.value,
            currency = excluded.currency
        """,
        (entity_id, reporting_date, concept, prefix, value, currency),
    )
    conn.commit()


def upsert_fx_rate(
    conn: sqlite3.Connection, currency: str, year: str, rate_per_eur: float
) -> None:
    """Insert or update an annual FX rate (units of `currency` per 1 EUR)."""
    conn.execute(
        """
        INSERT INTO fx_rates (currency, year, rate_per_eur)
        VALUES (?, ?, ?)
        ON CONFLICT(currency, year) DO UPDATE SET rate_per_eur = excluded.rate_per_eur
        """,
        (currency, year, rate_per_eur),
    )
    conn.commit()


if __name__ == "__main__":
    with connect() as c:
        init_db(c)
    print(f"Initialised schema at {DB_PATH}")
