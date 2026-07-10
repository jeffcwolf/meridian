"""Tests for the SQLite schema and upsert helpers.

These cover the two behaviours the pipeline relies on: the schema is created
correctly and re-runnably, and every upsert is idempotent (re-running refreshes
a row rather than duplicating it).
"""

from __future__ import annotations

import sqlite3

import db


def test_init_db_creates_all_tables(conn: sqlite3.Connection):
    tables = {
        row["name"]
        for row in conn.execute("SELECT name FROM sqlite_master WHERE type = 'table'")
    }
    assert {
        "entities",
        "filings",
        "financial_facts",
        "extension_facts",
        "fx_rates",
    } <= tables


def test_init_db_is_idempotent(conn: sqlite3.Connection):
    # A second (and third) call on an already-initialised database must not raise.
    db.init_db(conn)
    db.init_db(conn)


def test_upsert_entity_reuses_the_row_for_a_repeated_lei(conn: sqlite3.Connection):
    first = db.upsert_entity(conn, "Siemens AG", "LEI-SIE", "DE")
    second = db.upsert_entity(conn, "Siemens Aktiengesellschaft", "LEI-SIE", "DE")
    assert first == second
    name = conn.execute("SELECT name FROM entities WHERE id = ?", (first,)).fetchone()[
        "name"
    ]
    assert name == "Siemens Aktiengesellschaft"  # name is refreshed on conflict


def test_upsert_entity_without_lei_keys_on_name(conn: sqlite3.Connection):
    first = db.upsert_entity(conn, "No LEI Corp", None, "IT")
    second = db.upsert_entity(conn, "No LEI Corp", None, "IT")
    assert first == second
    count = conn.execute("SELECT COUNT(*) AS n FROM entities").fetchone()["n"]
    assert count == 1


def test_upsert_fact_refreshes_value_instead_of_duplicating(conn: sqlite3.Connection):
    entity = db.upsert_entity(conn, "ACME", "LEI-ACME", "FR")
    db.upsert_fact(conn, entity, "2023-12-31", "ifrs-full:Revenue", "100", "EUR")
    db.upsert_fact(conn, entity, "2023-12-31", "ifrs-full:Revenue", "200", "EUR")
    rows = conn.execute("SELECT value FROM financial_facts").fetchall()
    assert len(rows) == 1
    assert rows[0]["value"] == "200"


def test_upsert_fx_rate_refreshes_rate_on_conflict(conn: sqlite3.Connection):
    db.upsert_fx_rate(conn, "USD", "2023", 1.08)
    db.upsert_fx_rate(conn, "USD", "2023", 1.09)
    rows = conn.execute("SELECT rate_per_eur FROM fx_rates").fetchall()
    assert len(rows) == 1
    assert rows[0]["rate_per_eur"] == 1.09
