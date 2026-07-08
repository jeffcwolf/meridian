"""Fetch ESEF filing metadata for a seed list of European issuers.

Pipeline step 1: populate the ``entities`` and ``filings`` tables in
``data/meridian.db`` from the public filings.xbrl.org API (via the
``xbrl-filings-api`` wrapper), resolving each issuer's LEI through GLEIF.

Run locally (needs outbound access to api.gleif.org and filings.xbrl.org):

    cd scripts && uv run python src/fetch_filings.py

The script is idempotent — re-running refreshes the same rows.
"""

from __future__ import annotations

import warnings

import httpx
import xbrl_filings_api as xf
from xbrl_filings_api.exceptions import FilterNotSupportedWarning

import db

# ---------------------------------------------------------------------------
# Seed: 10 well-known European issuers across 7 countries.
# (Shell from the brief is omitted only because its ESEF filing jurisdiction is
#  ambiguous post-relisting; swap any of these freely — the LEI is resolved
#  from name + country at runtime.)
# ---------------------------------------------------------------------------
SEED: list[dict[str, str]] = [
    {"name": "Siemens AG", "country": "DE"},
    {"name": "SAP SE", "country": "DE"},
    {"name": "LVMH Moet Hennessy Louis Vuitton", "country": "FR"},
    {"name": "TotalEnergies SE", "country": "FR"},
    {"name": "Iberdrola SA", "country": "ES"},
    {"name": "Banco Santander SA", "country": "ES"},
    {"name": "Novo Nordisk A/S", "country": "DK"},
    {"name": "Nokia Oyj", "country": "FI"},
    {"name": "Enel SpA", "country": "IT"},
    {"name": "ASML Holding NV", "country": "NL"},
]

# Pin an exact LEI here if GLEIF full-text picks the wrong entity for a name,
# e.g. {"Siemens AG": "W38RGI023SG3JQ..."}. Empty by default.
LEI_OVERRIDES: dict[str, str] = {}

GLEIF_URL = "https://api.gleif.org/api/v1/lei-records"


def _significant_tokens(name: str) -> set[str]:
    """Lower-cased name tokens, dropping common legal-form suffixes."""
    stop = {
        "ag", "se", "sa", "spa", "s.p.a", "nv", "n.v", "oyj", "plc", "a/s",
        "holding", "group", "the", "and", "&", "co", "se.", "ab", "asa",
    }
    return {t for t in name.lower().replace(",", " ").split() if t not in stop}


def resolve_lei(client: httpx.Client, name: str, country: str) -> tuple[str | None, str]:
    """Resolve an issuer to (LEI, canonical legal name) via the GLEIF API.

    Returns ``(None, name)`` if no confident match is found.
    """
    if name in LEI_OVERRIDES:
        return LEI_OVERRIDES[name], name

    params = {
        "filter[fulltext]": name,
        "filter[entity.legalAddress.country]": country,
        "page[size]": "10",
    }
    try:
        resp = client.get(GLEIF_URL, params=params, timeout=30.0)
        resp.raise_for_status()
    except httpx.HTTPError as exc:
        print(f"  ! GLEIF lookup failed for {name}: {exc}")
        return None, name

    records = resp.json().get("data", [])
    want = _significant_tokens(name)
    best: tuple[int, int, str, str] | None = None  # (overlap, -len, lei, legal_name)
    for rec in records:
        attrs = rec.get("attributes", {})
        lei = attrs.get("lei")
        legal_name = (
            attrs.get("entity", {}).get("legalName", {}).get("name", "") or ""
        )
        if not lei:
            continue
        overlap = len(want & _significant_tokens(legal_name))
        score = (overlap, -len(legal_name), lei, legal_name)
        if best is None or score > best:
            best = score

    if best is None or best[0] == 0:
        print(f"  ! No confident GLEIF match for {name} ({country})")
        return None, name
    return best[2], best[3]


def _validation_count(filing: xf.Filing) -> int:
    """Number of validation messages for a filing (needs GET_VALIDATION_MESSAGES)."""
    msgs = getattr(filing, "validation_messages", None)
    if msgs:
        return len(msgs)
    return (
        (filing.error_count or 0)
        + (filing.warning_count or 0)
        + (filing.inconsistency_count or 0)
    )


