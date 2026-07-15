"""Guard: CardName enum in names.py stays in sync with the live Rust binary.

When a new card is added to ids.rs/cards.rs, this test fails until the
corresponding entry is added to CardName in python/sts_sim/names.py.
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
        + "\n\nAdd them to the CardName enum in python/sts_sim/names.py."
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
            "Remove it from the enum or add it to cards.rs."
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
            "Add an entry to _CARD_OVERRIDES in names.py."
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
        + "\n\nAdd them to the MonsterName enum in python/sts_sim/names.py."
    )


def test_monster_name_enum_has_no_stale_entries():
    """Every MonsterName value must exist in ids.rs (no orphans)."""
    rust_monsters = _monsters_from_rust()
    for m in MonsterName:
        assert m.value in rust_monsters, (
            f"MonsterName.{m.name} = {m.value!r} is not in MonsterId::all() in ids.rs. "
            "Remove it from the enum or add it to ids.rs."
        )


def test_monster_name_enum_covered_by_bridge_map():
    """Every MonsterName must have at least one entry in _MONSTER_MAP."""
    covered = set(_MONSTER_MAP.values())
    for m in MonsterName:
        assert m.value in covered, (
            f"MonsterName.{m.name} = {m.value!r} has no entry in _MONSTER_MAP. "
            "Add the bridge class name → sim name mapping in names.py."
        )
