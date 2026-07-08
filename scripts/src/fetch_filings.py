"""Fetch ESEF filing metadata for a curated list of European issuers.

Pipeline step 1: populate the ``entities`` and ``filings`` tables in
``data/meridian.db`` from the public filings.xbrl.org API (via the
``xbrl-filings-api`` wrapper), resolving each issuer's LEI through GLEIF.

Run locally (needs outbound access to api.gleif.org and filings.xbrl.org):

    cd scripts && uv run python src/fetch_filings.py

The script is idempotent — re-running refreshes the same rows.

Note on coverage: filings.xbrl.org aggregates from participating national OAMs.
Some jurisdictions — most notably Germany — do not publish their ESEF filings to
the public index, so issuers there resolve to a valid LEI but return zero
filings. Those issuers are kept on purpose to surface the coverage gap.
"""

from __future__ import annotations

import re
import unicodedata
import warnings
from collections import defaultdict

import httpx
import xbrl_filings_api as xf
from xbrl_filings_api.exceptions import FilterNotSupportedWarning

import db

# ---------------------------------------------------------------------------
# Curated universe: ~39 large-cap issuers across 12 countries. The German names
# are included deliberately to demonstrate the filings.xbrl.org coverage gap.
# LEIs are resolved from name + country at runtime (see resolve_lei); pin any
# that resolve to the wrong entity in LEI_OVERRIDES below.
# ---------------------------------------------------------------------------
SEED: list[dict[str, str]] = [
    # Netherlands
    {"name": "ASML Holding NV", "country": "NL"},
    {"name": "Koninklijke Philips NV", "country": "NL"},
    {"name": "Heineken NV", "country": "NL"},
    {"name": "ING Groep NV", "country": "NL"},
    {"name": "Koninklijke Ahold Delhaize NV", "country": "NL"},
    # France
    {"name": "LVMH Moet Hennessy Louis Vuitton", "country": "FR"},
    {"name": "TotalEnergies SE", "country": "FR"},
    {"name": "Sanofi", "country": "FR"},
    {"name": "Schneider Electric SE", "country": "FR"},
    # Spain
    {"name": "Iberdrola SA", "country": "ES"},
    {"name": "Banco Santander SA", "country": "ES"},
    {"name": "Industria de Diseno Textil SA", "country": "ES"},
    {"name": "Telefonica SA", "country": "ES"},
    # Italy
    {"name": "Enel SpA", "country": "IT"},
    {"name": "Eni SpA", "country": "IT"},
    {"name": "Intesa Sanpaolo SpA", "country": "IT"},
    {"name": "UniCredit SpA", "country": "IT"},
    # Denmark
    {"name": "Novo Nordisk A/S", "country": "DK"},
    {"name": "Carlsberg A/S", "country": "DK"},
    {"name": "Vestas Wind Systems A/S", "country": "DK"},
    # Finland
    {"name": "Nokia Oyj", "country": "FI"},
    {"name": "Kone Oyj", "country": "FI"},
    {"name": "Neste Oyj", "country": "FI"},
    {"name": "Nordea Bank Abp", "country": "FI"},
    # Sweden
    {"name": "Telefonaktiebolaget LM Ericsson", "country": "SE"},
    {"name": "Atlas Copco AB", "country": "SE"},
    {"name": "Hennes & Mauritz AB", "country": "SE"},
    {"name": "Investor AB", "country": "SE"},
    # Norway
    {"name": "Equinor ASA", "country": "NO"},
    {"name": "DNB Bank ASA", "country": "NO"},
    # Belgium
    {"name": "Anheuser-Busch InBev SA/NV", "country": "BE"},
    {"name": "KBC Group NV", "country": "BE"},
    # Portugal
    {"name": "EDP - Energias de Portugal SA", "country": "PT"},
    # Austria
    {"name": "OMV AG", "country": "AT"},
    # Ireland
    {"name": "Ryanair Holdings plc", "country": "IE"},
    # Germany (kept to demonstrate the coverage gap — expect zero filings)
    {"name": "SAP SE", "country": "DE"},
    {"name": "Siemens AG", "country": "DE"},
    {"name": "Volkswagen AG", "country": "DE"},
    {"name": "Allianz SE", "country": "DE"},
]

# Pin an exact LEI here if GLEIF resolves a name to the wrong entity,
# e.g. {"Siemens AG": "W38RGI023SG3JQ7VG076"}. Empty by default.
LEI_OVERRIDES: dict[str, str] = {}

GLEIF_URL = "https://api.gleif.org/api/v1/lei-records"

# Legal-form and generic words dropped before matching, so the distinctive part
# of a company name drives resolution.
STOPWORDS = {
    "ag", "se", "sa", "spa", "nv", "oyj", "oy", "plc", "ab", "asa", "abp",
    "kgaa", "sgps", "aktiengesellschaft", "aktiebolag", "holding", "holdings",
    "group", "groep", "groupe", "gruppo", "co", "company", "corporation",
    "corp", "inc", "ltd", "limited", "the", "and", "of", "publ", "societe",
    "europeenne", "anonyme", "de", "del", "di", "van", "von", "het", "bank",
}