def _reporting_date(filing: xf.Filing) -> str | None:
    date = getattr(filing, "reporting_date", None) or filing.last_end_date
    return str(date)[:10] if date else None


def _filing_url(filing: xf.Filing) -> str | None:
    return filing.xhtml_url or filing.viewer_url or filing.package_url


def _store_filing(conn, entity_id: int, filing: xf.Filing) -> None:
    db.upsert_filing(
        conn,
        entity_id=entity_id,
        reporting_date=_reporting_date(filing),
        filing_url=_filing_url(filing),
        xbrl_json_url=filing.json_url,
        country=filing.country,
        validation_message_count=_validation_count(filing),
    )


def _country_scan_by_name(name: str, country: str) -> list[xf.Filing]:
    """Fallback: scan a country's filings and match by entity name substring."""
    want = _significant_tokens(name)
    matched: list[xf.Filing] = []
    filings = xf.get_filings(
        filters={"country": country},
        sort="-last_end_date",
        limit=0,
        flags=xf.GET_ENTITY | xf.GET_VALIDATION_MESSAGES,
    )
    for filing in filings:
        ent = filing.entity
        if ent and want & _significant_tokens(ent.name or ""):
            matched.append(filing)
    return matched


def main() -> None:
    conn = db.connect()
    db.init_db(conn)

    # 1. Resolve LEIs and upsert entities.
    resolved: list[dict] = []
    with httpx.Client(headers={"Accept": "application/vnd.api+json"}) as client:
        for seed in SEED:
            lei, legal_name = resolve_lei(client, seed["name"], seed["country"])
            entity_id = db.upsert_entity(conn, legal_name, lei, seed["country"])
            resolved.append(
                {**seed, "lei": lei, "legal_name": legal_name, "entity_id": entity_id}
            )
            print(f"  {legal_name:45} {seed['country']}  LEI={lei}")

    # 2. Fetch filings for all resolved LEIs in one multi-filter query.
    leis = [r["lei"] for r in resolved if r["lei"]]
    by_lei: dict[str, int] = {r["lei"]: r["entity_id"] for r in resolved if r["lei"]}
    total = 0
    if leis:
        with warnings.catch_warnings():
            # entity.identifier is a valid server-side filter even though the
            # wrapper flags it as "unsupported" locally.
            warnings.simplefilter("ignore", FilterNotSupportedWarning)
            filings = xf.get_filings(
                filters={"entity.identifier": leis},
                sort="-last_end_date",
                limit=0,
                flags=xf.GET_ENTITY | xf.GET_VALIDATION_MESSAGES,
            )
        for filing in filings:
            ent = filing.entity
            lei = ent.identifier if ent else filing.entity_api_id
            entity_id = by_lei.get(lei)
            if entity_id is None:
                continue
            _store_filing(conn, entity_id, filing)
            total += 1

    # 3. Fallback for any seed with no filings: scan its country, match by name.
    counts = {
        r["entity_id"]: conn.execute(
            "SELECT COUNT(*) AS n FROM filings WHERE entity_id = ?", (r["entity_id"],)
        ).fetchone()["n"]
        for r in resolved
    }
    for r in resolved:
        if counts[r["entity_id"]]:
            continue
        print(f"  ~ no LEI-matched filings for {r['legal_name']}; scanning {r['country']}")
        for filing in _country_scan_by_name(r["name"], r["country"]):
            ent = filing.entity
            if ent and ent.identifier and not r["lei"]:
                # Backfill the entity's LEI from the matched filing.
                conn.execute(
                    "UPDATE entities SET lei = ? WHERE id = ?",
                    (ent.identifier, r["entity_id"]),
                )
                conn.commit()
            _store_filing(conn, r["entity_id"], filing)
            total += 1

    n_entities = conn.execute("SELECT COUNT(*) AS n FROM entities").fetchone()["n"]
    n_filings = conn.execute("SELECT COUNT(*) AS n FROM filings").fetchone()["n"]
    conn.close()
    print(f"\nDone: {n_entities} entities, {n_filings} filings in {db.DB_PATH}")


if __name__ == "__main__":
    main()
