"""The registry-derived Python surface matches what the TOML catalogues produced.

``fixtures/registry_snapshot.json`` was recorded from the generated modules
before the registry moved into ``crates/sts-ids``. The only intended
differences are listed in ``_ADDED_*`` / ``_REMOVED_*`` below.
"""

import json
from enum import Enum
from pathlib import Path

import pytest

from sts_sim import RunState, run_apply, run_legal_actions
from sts_sim.bench import Encounter
from sts_sim.sim import names
from sts_sim.sim.scenarios import MONSTER_STARTING_HP

_SNAPSHOT = json.loads(
    (Path(__file__).parent / "fixtures" / "registry_snapshot.json").read_text()
)

# Burn became a registry row (it was a hand-written _BRIDGE_CARD_MAP extra).
_ADDED_CARD_NAMES = [["BURN", "Burn"]]
_ADDED_CARD_STS2_IDS = {"Burn": "BURN"}
_ADDED_BRIDGE_CARDS = {
    f"Burn{suffix}": "Burn"
    for suffix in ("Ironclad", "Silent", "Defect", "Watcher", "Huntress")
}
# Unconfirmed bridge names with no registry row; monster() still maps them by
# its CamelCase fallback.
_REMOVED_MONSTER_MAP_KEYS = {"AcidSlime_L", "AcidSlime_M", "AcidSlime_S"}


def _plain(d: dict) -> dict:
    return {
        getattr(k, "value", k): v.value if isinstance(v, Enum) else v
        for k, v in d.items()
    }


def test_card_name_members_and_order():
    actual = [[m.name, m.value] for m in names.CardName]
    assert actual == _SNAPSHOT["CardName"] + _ADDED_CARD_NAMES


def test_monster_name_members_and_order():
    actual = [[m.name, m.value] for m in names.MonsterName]
    assert actual == _SNAPSHOT["MonsterName"]


def test_status_name_members_and_bridge_classes():
    actual = [[m.name, m.value, list(m.bridge_classes)] for m in names.StatusName]
    assert actual == _SNAPSHOT["StatusName"]


def test_encounter_members():
    assert {m.name: m.value for m in Encounter} == _SNAPSHOT["Encounter"]


def test_monster_map():
    expected = {
        k: v
        for k, v in _SNAPSHOT["_MONSTER_MAP"].items()
        if k not in _REMOVED_MONSTER_MAP_KEYS
    }
    assert _plain(names._MONSTER_MAP) == expected
    assert all(isinstance(v, names.MonsterName) for v in names._MONSTER_MAP.values())


@pytest.mark.parametrize("bridge_name", sorted(_REMOVED_MONSTER_MAP_KEYS))
def test_dropped_monster_names_still_normalise_by_fallback(bridge_name):
    assert names.monster(bridge_name) == _SNAPSHOT["_MONSTER_MAP"][bridge_name]


def test_bridge_card_map():
    expected = _SNAPSHOT["_BRIDGE_CARD_MAP"] | _ADDED_BRIDGE_CARDS
    assert _plain(names._BRIDGE_CARD_MAP) == expected


def test_card_sts2_ids():
    expected = _SNAPSHOT["CARD_STS2_ID"] | _ADDED_CARD_STS2_IDS
    assert names.CARD_STS2_ID == expected


def test_monster_starting_hp():
    assert _plain(MONSTER_STARTING_HP) == _SNAPSHOT["MONSTER_STARTING_HP"]


@pytest.mark.parametrize("seed", sorted(int(s) for s in _SNAPSHOT["reward_offers"]))
def test_seeded_reward_offers_are_unchanged(seed):
    run = RunState(
        seed=seed,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=["Nibbit"],
    )
    after = run_apply(run, "ResolveCombat")
    offered = sorted(a for a in run_legal_actions(after) if a.startswith("Take:"))
    assert offered == _SNAPSHOT["reward_offers"][str(seed)]
