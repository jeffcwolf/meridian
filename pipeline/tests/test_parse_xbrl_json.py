"""Unit tests for the pure XBRL-JSON parsing helpers.

These cover the fact-selection logic (latest period, consolidated-only,
standard-vs-extension concepts) without touching the network or the database.
"""

from __future__ import annotations

import parse_xbrl_json as p


def _fact(concept, period, value, unit="iso4217:EUR", **extra_dims):
    dims = {"concept": concept, "period": period, "unit": unit}
    dims.update(extra_dims)
    return {"dimensions": dims, "value": value}


def test_period_end_returns_end_of_a_duration():
    assert p._period_end("2023-01-01/2023-12-31") == "2023-12-31"


def test_period_end_returns_an_instant_unchanged():
    assert p._period_end("2023-12-31") == "2023-12-31"


def test_period_end_of_empty_string_is_empty():
    assert p._period_end("") == ""


def test_currency_strips_the_iso4217_prefix():
    assert p._currency("iso4217:EUR") == "EUR"


def test_currency_of_none_is_none():
    assert p._currency(None) is None


def test_extract_facts_keeps_the_latest_reporting_year():
    report = {
        "facts": {
            "prior": _fact("ifrs-full:Revenue", "2022-01-01/2022-12-31", "100"),
            "current": _fact("ifrs-full:Revenue", "2023-01-01/2023-12-31", "200"),
        }
    }
    assert p.extract_facts(report) == {"ifrs-full:Revenue": ("200", "EUR")}


def test_extract_facts_skips_segment_breakdowns():
    # A fact carrying a member/axis dimension is a segment split, not the
    # consolidated top line, and must be ignored.
    report = {
        "facts": {
            "consolidated": _fact("ifrs-full:Revenue", "2023-01-01/2023-12-31", "200"),
            "segment": _fact(
                "ifrs-full:Revenue",
                "2023-01-01/2023-12-31",
                "50",
                **{"ifrs-full:SegmentsAxis": "seg:Europe"},
            ),
        }
    }
    assert p.extract_facts(report) == {"ifrs-full:Revenue": ("200", "EUR")}


def test_extract_facts_ignores_untargeted_concepts_and_null_values():
    report = {
        "facts": {
            "offtag": _fact("ifrs-full:NotTracked", "2023-01-01/2023-12-31", "999"),
            "null": _fact("ifrs-full:Assets", "2023-01-01/2023-12-31", None),
        }
    }
    assert p.extract_facts(report) == {}


def test_extract_extensions_selects_company_specific_monetary_tags():
    report = {
        "facts": {
            "standard": _fact("ifrs-full:Revenue", "2023-01-01/2023-12-31", "200"),
            "extension": _fact("ext:PatrimonioNetto", "2023-01-01/2023-12-31", "500"),
        }
    }
    assert p.extract_extensions(report) == {
        "ext:PatrimonioNetto": ("ext", "500", "EUR")
    }


def test_extract_extensions_skips_non_monetary_extension_facts():
    # No unit -> not a monetary figure -> excluded from the extension table.
    report = {
        "facts": {
            "headcount": _fact(
                "ext:Employees", "2023-01-01/2023-12-31", "1000", unit=None
            ),
        }
    }
    assert p.extract_extensions(report) == {}
