"""Guard: monster registration in etc/monsters.toml stays in sync with all
5 catalogues (ids.rs, names.py MonsterName, names.py _MONSTER_MAP,
scenarios.py MONSTER_STARTING_HP, bench/__init__.py Encounter).

When adding a new monster:
  1. Add a [[monster]] entry to etc/monsters.toml first
  2. Add the Rust variant to ids.rs + AI to monsters.rs
  3. Run this test — it will tell you what Python catalogues are missing
"""

import tomllib

from sts_sim._sts_sim import all_monster_names
from sts_sim.bench import Encounter
from sts_sim.sim.names import MonsterName, _MONSTER_MAP
from sts_sim.sim.scenarios import MONSTER_STARTING_HP


def _toml_monsters():
    with open("etc/monsters.toml", "rb") as f:
        data = tomllib.load(f)
    return data.get("monster", [])


def _toml_by_display() -> dict[str, dict]:
    return {m["display_name"]: m for m in _toml_monsters()}


def _toml_by_variant() -> dict[str, dict]:
    return {m["variant"]: m for m in _toml_monsters()}


def _rust_monsters() -> set[str]:
    return set(all_monster_names())


def _monster_name_values() -> set[str]:
    return {m.value for m in MonsterName}


def _hp_keys() -> set[str]:
    return set(MONSTER_STARTING_HP.keys())


def _encounter_values() -> set[str]:
    return {e.value for e in Encounter}


# Multi-monster encounters that don't correspond to a single TOML monster entry.
_MULTI_MONSTER_ENCOUNTERS = {
    "slimes-weak",
    "slimes-weak-twig",
    "inklets",
    "ruby-raiders",
    "the-kin",
}


# ── Rust ↔ TOML ──────────────────────────────────────────────────────────


def test_toml_covers_all_rust_monsters():
    toml = _toml_by_display()
    rust = _rust_monsters()
    missing = rust - set(toml)
    assert not missing, (
        "Monsters in Rust (MonsterId::all()) but missing from etc/monsters.toml:\n"
        + "\n".join(f"  {m}" for m in sorted(missing))
    )


def test_rust_covers_all_toml_monsters():
    toml = _toml_by_display()
    rust = _rust_monsters()
    missing = set(toml) - rust
    assert not missing, (
        "Monsters in etc/monsters.toml but missing from Rust (MonsterId::all()):\n"
        + "\n".join(f"  {m}" for m in sorted(missing))
    )


# ── MonsterName ↔ TOML ───────────────────────────────────────────────────


def test_monster_name_enum_has_toml_entry():
    toml = _toml_by_display()
    names = _monster_name_values()
    missing = names - set(toml)
    assert not missing, (
        "MonsterName values without a matching etc/monsters.toml entry:\n"
        + "\n".join(f"  {m}" for m in sorted(missing))
    )


def test_toml_monsters_with_bridge_names_have_monster_name():
    toml = _toml_by_display()
    names = _monster_name_values()
    missing = {
        m["display_name"]
        for m in toml.values()
        if m.get("bridge_names") and m["display_name"] not in names
    }
    assert not missing, (
        "Monsters in etc/monsters.toml with bridge_names but no MonsterName entry:\n"
        + "\n".join(f"  {m}" for m in sorted(missing))
        + "\n\nAdd them to the MonsterName enum in python/sts_sim/names.py."
    )


# ── _MONSTER_MAP ↔ TOML ─────────────────────────────────────────────────


def test_monster_map_values_have_toml_entry():
    toml = _toml_by_display()
    mapped_names = {v for v in _MONSTER_MAP.values() if v in _monster_name_values()}
    missing = mapped_names - set(toml)
    assert not missing, (
        "_MONSTER_MAP values without a matching etc/monsters.toml entry:\n"
        + "\n".join(f"  {m}" for m in sorted(missing))
    )


def test_toml_monsters_have_bridge_map_entry():
    toml = _toml_by_display()
    mapped_values = set(_MONSTER_MAP.values())
    missing = {
        m["display_name"]
        for m in toml.values()
        if m.get("bridge_names") and m["display_name"] not in mapped_values
    }
    assert not missing, (
        "Monsters in etc/monsters.toml with bridge_names missing from _MONSTER_MAP:\n"
        + "\n".join(f"  {m}" for m in sorted(missing))
        + "\n\nAdd them to _MONSTER_MAP in python/sts_sim/names.py."
    )


# ── MONSTER_STARTING_HP ↔ TOML ──────────────────────────────────────────


def test_hp_table_covers_all_toml_monsters_with_hp():
    toml = _toml_by_display()
    hp_keys = _hp_keys()
    missing = {
        m["display_name"]
        for m in toml.values()
        if m.get("hp") is not None and m["display_name"] not in hp_keys
    }
    assert not missing, (
        "Monsters in etc/monsters.toml with hp but missing from "
        "MONSTER_STARTING_HP dict:\n"
        + "\n".join(f"  {m}" for m in sorted(missing))
        + "\n\nAdd them to MONSTER_STARTING_HP in python/sts_sim/scenarios.py."
    )


def test_hp_table_entries_have_toml_hp():
    toml = _toml_by_display()
    hp_keys = _hp_keys()
    tom_with_hp = {m["display_name"] for m in toml.values() if m.get("hp") is not None}
    orphaned = hp_keys - tom_with_hp
    assert not orphaned, (
        "MONSTER_STARTING_HP entries without a matching etc/monsters.toml entry:\n"
        + "\n".join(f"  {m}" for m in sorted(orphaned))
    )


# ── Encounter ↔ TOML ────────────────────────────────────────────────────


def test_encounter_enum_covers_all_toml_monsters_with_encounter():
    toml = _toml_by_display()
    encounters = _encounter_values()
    missing = {
        m["encounter"]
        for m in toml.values()
        if m.get("encounter") and m["encounter"] not in encounters
    }
    assert not missing, (
        "Monsters in etc/monsters.toml with encounter but missing from "
        "Encounter enum:\n"
        + "\n".join(f"  {m}" for m in sorted(missing))
        + "\n\nAdd them to Encounter in python/sts_sim/bench/__init__.py."
    )


def test_single_monster_encounters_have_toml_entry():
    toml = _toml_by_display()
    toml_encounters = {m["encounter"] for m in toml.values() if m.get("encounter")}
    all_encounters = _encounter_values()
    single = all_encounters - _MULTI_MONSTER_ENCOUNTERS
    orphaned = single - toml_encounters
    assert not orphaned, (
        "Single-monster Encounter values without a matching etc/monsters.toml entry:\n"
        + "\n".join(f"  {m}" for m in sorted(orphaned))
        + "\n\nAdd the missing [[monster]] entry or add the encounter key "
        "to its existing entry."
    )