def _normalize(text: str) -> str:
    """Lower-case and strip accents (Telefónica -> telefonica)."""
    decomposed = unicodedata.normalize("NFKD", text)
    return "".join(c for c in decomposed if not unicodedata.combining(c)).lower()


def _significant_tokens(name: str) -> set[str]:
    """Distinctive lower-cased name tokens (no punctuation, legal forms, or
    single characters)."""
    cleaned = re.sub(r"[^a-z0-9]+", " ", _normalize(name))
    return {t for t in cleaned.split() if len(t) > 1 and t not in STOPWORDS}


def resolve_lei(client: httpx.Client, name: str, country: str) -> tuple[str | None, str]:
    """Resolve an issuer to (LEI, canonical legal name) via the GLEIF API.

    Candidates are scored by token overlap first, then by fewest *extra* tokens
    — so the listed parent (e.g. "Siemens Aktiengesellschaft") beats a
    same-prefixed subsidiary (e.g. "Siemens Healthineers AG"). Returns
    ``(None, name)`` if no confident match is found.
    """
    if name in LEI_OVERRIDES:
        return LEI_OVERRIDES[name], name

    params = {
        "filter[fulltext]": name,
        "filter[entity.legalAddress.country]": country,
        "page[size]": "20",
    }
    try:
        resp = client.get(GLEIF_URL, params=params, timeout=30.0)
        resp.raise_for_status()
    except httpx.HTTPError as exc:
        print(f"  ! GLEIF lookup failed for {name}: {exc}")
        return None, name

    want = _significant_tokens(name)
    best: tuple[tuple[int, int], str, str] | None = None  # ((overlap, -extra), lei, name)
    for rec in resp.json().get("data", []):
        attrs = rec.get("attributes", {})
        lei = attrs.get("lei")
        legal_name = (attrs.get("entity", {}).get("legalName", {}).get("name") or "")
        if not lei:
            continue
        cand = _significant_tokens(legal_name)
        overlap = len(want & cand)
        if overlap == 0:
            continue
        score = (overlap, -len(cand - want))
        if best is None or score > best[0]:
            best = (score, lei, legal_name)

    if best is None:
        print(f"  ! No confident GLEIF match for {name} ({country})")
        return None, name
    return best[1], best[2]


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


def _annual_only(filings: list[xf.Filing]) -> list[xf.Filing]:
    """Keep one filing per reporting year — the one with the latest period end
    (the annual report), newest amendment winning ties. ESEF is an annual-report
    mandate, so this drops interim/duplicate packages that would otherwise
    collide on the same year column. Assumes calendar-year filers.
    """
    best_by_year: dict[str, tuple[tuple[str, str], xf.Filing]] = {}
    for filing in filings:
        rd = _reporting_date(filing)
        if not rd:
            continue
        year = rd[:4]
        key = (rd, str(getattr(filing, "processed_time", "") or ""))
        if year not in best_by_year or key > best_by_year[year][0]:
            best_by_year[year] = (key, filing)
    return [f for _, (_, f) in sorted(best_by_year.items())]


def _country_scan_by_name(name: str, country: str) -> list[xf.Filing]:
    """Fallback: scan a country's filings and match by entity name tokens."""
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
            print(f"  {legal_name[:45]:45} {seed['country']}  LEI={lei}")

    # 2. Fetch filings for all resolved LEIs in one multi-filter query, grouped
    #    back to their entity.
    leis = [r["lei"] for r in resolved if r["lei"]]
    by_lei = {r["lei"]: r["entity_id"] for r in resolved if r["lei"]}
    per_entity: dict[int, list[xf.Filing]] = defaultdict(list)
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
            if entity_id is not None:
                per_entity[entity_id].append(filing)

    # 3. Fallback for any seed with no filings: scan its country, match by name.
    for r in resolved:
        if per_entity.get(r["entity_id"]):
            continue
        print(f"  ~ no LEI-matched filings for {r['legal_name']}; scanning {r['country']}")
        for filing in _country_scan_by_name(r["name"], r["country"]):
            ent = filing.entity
            if ent and ent.identifier and not r["lei"]:
                conn.execute(
                    "UPDATE entities SET lei = ? WHERE id = ?",
                    (ent.identifier, r["entity_id"]),
                )
                conn.commit()
            per_entity[r["entity_id"]].append(filing)

    # 4. Reduce to one annual per year and store.
    total = 0
    for entity_id, flist in per_entity.items():
        for filing in _annual_only(flist):
            _store_filing(conn, entity_id, filing)
            total += 1

    n_entities = conn.execute("SELECT COUNT(*) AS n FROM entities").fetchone()["n"]
    n_filings = conn.execute("SELECT COUNT(*) AS n FROM filings").fetchone()["n"]
    n_empty = conn.execute(
        "SELECT COUNT(*) AS n FROM entities e "
        "WHERE NOT EXISTS (SELECT 1 FROM filings f WHERE f.entity_id = e.id)"
    ).fetchone()["n"]
    conn.close()
    print(
        f"\nDone: {n_entities} entities ({n_empty} with no discoverable filings), "
        f"{n_filings} filings in {db.DB_PATH}"
    )


if __name__ == "__main__":
    main()
