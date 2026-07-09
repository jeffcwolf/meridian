"""Fetch ECB annual reference FX rates for the currencies in the cache.

Pipeline step 3 (optional): populate the ``fx_rates`` table so the comparator
can convert figures to a common currency. Uses the ECB Data Portal EXR dataflow,
annual-average series (``A.<CUR>.EUR.SP00.A``). Rates are quoted as units of the
foreign currency per 1 EUR, so an amount in CUR converts to EUR by dividing.

Run locally (needs outbound access to data-api.ecb.europa.eu), after
fetch_filings + parse_xbrl_json have populated the currencies:

    cd scripts && uv run python src/fetch_fx_rates.py
"""

from __future__ import annotations

import csv
import io

import httpx

import db

ECB_URL = "https://data-api.ecb.europa.eu/service/data/EXR/A.{currency}.EUR.SP00.A"
START_YEAR = "2018"


def fetch_currency(client: httpx.Client, currency: str) -> list[tuple[str, float]]:
    """Return [(year, rate_per_eur)] of ECB annual-average rates for a currency."""
    resp = client.get(
        ECB_URL.format(currency=currency),
        params={"startPeriod": START_YEAR, "format": "csvdata"},
        timeout=30.0,
    )
    resp.raise_for_status()
    rates: list[tuple[str, float]] = []
    for row in csv.DictReader(io.StringIO(resp.text)):
        year, value = row.get("TIME_PERIOD"), row.get("OBS_VALUE")
        if not year or not value:
            continue
        try:
            rates.append((year, float(value)))
        except ValueError:
            continue
    return rates


def main() -> None:
    conn = db.connect()
    db.init_db(conn)

    currencies = [
        row["currency"]
        for row in conn.execute(
            "SELECT DISTINCT currency FROM financial_facts "
            "WHERE currency IS NOT NULL AND currency != '' AND currency != 'EUR'"
        )
    ]
    if not currencies:
        conn.close()
        print("No non-EUR currencies in the cache — nothing to fetch.")
        return

    print(f"Currencies needing rates: {', '.join(sorted(currencies))}")
    with httpx.Client(headers={"Accept": "text/csv"}) as client:
        for currency in sorted(currencies):
            try:
                rates = fetch_currency(client, currency)
            except httpx.HTTPError as exc:
                print(f"  ! {currency}: {exc}")
                continue
            for year, rate in rates:
                db.upsert_fx_rate(conn, currency, year, rate)
            print(f"  {currency}: {len(rates)} annual rates")

    n = conn.execute("SELECT COUNT(*) AS n FROM fx_rates").fetchone()["n"]
    conn.close()
    print(f"\nDone: {n} FX rates in {db.DB_PATH}")


if __name__ == "__main__":
    main()
