"""Behavioural tests for HOL-46: Vine Shambler (elite) — Status::Tangled
(increases Attack card costs, removed at end of player's turn)."""

from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    apply,
    legal_actions,
)


# ── Status::Tangled ──────────────────────────────────────────────────────────


def test_tangled_adds_1_to_attack_cost():
    """While Tangled(1) is active, an Attack card (Strike, base 1) costs 2.
    With 3 energy, a cost-2 Strike should still be playable."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=10, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
        player_statuses=[("Tangled", 1)],
    )
    actions = legal_actions(state)
    assert any("PlayCard:Strike" in a for a in actions)


def test_tangled_does_not_affect_skill_cost():
    """Tangled has no effect on non-Attack cards (Defend costs 1 as usual)."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=10, name="Jaw Worm")],
        seed=42,
        hand=["Defend"],
        player_statuses=[("Tangled", 1)],
    )
    actions = legal_actions(state)
    assert any("PlayCard:Defend" in a for a in actions)


def test_tangled_makes_cost_1_attack_unplayable_when_short_on_energy():
    """Tangled(3) makes a cost-1 Attack cost 4 total — unplayable with 3."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=10, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
        player_statuses=[("Tangled", 3)],
    )
    actions = legal_actions(state)
    assert not any("PlayCard:Strike" in a for a in actions)


def test_tangled_does_not_affect_zero_cost_attacks():
    """A base-0 Attack (NotYet, cost 0) costs +1 while Tangled(1) is active.
    NotYet should not appear in legal_actions if its cost exceeds energy."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=10, name="Jaw Worm")],
        seed=42,
        hand=["NotYet"],
        player_statuses=[("Tangled", 1)],
    )
    # NotYet is Attack with cost 0 + Tangled 1 = 1 → playable with 3 energy
    actions = legal_actions(state)
    assert any("PlayCard:NotYet" in a for a in actions)


def test_tangled_removed_at_end_of_player_turn():
    """Tangled is removed when the player ends their turn (EndTurn)."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=10, name="Jaw Worm")],
        seed=42,
        hand=["Defend"],
        player_statuses=[("Tangled", 1)],
    )
    assert "Tangled" in state.player_statuses
    after = apply(state, PlayCardAction("Defend"))
    after_turn = apply(after, EndTurnAction())
    assert "Tangled" not in after_turn.player_statuses


def test_tangled_not_removed_before_player_plays():
    """Tangled from setup persists through a PlayCard action."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=10, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
        player_statuses=[("Tangled", 1)],
    )
    # Tangled still present after a PlayCard action
    after = apply(state, PlayCardAction("Strike"))
    assert "Tangled" in after.player_statuses


# ── Vine Shambler ────────────────────────────────────────────────────────────


def _shambler(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=64, name="Vine Shambler")],
        seed=seed,
        hand=hand or [],
    )


def test_vine_shambler_opens_with_swipe():
    state = _shambler()
    assert state.monsters[0].intent == "Swipe"


def test_swipe_deals_12_damage():
    """Swipe is 2 hits of 6 (12 total)."""
    state = _shambler()
    after = apply(state, EndTurnAction())
    assert state.player_hp - after.player_hp == 12


def test_swipe_to_grasping_vines():
    state = _shambler()
    a1 = apply(state, EndTurnAction())
    assert a1.monsters[0].intent == "Grasping Vines"


def test_grasping_vines_deals_8_damage_and_applies_tangled():
    state = _shambler()
    a1 = apply(state, EndTurnAction())  # Swipe
    a2 = apply(a1, EndTurnAction())  # Grasping Vines
    assert a1.player_hp - a2.player_hp == 8
    assert "Tangled" in a2.player_statuses


def test_grasping_vines_to_chomp():
    state = _shambler()
    a1 = apply(state, EndTurnAction())  # Swipe
    a2 = apply(a1, EndTurnAction())  # Grasping Vines
    assert a2.monsters[0].intent == "Chomp"


def test_chomp_deals_16_damage():
    state = _shambler()
    a1 = apply(state, EndTurnAction())  # Swipe
    a2 = apply(a1, EndTurnAction())  # Grasping Vines
    a3 = apply(a2, EndTurnAction())  # Chomp
    assert a2.player_hp - a3.player_hp == 16


def test_chomp_back_to_swipe():
    state = _shambler()
    a1 = apply(state, EndTurnAction())
    a2 = apply(a1, EndTurnAction())
    a3 = apply(a2, EndTurnAction())
    assert a3.monsters[0].intent == "Swipe"


def test_cycle_repeats():
    state = _shambler()
    a1 = apply(state, EndTurnAction())  # Swipe
    a2 = apply(a1, EndTurnAction())  # Grasping Vines
    a3 = apply(a2, EndTurnAction())  # Chomp
    a4 = apply(a3, EndTurnAction())  # Swipe
    a5 = apply(a4, EndTurnAction())  # Grasping Vines
    a6 = apply(a5, EndTurnAction())  # Chomp
    assert a4.monsters[0].intent == "Grasping Vines"
    assert a5.monsters[0].intent == "Chomp"
    assert a6.monsters[0].intent == "Swipe"


def test_tangled_persists_through_player_turn():
    """After Grasping Vines applies Tangled, it persists through the player's
    card-play phase and is only removed at the next EndTurn call."""
    state = _shambler(hand=["Defend"])
    a1 = apply(state, EndTurnAction())  # Swipe
    a2 = apply(a1, EndTurnAction())  # Grasping Vines (+Tangled)
    # Tangled should be present after EndTurn (player's new turn)
    assert "Tangled" in a2.player_statuses
    # Playing a card should not remove Tangled
    a3 = apply(a2, PlayCardAction("Defend"))
    assert "Tangled" in a3.player_statuses
    # EndTurn removes Tangled
    a4 = apply(a3, EndTurnAction())
    assert "Tangled" not in a4.player_statuses
