"""Behavioural tests for HOL-32: Bygone Effigy — Status::Slow + cards_played_this_turn."""

from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
)


def _strike_and_target(state):
    """Play Strike and target monster 0, returning the resulting state."""
    return apply(apply(state, PlayCardAction("Strike")), SelectTargetAction(0))


# ── Status::Slow — damage scaling ────────────────────────────────────────────


def test_slow_scales_attack_damage_by_cards_played():
    """Each Strike counts itself: 1st at ~1.1x (6), 2nd at ~1.2x (7)."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=100, name="Bygone Effigy", statuses=[("Slow", 1)])],
        seed=42,
        hand=["Strike", "Strike"],
    )
    a1 = _strike_and_target(state)
    # 1 card this turn (counting itself) → 1.1x → floor(6.6) = 6
    assert 100 - a1.monsters[0].hp == 6, f"expected 6, got {100 - a1.monsters[0].hp}"

    a2 = _strike_and_target(a1)
    # 2 cards this turn → 1.2x → floor(7.2) = 7
    assert a1.monsters[0].hp - a2.monsters[0].hp == 7, (
        f"expected 7, got {a1.monsters[0].hp - a2.monsters[0].hp}"
    )


def test_slow_scales_with_more_cards():
    """At 3 cards played this turn → 1.3x → Strike deals 7 (floor(6*1.3)=7)."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=100, name="Bygone Effigy", statuses=[("Slow", 1)])],
        seed=42,
        hand=["Defend", "Defend", "Strike"],
    )
    a1 = apply(state, PlayCardAction("Defend"))  # 1 card
    a2 = apply(a1, PlayCardAction("Defend"))  # 2 cards
    a3 = _strike_and_target(a2)  # 3 cards → 1.3x → floor(7.8) = 7
    assert 100 - a3.monsters[0].hp == 7, f"expected 7, got {100 - a3.monsters[0].hp}"


def test_slow_does_not_scale_non_attack_damage():
    """Slow only applies to Attack cards."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, name="Bygone Effigy", statuses=[("Slow", 1)])],
        seed=42,
        hand=["Defend", "Strike"],
    )
    a1 = apply(state, PlayCardAction("Defend"))  # 1 card, no damage
    assert a1.monsters[0].hp == 50

    a2 = _strike_and_target(a1)  # 2 cards → 1.2x → floor(7.2) = 7
    assert 50 - a2.monsters[0].hp == 7


def test_slow_requires_target_to_have_it():
    """Monsters without Slow deal normal damage."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
    )
    a1 = _strike_and_target(state)
    assert 50 - a1.monsters[0].hp == 6


# ── Bygone Effigy — move pool ────────────────────────────────────────────────


def _effigy(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=132, name="Bygone Effigy")],
        seed=seed,
        hand=hand or [],
    )


def test_bygone_effigy_opens_with_sleep():
    state = _effigy()
    assert state.monsters[0].intent == "Sleep"


def test_sleep_does_no_damage_turn_1():
    """Sleep is a no-op — no damage, no statuses."""
    state = _effigy()
    a1 = apply(state, EndTurnAction())
    assert a1.player_hp == 80
    assert a1.monsters[0].hp == 132


def test_sleep_to_wake():
    """After Sleep, intent becomes Wake."""
    state = _effigy()
    a1 = apply(state, EndTurnAction())
    assert a1.monsters[0].intent == "Wake"


def test_wake_grants_10_strength():
    """Wake applies +10 Strength to the Effigy."""
    state = _effigy()
    a1 = apply(state, EndTurnAction())  # Sleep (no-op)
    a2 = apply(a1, EndTurnAction())  # Wake
    assert a2.monsters[0].strength == 10


def test_wake_to_slashes():
    """After Wake, intent becomes Slashes."""
    state = _effigy()
    a1 = apply(state, EndTurnAction())  # Sleep
    a2 = apply(a1, EndTurnAction())  # Wake
    assert a2.monsters[0].intent == "Slashes"


def test_slashes_deals_13_damage():
    """Slashes deals 13 damage (+10 Strength from Wake = 23)."""
    state = _effigy()
    a1 = apply(state, EndTurnAction())  # Sleep
    a2 = apply(a1, EndTurnAction())  # Wake (+10 Strength)
    a3 = apply(a2, EndTurnAction())  # Slashes (13 + 10 = 23)
    assert a2.player_hp - a3.player_hp == 23


def test_slashes_repeats_forever():
    """After the first Slashes, intent stays Slashes forever."""
    state = _effigy()
    a1 = apply(state, EndTurnAction())  # Sleep
    a2 = apply(a1, EndTurnAction())  # Wake
    a3 = apply(a2, EndTurnAction())  # Slashes
    a4 = apply(a3, EndTurnAction())  # Slashes
    a5 = apply(a4, EndTurnAction())  # Slashes
    assert a3.monsters[0].intent == "Slashes"
    assert a4.monsters[0].intent == "Slashes"
    assert a5.monsters[0].intent == "Slashes"
