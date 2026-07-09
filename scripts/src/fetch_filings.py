"""Fetch ESEF filing metadata for a curated list of European issuers.

Pipeline step 1: populate the ``entities`` and ``filings`` tables in
``data/meridian.db`` from the public filings.xbrl.org API (via the
``xbrl-filings-api`` wrapper).

Run locally (needs outbound access to filings.xbrl.org):

    cd scripts && uv run python src/fetch_filings.py

Resolution strategy: rather than resolving names through GLEIF (whose
full-text ranking surfaces subsidiaries and misses parents), we scan each
country's filings on filings.xbrl.org and match each seed name to the actual
*filer*. Only the listed parent files a consolidated ESEF annual report, so the
filer entity is the one we want — and its LEI + canonical name come straight
from the index.

Note on coverage: filings.xbrl.org aggregates from participating national OAMs.
Some jurisdictions — most notably Germany — do not publish their ESEF filings to
the public index, so issuers there match nothing and are stored with no filings
on purpose, to surface the coverage gap.

The script is idempotent — re-running refreshes the same rows.
"""

from __future__ import annotations

import re
import unicodedata
from collections import defaultdict
from datetime import date

import xbrl_filings_api as xf
from xbrl_filings_api.exceptions import FilingsAPIError

import db

# ---------------------------------------------------------------------------
# Curated universe: ~39 large-cap issuers across 13 countries. The German names
# are included deliberately to demonstrate the filings.xbrl.org coverage gap.
# ---------------------------------------------------------------------------
SEED: list[dict[str, str]] = [
    # Netherlands
    {"name": "ASML Holding", "country": "NL"},
    {"name": "Koninklijke Philips", "country": "NL"},
    {"name": "Heineken", "country": "NL"},
    {"name": "ING Groep", "country": "NL"},
    {"name": "Koninklijke Ahold Delhaize", "country": "NL"},
    # France
    {"name": "LVMH Moet Hennessy Louis Vuitton", "country": "FR"},
    {"name": "TotalEnergies", "country": "FR"},
    {"name": "Sanofi", "country": "FR"},
    {"name": "Schneider Electric", "country": "FR"},
    # Spain
    {"name": "Iberdrola", "country": "ES"},
    {"name": "Banco Santander", "country": "ES"},
    {"name": "Industria de Diseno Textil", "country": "ES"},
    {"name": "Telefonica", "country": "ES"},
    # Italy
    {"name": "Enel", "country": "IT"},
    {"name": "Eni", "country": "IT"},
    {"name": "Intesa Sanpaolo", "country": "IT"},
    {"name": "UniCredit", "country": "IT"},
    # Denmark
    {"name": "Novo Nordisk", "country": "DK"},
    {"name": "Carlsberg", "country": "DK"},
    {"name": "Vestas Wind Systems", "country": "DK"},
    # Finland
    {"name": "Nokia", "country": "FI"},
    {"name": "Kone", "country": "FI"},
    {"name": "Neste", "country": "FI"},
    {"name": "Nordea Bank", "country": "FI"},
    # Sweden
    {"name": "Telefonaktiebolaget LM Ericsson", "country": "SE"},
    {"name": "Atlas Copco", "country": "SE"},
    {"name": "Hennes & Mauritz", "country": "SE"},
    {"name": "Investor", "country": "SE"},
    # Norway
    {"name": "Equinor", "country": "NO"},
    {"name": "DNB Bank", "country": "NO"},
    # Belgium
    {"name": "Anheuser-Busch InBev", "country": "BE"},
    {"name": "KBC Group", "country": "BE"},
    # Portugal
    {"name": "EDP Energias de Portugal", "country": "PT"},
    # Austria
    {"name": "OMV", "country": "AT"},
    # Ireland
    {"name": "Ryanair Holdings", "country": "IE"},
    # Germany (kept to demonstrate the coverage gap — expect zero filings)
    {"name": "SAP", "country": "DE"},
    {"name": "Siemens", "country": "DE"},
    {"name": "Volkswagen", "country": "DE"},
    {"name": "Allianz", "country": "DE"},
]

# Legal-form and generic words dropped before matching, so the distinctive part
# of a company name drives resolution.
# Only pure legal-form suffixes and articles are dropped. Distinctive words like
# "bank", "group"/"groep" and "holding" are kept — they disambiguate the listed
# parent from siblings (e.g. "ING Groep" vs "ING Bank").
STOPWORDS = {
    "ag", "se", "sa", "spa", "nv", "oyj", "oy", "plc", "ab", "asa", "abp",
    "kgaa", "aktiengesellschaft", "aktiebolag", "co", "company", "corporation",
    "corp", "inc", "ltd", "limited", "the", "and", "of", "publ", "societe",
    "europeenne", "anonyme", "de", "del", "di", "van", "von", "het",
}


