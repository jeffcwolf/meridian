"""Shared pytest fixtures for the pipeline tests."""

from __future__ import annotations

import sqlite3
from collections.abc import Iterator

import pytest

import db


@pytest.fixture
def conn() -> Iterator[sqlite3.Connection]:
    """An initialised, empty Meridian database held in memory."""
    connection = sqlite3.connect(":memory:")
    connection.row_factory = sqlite3.Row
    connection.execute("PRAGMA foreign_keys = ON")
    db.init_db(connection)
    yield connection
    connection.close()
