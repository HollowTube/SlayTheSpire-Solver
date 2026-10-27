"""The sim's Ironclad card data against the fixture extracted from the game files.

Re-extract after a game patch: see README "Card parity".
"""

from __future__ import annotations

import copy

import pytest

from sts_sim import card_spec
from sts_sim.parity.fixture import GAME_VERSION, fixture_path, load_fixture
from sts_sim.parity.mapping import KNOWN_FAILURES, check
from sts_sim.parity.report import registry_join

FIXTURE = load_fixture(fixture_path(GAME_VERSION))
JOINED, MISSING = registry_join(FIXTURE)


def test_fixture_covers_the_whole_pool() -> None:
    assert FIXTURE["game_version"] == GAME_VERSION
    assert FIXTURE["pool"] == "IroncladCardPool"
    assert len(FIXTURE["cards"]) == 87


@pytest.mark.parametrize("key", sorted(JOINED))
def test_card_matches_the_game(key: str) -> None:
    name = JOINED[key]
    unexpected, stale = check(
        FIXTURE["cards"][key], card_spec(name), card_spec(name + "+")
    )
    assert not unexpected, "\n".join(str(m) for m in unexpected)
    assert not stale, "\n".join(stale)


def test_every_known_failure_names_a_checked_card() -> None:
    checked = {FIXTURE["cards"][key]["class"] for key in JOINED}
    assert {c for c, _ in KNOWN_FAILURES} <= checked


def test_a_mismatch_names_card_field_game_and_sim_values() -> None:
    entry = copy.deepcopy(FIXTURE["cards"]["BASH"])
    entry["vars"]["Damage"]["base"] = 99
    entry["cost"] = 5
    unexpected, _ = check(entry, card_spec("Bash"), card_spec("Bash+"))
    assert [str(m) for m in unexpected] == [
        "Bash: cost: game 5 != sim 2",
        "Bash: var.Damage: game 99 != sim 8",
    ]


def test_a_known_failure_that_passes_is_reported_stale() -> None:
    known = {("Bash", "var.Damage"): "HOL-0"}
    unexpected, stale = check(
        FIXTURE["cards"]["BASH"], card_spec("Bash"), card_spec("Bash+"), known
    )
    assert not unexpected
    assert stale == [
        "Bash: var.Damage matches now; remove it from KNOWN_FAILURES (HOL-0)"
    ]
