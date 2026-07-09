"""Populate data/meridian.db with an OFFLINE demo dataset (no network).

This exists purely so the Rust/Leptos app can be run and demonstrated without
first fetching from filings.xbrl.org. The figures are illustrative, hand-entered
approximations of published annual reports — NOT authoritative. Run the real
pipeline (``fetch_filings.py`` + ``parse_xbrl_json.py``) for accurate data.

    cd scripts && uv run python src/seed_demo.py
"""

from __future__ import annotations

import db

# concept short-hand -> canonical IFRS tag
CONCEPTS = {
    "rev": "ifrs-full:Revenue",
    "int": "ifrs-full:RevenueFromInterest",
    "fee": "ifrs-full:RevenueFromFeeAndCommissionIncome",
    "assets": "ifrs-full:Assets",
    "pl": "ifrs-full:ProfitLoss",
    "eq": "ifrs-full:Equity",
    "cfo": "ifrs-full:CashFlowsFromUsedInOperatingActivities",
}

# name, LEI, country, currency, {year: {concept: value_in_full_currency_units}}
DEMO: list[tuple[str, str, str, str, dict[int, dict[str, str]]]] = [
    ("Siemens AG", "W38RGI023SG3JQ7VG076", "DE", "EUR", {
        2023: {"rev": "77769000000", "assets": "141344000000", "pl": "8534000000",
               "eq": "63699000000", "cfo": "10171000000"},
        2022: {"rev": "71968000000", "assets": "135335000000", "pl": "4409000000",
               "eq": "56289000000", "cfo": "8264000000"},
    }),
    # German issuers: valid LEI but no discoverable filings (coverage gap demo).
    ("SAP SE", "5299007FMCAENG2ZLB13", "DE", "EUR", {}),
    ("Volkswagen AG", "529900NNUPAGGOM1KL20", "DE", "EUR", {}),
    ("LVMH Moet Hennessy Louis Vuitton SE", "969500FIF7GLA1WEDL01", "FR", "EUR", {
        2023: {"rev": "86153000000", "assets": "147245000000", "pl": "15174000000",
               "eq": "76633000000", "cfo": "22851000000"},
        2022: {"rev": "79184000000", "assets": "134563000000", "pl": "14084000000",
               "eq": "68526000000", "cfo": "19842000000"},
    }),
    ("TotalEnergies SE", "529900QNAI1XCGCDLD62", "FR", "USD", {
        2023: {"rev": "218945000000", "assets": "285282000000", "pl": "21384000000",
               "eq": "115262000000", "cfo": "40679000000"},
    }),
    ("Iberdrola SA", "5493006QMFDDMYWIAM13", "ES", "EUR", {
        2023: {"rev": "49336000000", "assets": "160330000000", "pl": "4803000000",
               "eq": "60890000000", "cfo": "11466000000"},
        2022: {"rev": "53949000000", "assets": "154618000000", "pl": "4339000000",
               "eq": "56209000000", "cfo": "9411000000"},
    }),
    # A bank: reports interest / fee income rather than a single "Revenue" line.
    ("Banco Santander SA", "549300F0WLW5CWKUWM90", "ES", "EUR", {
        2023: {"int": "90123000000", "fee": "12057000000", "assets": "1797062000000",
               "pl": "11076000000", "eq": "97627000000", "cfo": "38400000000"},
        2022: {"int": "70000000000", "fee": "11790000000", "assets": "1734659000000",
               "pl": "9605000000", "eq": "97585000000", "cfo": "21500000000"},
    }),
    ("Novo Nordisk A/S", "549300DAQ1UW3ZDN0M43", "DK", "DKK", {
        2023: {"rev": "232261000000", "assets": "290719000000", "pl": "83683000000",
               "eq": "101750000000", "cfo": "84604000000"},
        2022: {"rev": "176954000000", "assets": "213133000000", "pl": "55525000000",
               "eq": "83486000000", "cfo": "68737000000"},
    }),
    ("Nokia Oyj", "549300A0JPRWG1KI7U06", "FI", "EUR", {
        2023: {"rev": "22258000000", "assets": "38788000000", "pl": "615000000",
               "eq": "20605000000", "cfo": "1801000000"},
    }),
    ("Enel SpA", "WOCMU6HCI0OJWNPRZS33", "IT", "EUR", {
        2023: {"rev": "95560000000", "assets": "220681000000", "pl": "3446000000",
               "eq": "48065000000", "cfo": "15328000000"},
    }),
    ("ASML Holding NV", "724500Y6DUVHQD6OXN27", "NL", "EUR", {
        2023: {"rev": "27558500000", "assets": "40800800000", "pl": "7838700000",
               "eq": "13068700000", "cfo": "6553900000"},
    }),
]

BASE_URL = "https://filings.xbrl.org"


def main() -> None:
    conn = db.connect()
    db.init_db(conn)

    for name, lei, country, currency, years in DEMO:
        entity_id = db.upsert_entity(conn, name, lei, country)
        for year, facts in years.items():
            reporting_date = f"{year}-12-31"
            slug = f"{lei}-{year}-12-31-ESEF-{country}-0"
            db.upsert_filing(
                conn,
                entity_id=entity_id,
                reporting_date=reporting_date,
                filing_url=f"{BASE_URL}/{slug}/reports/ixbrl-viewer.html",
                xbrl_json_url=f"{BASE_URL}/{slug}/reports/report.json",
                country=country,
                validation_message_count=(year % 7),
            )
            for short, value in facts.items():
                db.upsert_fact(
                    conn,
                    entity_id=entity_id,
                    reporting_date=reporting_date,
                    concept=CONCEPTS[short],
                    value=value,
                    currency=currency,
                )

    # Illustrative ECB annual-average rates (units per EUR) so the comparator's
    # currency conversion works offline.
    demo_fx = {
        "USD": {"2022": 1.0530, "2023": 1.0813},
        "DKK": {"2022": 7.4400, "2023": 7.4508},
    }
    for currency, by_year in demo_fx.items():
        for year, rate in by_year.items():
            db.upsert_fx_rate(conn, currency, year, rate)

    n_e = conn.execute("SELECT COUNT(*) AS n FROM entities").fetchone()["n"]
    n_f = conn.execute("SELECT COUNT(*) AS n FROM filings").fetchone()["n"]
    n_x = conn.execute("SELECT COUNT(*) AS n FROM financial_facts").fetchone()["n"]
    conn.close()
    print(f"Demo data written to {db.DB_PATH}: {n_e} entities, {n_f} filings, {n_x} facts")
    print("NOTE: illustrative figures only — run the real pipeline for accurate data.")


if __name__ == "__main__":
    main()
