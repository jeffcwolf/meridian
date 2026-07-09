"""Diagnose which IFRS concepts each issuer actually tags.

One-shot report to replace piecemeal concept fixes: downloads the most recent
annual XBRL-JSON per entity and shows, per company, which headline rows are
present and under which tag — plus, for anything missing, the largest monetary
core-dimension facts so we can see what tag the issuer used instead.

    cd scripts && uv run python src/diagnose_concepts.py

Paste the output back and we can build a complete concept/alias map in one go.
"""

from __future__ import annotations

from collections import defaultdict

import httpx

import db
from parse_xbrl_json import CORE_DIMENSIONS, _period_end, download_report

# Logical row -> accepted tags (extend this from what the report reveals).
TARGET_ROWS: dict[str, list[str]] = {
    "Revenue": ["ifrs-full:Revenue", "ifrs-full:RevenueFromContractsWithCustomers"],
    "ProfitLoss": ["ifrs-full:ProfitLoss"],
    "Assets": ["ifrs-full:Assets"],
    "Equity": ["ifrs-full:Equity", "ifrs-full:EquityAttributableToOwnersOfParent"],
    "OpCashFlow": ["ifrs-full:CashFlowsFromUsedInOperatingActivities"],
}


def _as_int(value: str | None) -> int:
    digits = "".join(c for c in (value or "") if c.isdigit() or c == "-")
    try:
        return int(digits)
    except ValueError:
        return 0


def core_monetary_facts(report: dict) -> dict[str, tuple[str, str]]:
    """concept -> (value, unit) for core-dimension monetary facts (latest period)."""
    best: dict[str, tuple[str, str, str]] = {}  # concept -> (end, value, unit)
    for fact in report.get("facts", {}).values():
        dims = fact.get("dimensions", {})
        concept = dims.get("concept")
        unit = dims.get("unit", "") or ""
        if not concept or not unit.startswith("iso4217:"):
            continue
        if set(dims) - CORE_DIMENSIONS:
            continue
        end = _period_end(dims.get("period", ""))
        current = best.get(concept)
        if current is None or end > current[0]:
            best[concept] = (end, fact.get("value"), unit)
    return {c: (v, u) for c, (_e, v, u) in best.items()}


def main() -> None:
    conn = db.connect()
    rows = conn.execute(
        """
        SELECT e.name, f.reporting_date, f.xbrl_json_url
        FROM filings f JOIN entities e ON e.id = f.entity_id
        WHERE f.xbrl_json_url IS NOT NULL AND f.xbrl_json_url != ''
          AND f.reporting_date = (
              SELECT MAX(reporting_date) FROM filings WHERE entity_id = e.id
          )
        ORDER BY e.name
        """
    ).fetchall()
    conn.close()

    tag_usage: dict[str, dict[str, list[str]]] = defaultdict(lambda: defaultdict(list))
    missing: dict[str, list[str]] = defaultdict(list)

    with httpx.Client(headers={"Accept": "application/json"}) as client:
        for row in rows:
            report = download_report(client, row["xbrl_json_url"])
            if report is None:
                continue
            facts = core_monetary_facts(report)
            present = set(facts)

            summary = []
            incomplete = False
            for label, tags in TARGET_ROWS.items():
                found = next((t for t in tags if t in present), None)
                if found:
                    tag_usage[label][found].append(row["name"])
                    summary.append(f"{label}=✓")
                else:
                    missing[label].append(row["name"])
                    summary.append(f"{label}=—")
                    incomplete = True
            print(f"{row['name'][:34]:34} {row['reporting_date']}  {' '.join(summary)}")

            if incomplete:
                top = sorted(
                    facts.items(), key=lambda kv: abs(_as_int(kv[1][0])), reverse=True
                )[:12]
                for concept, (value, _unit) in top:
                    print(f"      {concept:62} {value}")

    print("\n=== tag usage (which element each company reports the line under) ===")
    for label, tags in TARGET_ROWS.items():
        print(f"{label}:")
        for tag, comps in tag_usage[label].items():
            print(f"   {tag:58} {len(comps)}")

    print("\n=== still missing (no accepted tag found) ===")
    for label in TARGET_ROWS:
        comps = missing[label]
        if comps:
            print(f"{label} ({len(comps)}): {', '.join(comps)}")


if __name__ == "__main__":
    main()
