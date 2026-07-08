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

# scripts/src/db.py -> scripts/src -> scripts -> repo root -> data/meridian.db
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
) -> None:
    """Insert or update a filing keyed by (entity_id, reporting_date)."""
    conn.execute(
        """
        INSERT INTO filings (
            entity_id, reporting_date, filing_url, xbrl_json_url,
            country, validation_message_count
        )
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(entity_id, reporting_date) DO UPDATE SET
            filing_url = excluded.filing_url,
            xbrl_json_url = excluded.xbrl_json_url,
            country = excluded.country,
            validation_message_count = excluded.validation_message_count
        """,
        (
            entity_id,
            reporting_date,
            filing_url,
            xbrl_json_url,
            country,
            validation_message_count,
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


if __name__ == "__main__":
    with connect() as c:
        init_db(c)
    print(f"Initialised schema at {DB_PATH}")
