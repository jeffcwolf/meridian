"""Parse XBRL-JSON extracts into structured IFRS financial facts.

Pipeline step 2: for every filing recorded by ``fetch_filings.py``, download
the xBRL-JSON (OIM) extract from ``filings.filings.xbrl.org`` and pull out a
handful of headline IFRS concepts into the ``financial_facts`` table.

Run locally (needs outbound access to filings.xbrl.org):

    cd scripts && uv run python src/parse_xbrl_json.py

The script is idempotent — re-running refreshes the same facts.
"""

from __future__ import annotations

import time

import httpx

import db

# Headline IFRS concepts we surface in the single-company viewer.
# Headline concepts plus accepted aliases. Facts are stored as-is (the real
# tag); the UI coalesces aliases into one row. Different issuers tag the same
# line differently — e.g. some use RevenueFromContractsWithCustomers, and some
# report only equity attributable to owners of the parent.
TARGET_CONCEPTS: set[str] = {
    "ifrs-full:Revenue",
    "ifrs-full:RevenueFromContractsWithCustomers",
    "ifrs-full:RevenueFromSaleOfGoods",
    # Bank income lines (banks have no single "Revenue"): shown as their own
    # rows for financial issuers.
    "ifrs-full:RevenueFromInterest",
    "ifrs-full:RevenueFromFeeAndCommissionIncome",
    "ifrs-full:FeeAndCommissionIncome",
    "ifrs-full:Assets",
    "ifrs-full:ProfitLoss",
    "ifrs-full:Equity",
    "ifrs-full:EquityAttributableToOwnersOfParent",
    # The IFRS taxonomy element is "From(Used In)" — there is no plain
    # CashFlowsFromOperatingActivities element. Issuers with discontinued
    # operations tag the "ContinuingOperations" variant.
    "ifrs-full:CashFlowsFromUsedInOperatingActivities",
    "ifrs-full:CashFlowsFromUsedInOperatingActivitiesContinuingOperations",
}

# xBRL-JSON built-in ("core") dimensions. A fact carrying *only* these is a
# top-line consolidated value; anything with extra taxonomy dimensions is a
# segment/member breakdown that we skip.
CORE_DIMENSIONS: set[str] = {"concept", "entity", "period", "unit", "language"}


def _period_end(period: str) -> str:
    """Return the sortable end instant of an xBRL-JSON period string.

    Durations are ``start/end``; instants are a single datetime. Comparing the
    end lexicographically (ISO-8601) is enough to rank reporting years.
    """
    if not period:
        return ""
    return period.split("/")[-1]


def _currency(unit: str | None) -> str | None:
    """``iso4217:EUR`` -> ``EUR``."""
    if not unit:
        return None
    return unit.split(":")[-1]


def extract_facts(report: dict) -> dict[str, tuple[str, str | None]]:
    """Pick the current-year, consolidated value for each target concept.

    Returns ``{concept: (value, currency)}`` choosing, per concept, the fact
    with the latest period end among those carrying only core dimensions.
    """
    facts = report.get("facts", {})
    best: dict[str, tuple[str, str, str | None]] = {}  # concept -> (end, value, ccy)
    for fact in facts.values():
        dims = fact.get("dimensions", {})
        concept = dims.get("concept")
        if concept not in TARGET_CONCEPTS:
            continue
        # Reject facts with member/axis dimensions (segment breakdowns).
        if set(dims) - CORE_DIMENSIONS:
            continue
        value = fact.get("value")
        if value is None:
            continue
        end = _period_end(dims.get("period", ""))
        ccy = _currency(dims.get("unit"))
        current = best.get(concept)
        if current is None or end > current[0]:
            best[concept] = (end, str(value), ccy)
    return {c: (v, ccy) for c, (_end, v, ccy) in best.items()}


def download_report(client: httpx.Client, url: str, retries: int = 3) -> dict | None:
    """GET and JSON-decode an xBRL-JSON extract, with simple backoff."""
    for attempt in range(retries):
        try:
            resp = client.get(url, timeout=60.0)
            resp.raise_for_status()
            return resp.json()
        except (httpx.HTTPError, ValueError) as exc:
            wait = 2 ** attempt
            if attempt == retries - 1:
                print(f"    ! failed ({exc}); giving up")
                return None
            print(f"    ! {exc}; retrying in {wait}s")
            time.sleep(wait)
    return None


def main() -> None:
    conn = db.connect()
    db.init_db(conn)

    rows = conn.execute(
        """
        SELECT f.entity_id, f.reporting_date, f.xbrl_json_url, e.name
        FROM filings f JOIN entities e ON e.id = f.entity_id
        WHERE f.xbrl_json_url IS NOT NULL AND f.xbrl_json_url != ''
        ORDER BY e.name, f.reporting_date
        """
    ).fetchall()

    stored = 0
    with httpx.Client(headers={"Accept": "application/json"}) as client:
        for row in rows:
            print(f"  {row['name']} @ {row['reporting_date']}")
            report = download_report(client, row["xbrl_json_url"])
            if report is None:
                continue
            found = extract_facts(report)
            for concept, (value, currency) in found.items():
                db.upsert_fact(
                    conn,
                    entity_id=row["entity_id"],
                    reporting_date=row["reporting_date"],
                    concept=concept,
                    value=value,
                    currency=currency,
                )
                stored += 1
            missing = TARGET_CONCEPTS - set(found)
            note = f" (missing: {', '.join(sorted(missing))})" if missing else ""
            print(f"    stored {len(found)}/{len(TARGET_CONCEPTS)} concepts{note}")

    n_facts = conn.execute("SELECT COUNT(*) AS n FROM financial_facts").fetchone()["n"]
    conn.close()
    print(f"\nDone: {stored} facts written this run, {n_facts} total in {db.DB_PATH}")


if __name__ == "__main__":
    main()
