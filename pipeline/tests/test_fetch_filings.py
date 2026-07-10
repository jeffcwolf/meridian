"""Unit tests for the pure name-normalisation helpers used to match seed names
to the actual ESEF filer on filings.xbrl.org."""

from __future__ import annotations

import fetch_filings as ff


def test_normalize_lowercases_and_strips_accents():
    assert ff._normalize("Telefónica") == "telefonica"
    assert ff._normalize("SIEMENS") == "siemens"


def test_tokens_drop_legal_forms_and_punctuation():
    assert ff._tokens("Siemens AG") == {"siemens"}


def test_tokens_keep_distinctive_words_like_group():
    # "Groep" is kept (it disambiguates ING Groep from ING Bank); "N.V." is a
    # legal form and its letters are dropped as single characters.
    assert ff._tokens("ING Groep N.V.") == {"ing", "groep"}


def test_tokens_drop_single_characters():
    assert ff._tokens("A. P. Moller") == {"moller"}


def test_tokens_combine_accent_stripping_and_stopword_removal():
    assert ff._tokens("Telefónica SA") == {"telefonica"}
