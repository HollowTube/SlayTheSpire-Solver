"""Guard: the Python name enums agree with the live Rust binary.

Both are built from the registry rows in crates/sts-ids/src/registry.rs, so a failure here
means the extension and the Python package are out of step (rebuild).
"""

from sts_sim._sts_sim import all_card_names, all_monster_names
from sts_sim.sim.names import CardName, MonsterName, card, _MONSTER_MAP


# ── helpers ──────────────────────────────────────────────────────────────────


def _cards_from_rust() -> set[str]:
    """All card display names from the live Rust binary (CardId::all())."""
    return set(all_card_names())


def _monsters_from_rust() -> set[str]:
    """All monster display names from the live Rust binary (MonsterId::all())."""
    return set(all_monster_names())


# ── card tests ───────────────────────────────────────────────────────────────


def test_card_name_enum_covers_all_rust_cards():
    """Every card in cards.rs must have a CardName entry."""
    rust_cards = _cards_from_rust()
    enum_values = {c.value for c in CardName}
    missing = rust_cards - enum_values
    assert not missing, (
        "Cards in cards.rs but missing from CardName enum in names.py:\n"
        + "\n".join(f"  {c}" for c in sorted(missing))
        + '\n\nRebuild the extension: uv pip install -e ".[dev]".'
    )


def test_card_name_enum_has_no_stale_entries():
    """Every CardName value must exist in cards.rs (no orphans)."""
    rust_cards = _cards_from_rust()
    # Status cards are not in card_data() — skip them
    status_cards = {"Dazed", "Slimed", "Wound", "Burn"}
    for c in CardName:
        if c.value in status_cards:
            continue
        assert c.value in rust_cards, (
            f"CardName.{c.name} = {c.value!r} is not in cards.rs card_data(). "
            "Remove its row from crates/sts-ids/src/registry.rs or add it to cards.rs."
        )


def test_card_normalisation_round_trips():
    """All CardName values survive a bridge-name round-trip."""
    for c in CardName:
        bridge_with_suffix = c.value.replace(" ", "") + "Ironclad"
        bridge_bare = c.value.replace(" ", "")
        result_w = card(bridge_with_suffix)
        result_b = card(bridge_bare)
        assert result_w == c.value or result_b == c.value, (
            f"CardName.{c.name} = {c.value!r} doesn't round-trip through names.card(). "
            f"Got {result_w!r} (with suffix) or {result_b!r} (bare). "
            "Add a bridge alias to its row in crates/sts-ids/src/registry.rs."
        )


# ── monster tests ─────────────────────────────────────────────────────────────


def test_monster_name_enum_covers_all_rust_monsters():
    """Every monster in ids.rs must have a MonsterName entry."""
    rust_monsters = _monsters_from_rust()
    enum_values = {m.value for m in MonsterName}
    missing = rust_monsters - enum_values
    assert not missing, (
        "Monsters in ids.rs but missing from MonsterName enum in names.py:\n"
        + "\n".join(f"  {m}" for m in sorted(missing))
        + '\n\nRebuild the extension: uv pip install -e ".[dev]".'
    )


def test_monster_name_enum_has_no_stale_entries():
    """Every MonsterName value must exist in ids.rs (no orphans)."""
    rust_monsters = _monsters_from_rust()
    for m in MonsterName:
        assert m.value in rust_monsters, (
            f"MonsterName.{m.name} = {m.value!r} is not in MonsterId::all() in ids.rs. "
            "Remove its row from crates/sts-ids/src/registry.rs."
        )


def test_monster_name_enum_covered_by_bridge_map():
    """Every MonsterName must have at least one entry in _MONSTER_MAP."""
    covered = set(_MONSTER_MAP.values())
    for m in MonsterName:
        assert m.value in covered, (
            f"MonsterName.{m.name} = {m.value!r} has no entry in _MONSTER_MAP. "
            "Add `bridge = [...]` to its row in crates/sts-ids/src/registry.rs."
        )