def _normalize(text: str) -> str:
    """Lower-case and strip accents (Telefónica -> telefonica)."""
    decomposed = unicodedata.normalize("NFKD", text)
    return "".join(c for c in decomposed if not unicodedata.combining(c)).lower()


def _tokens(name: str) -> set[str]:
    """Distinctive lower-cased name tokens (no punctuation, legal forms, or
    single characters)."""
    cleaned = re.sub(r"[^a-z0-9]+", " ", _normalize(name))
    return {t for t in cleaned.split() if len(t) > 1 and t not in STOPWORDS}


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
        error_count=filing.error_count or 0,
        warning_count=filing.warning_count or 0,
        inconsistency_count=filing.inconsistency_count or 0,
    )


def _annual_only(filings: list[xf.Filing]) -> list[xf.Filing]:
    """Keep one annual filing per fiscal year.

    ESEF is an annual-report mandate, but the index also carries interim/quarterly
    packages and duplicates. Consecutive annual reports are ~12 months apart
    whatever the exact year-end, while interims fall closer, so we sort by period
    end and greedily keep a filing only when it is >= 300 days after the last one
    kept. This drops off-cycle interims (e.g. a Q1 report) yet works for December,
    January (retail) and 52/53-week fiscal calendars alike. Exact-date duplicates
    collapse to the newest amendment first.
    """
    # Collapse exact-date duplicates/amendments, keeping the newest processed.
    by_date: dict[str, xf.Filing] = {}
    for filing in filings:
        rd = _reporting_date(filing)
        if not rd:
            continue
        prev = by_date.get(rd)
        if prev is None or str(getattr(filing, "processed_time", "") or "") >= str(
            getattr(prev, "processed_time", "") or ""
        ):
            by_date[rd] = filing

    kept: list[xf.Filing] = []
    last: date | None = None
    for rd in sorted(by_date):
        end = date.fromisoformat(rd)
        if last is None or (end - last).days >= 300:
            kept.append(by_date[rd])
            last = end
    return kept


def _best_match(seed_name: str, filers: dict[str, tuple[str, list]]) -> str | None:
    """Pick the filer LEI best matching a seed name: most token overlap, then
    fewest extra tokens (parent over subsidiary), then most filings.
    """
    want = _tokens(seed_name)
    best: tuple[tuple[int, int, int], str] | None = None
    for lei, (name, flist) in filers.items():
        cand = _tokens(name)
        overlap = len(want & cand)
        if overlap == 0:
            continue
        score = (overlap, -len(cand - want), len(flist))
        if best is None or score > best[0]:
            best = (score, lei)
    return best[1] if best else None


def _scan_country(country: str) -> dict[str, tuple[str, list]]:
    """Return {LEI: (entity_name, [filings])} for every filer in a country."""
    filers: dict[str, list] = defaultdict(lambda: ["", []])
    try:
        filings = xf.get_filings(
            filters={"country": country},
            sort="-last_end_date",
            limit=0,
            flags=xf.GET_ENTITY | xf.GET_VALIDATION_MESSAGES,
        )
    except FilingsAPIError as exc:
        print(f"  ! API error scanning {country}: {exc}")
        return {}
    for filing in filings:
        ent = filing.entity
        if not ent or not ent.identifier:
            continue
        record = filers[ent.identifier]
        record[0] = ent.name or record[0]
        record[1].append(filing)
    return {lei: (name, flist) for lei, (name, flist) in filers.items()}


def main() -> None:
    conn = db.connect()
    db.init_db(conn)

    by_country: dict[str, list[dict]] = defaultdict(list)
    for seed in SEED:
        by_country[seed["country"]].append(seed)

    matched = 0
    gaps = 0
    for country in sorted(by_country):
        seeds = by_country[country]
        print(f"Scanning {country} ({len(seeds)} seeds)…")
        filers = _scan_country(country)
        print(f"  {len(filers)} filers in the index")
        used: set[str] = set()
        for seed in seeds:
            lei = _best_match(seed["name"], filers)
            if lei and lei not in used:
                used.add(lei)
                name, flist = filers[lei]
                entity_id = db.upsert_entity(conn, name, lei, country)
                annual = _annual_only(flist)
                for filing in annual:
                    _store_filing(conn, entity_id, filing)
                matched += 1
                print(f"    {name[:44]:44} {len(annual):>2} filings  {lei}")
            else:
                db.upsert_entity(conn, seed["name"], None, country)
                gaps += 1
                print(f"    {seed['name'][:44]:44}  no discoverable filings")

    n_entities = conn.execute("SELECT COUNT(*) AS n FROM entities").fetchone()["n"]
    n_filings = conn.execute("SELECT COUNT(*) AS n FROM filings").fetchone()["n"]
    conn.close()
    print(
        f"\nDone: {n_entities} entities ({matched} matched, {gaps} with no "
        f"discoverable filings), {n_filings} filings in {db.DB_PATH}"
    )


if __name__ == "__main__":
    main()
