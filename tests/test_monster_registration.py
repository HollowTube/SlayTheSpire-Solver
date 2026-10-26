"""Guard: the hand-written per-monster Python code covers every registry row.

A monster is registered by one row in ``crates/sts-ids/src/registry.rs``.
``cargo check`` then names the AI functions in ``src/monsters.rs`` that need an
arm; these tests name the Python pieces that are still missing.
"""

from sts_sim.bench import _SCENARIOS, Encounter
from sts_sim.sim._registry import (
    MONSTER_ROWS,
    MULTI_MONSTER_ENCOUNTERS,
    encounter_member,
)
from sts_sim.sim.names import _MONSTER_MAP, MonsterName
from sts_sim.sim.scenarios import MONSTER_STARTING_HP


def _missing(lines: list[str], fix: str) -> str:
    return "\n".join(f"  {line}" for line in sorted(lines)) + f"\n\n{fix}"


def test_every_encounter_row_has_a_scenario_factory():
    missing = [
        f"{row['display']} (encounter {row['encounter']!r})"
        for row in MONSTER_ROWS
        if row["encounter"] and Encounter(row["encounter"]) not in _SCENARIOS
    ]
    assert not missing, "Registry monsters with no scenario factory:\n" + _missing(
        missing,
        "Add an ironclad_starter_deck_vs_* factory in python/sts_sim/sim/scenarios.py "
        "and its Encounter entry in _SCENARIOS (python/sts_sim/bench/__init__.py).",
    )


def test_every_multi_monster_encounter_has_a_scenario_factory():
    missing = [k for k in MULTI_MONSTER_ENCOUNTERS if Encounter(k) not in _SCENARIOS]
    assert not missing, "Multi-monster encounters with no scenario factory:\n" + (
        _missing(missing, "Add them to _SCENARIOS in python/sts_sim/bench/__init__.py.")
    )


def test_every_scenario_is_a_registry_encounter():
    known = {row["encounter"] for row in MONSTER_ROWS if row["encounter"]}
    known |= set(MULTI_MONSTER_ENCOUNTERS)
    orphaned = [e.value for e in _SCENARIOS if e.value not in known]
    assert not orphaned, "_SCENARIOS keys with no registry encounter:\n" + _missing(
        orphaned,
        "Set `encounter = ...` on the monster's row in crates/sts-ids/src/registry.rs, "
        "or list a multi-monster key in MULTI_MONSTER_ENCOUNTERS "
        "(python/sts_sim/sim/_registry.py).",
    )


def test_encounter_member_names_follow_their_keys():
    for e in Encounter:
        assert e.name == encounter_member(e.value)


def test_every_hp_row_has_a_starting_hp():
    missing = [
        row["display"]
        for row in MONSTER_ROWS
        if row["hp"] is not None and row["display"] not in MONSTER_STARTING_HP
    ]
    assert not missing, (
        "Registry monsters with hp missing from MONSTER_STARTING_HP:\n"
        + (_missing(missing, "See python/sts_sim/sim/scenarios.py."))
    )


def test_every_monster_has_a_bridge_name():
    mapped = set(_MONSTER_MAP.values())
    missing = [m.value for m in MonsterName if m.value not in mapped]
    assert not missing, "MonsterName values with no bridge name:\n" + _missing(
        missing, "Add `bridge = [...]` to the row in crates/sts-ids/src/registry.rs."
    )
