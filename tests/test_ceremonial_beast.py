"""Tests for Ceremonial Beast boss (HOL-73).

Two-phase AI with Plow HP threshold and Ringing player debuff.
"""

from sts_sim import CombatState, EndTurnAction, Monster, apply
from sts_sim.scenarios import IRONCLAD_STARTING_DECK, PLAYER_STARTING_HP

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _cb_state(
    hp: int = 252, seed: int = 42, player_hp: int = PLAYER_STARTING_HP
) -> CombatState:
    """Return a CombatState with Ceremonial Beast as the only monster."""
    return CombatState(
        player_hp=player_hp,
        player_energy=3,
        monsters=[
            Monster(
                hp=hp,
                name="Ceremonial Beast",
            )
        ],
        seed=seed,
        deck=list(IRONCLAD_STARTING_DECK),
    )


# ---------------------------------------------------------------------------
# Phase 1 — Stamp + Plow loop
# ---------------------------------------------------------------------------


def test_opens_with_stamp():
    """CB's opening intent is always Stamp (no RNG)."""
    state = _cb_state(seed=7)
    assert state.monsters[0].intent == "Stamp"
    assert state.monsters[0].hp == 252


def test_stamp_applies_plow_no_damage():
    """Stamp applies Plow(150) to CB, deals no damage."""
    state = _cb_state()
    after = apply(state, EndTurnAction())
    cb = after.monsters[0]
    # Stamp should deal no damage
    assert after.player_hp == 80
    # CB should have Plow
    plow = [s for s in cb.statuses if s.startswith("Plow")]
    assert len(plow) == 1, f"expected Plow, got statuses: {cb.statuses}"


def test_plow_deals_18_and_gains_strength():
    """Plow deals 18 damage and gives CB +2 Strength."""
    state = _cb_state()
    # Turn 1: Stamp resolves (no damage, Plow applied)
    after_stamp = apply(state, EndTurnAction())
    assert after_stamp.monsters[0].intent == "Plow"
    # Turn 2: Plow resolves
    after_plow = apply(after_stamp, EndTurnAction())
    hp_lost = state.player_hp - after_plow.player_hp
    assert hp_lost == 18, f"Plow should deal 18 damage, dealt {hp_lost}"
    # CB should have +2 Strength
    cb = after_plow.monsters[0]
    str_stacks = [s for s in cb.statuses if "Strength" in s]
    assert len(str_stacks) > 0, f"expected Strength on CB, got: {cb.statuses}"


def test_plow_loops_indefinitely():
    """After Stamp, CB loops Plow forever — gaining +2 Str each cycle."""
    state = _cb_state(seed=1)
    # Stamp
    state = apply(state, EndTurnAction())
    # Plow × 3
    for i in range(1, 4):
        assert state.monsters[0].intent == "Plow", (
            f"turn {i}: expected Plow, got {state.monsters[0].intent}"
        )
        state = apply(state, EndTurnAction())
    # Still looping Plow
    assert state.monsters[0].intent == "Plow"


# ---------------------------------------------------------------------------
# Phase transition — Plow threshold
# ---------------------------------------------------------------------------


def test_plow_threshold_strips_strength_and_sets_stun():
    """When CB HP ≤ 150 after taking damage, strip Strength and set Stun."""
    from sts_sim import PlayCardAction, SelectTargetAction

    # Start CB at 151 HP so one Strike (6 dmg) triggers the threshold
    state = _cb_state(hp=151, seed=42)
    # Stamp → Plow(150) applied
    state = apply(state, EndTurnAction())
    # Plow → takes 18, CB gets +2 Str
    state = apply(state, EndTurnAction())
    # Player plays Strike on CB (6 dmg → 151 - 6 = 145 ≤ 150)
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    assert state.monsters[0].intent == "Stun", (
        f"expected Stun after threshold breach, got {state.monsters[0].intent}"
    )
    # Strength should be stripped
    cb = state.monsters[0]
    str_stacks = [s for s in cb.statuses if "Strength" in s]
    assert len(str_stacks) == 0, f"Strength should be stripped, got: {cb.statuses}"
    # Plow status should be removed (the idempotency guard)
    plow = [s for s in cb.statuses if s.startswith("Plow")]
    assert len(plow) == 0, f"Plow should be removed, got: {cb.statuses}"


def test_plow_threshold_does_not_fire_on_blocked_hit():
    """Plow threshold only fires on unblocked damage — blocked hits don't count."""
    from sts_sim import PlayCardAction

    # Lower threshold to make it easier to trigger
    state = _cb_state(hp=151, seed=42)
    # Stamp → Plow applied
    state = apply(state, EndTurnAction())
    # Plow → takes 18, CB has +2 Str, player at 62 HP
    state = apply(state, EndTurnAction())
    # Play Defend (5 block) then EndTurn — no damage done to CB
    state = apply(state, PlayCardAction("Defend"))
    state = apply(state, EndTurnAction())
    # Player's next turn: CB still has Plow, still telegraphing Plow
    assert state.monsters[0].intent == "Plow", (
        f"threshold should not fire without damage, got {state.monsters[0].intent}"
    )
    plow = [s for s in state.monsters[0].statuses if s.startswith("Plow")]
    assert len(plow) == 1, (
        f"Plow should still be present, got: {state.monsters[0].statuses}"
    )


def test_plow_threshold_fires_once():
    """Plow threshold should fire exactly once — Plow removal guards against
    re-trigger."""
    from sts_sim import PlayCardAction, SelectTargetAction

    # Start at 158: two Strikes needed, first triggers, second should NOT
    # re-trigger (Stun intent already set, Plow already removed)
    state = _cb_state(hp=158, seed=42)
    # Stamp
    state = apply(state, EndTurnAction())
    # Plow (CB gets +2 Str)
    state = apply(state, EndTurnAction())
    # Strike 1: 158 - 6 = 152 > 150 (no trigger yet)
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    assert state.monsters[0].hp == 152
    # Strike 2: 152 - 6 = 146 ≤ 150 → trigger
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    assert state.monsters[0].intent == "Stun"
    assert state.monsters[0].hp == 146
    # Strike 3: should NOT trigger again (no Plow status, no intent change)
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    assert state.monsters[0].intent == "Stun", (
        f"threshold fired twice! Intent should stay Stun, got {state.monsters[0].intent}"
    )
    assert state.monsters[0].hp == 140


# ---------------------------------------------------------------------------
# Phase 2 — Beast Cry cycle after Plow threshold
# ---------------------------------------------------------------------------


def test_phase2_stun_then_beast_cry_cycle():
    """After Stun, CB enters Phase 2: Beast Cry → Stomp → Crush → loop."""
    from sts_sim import PlayCardAction, SelectTargetAction

    # Trigger threshold: start at 151, one Strike triggers
    state = _cb_state(hp=151, seed=42)
    # Stamp → Plow
    state = apply(state, EndTurnAction())
    state = apply(state, EndTurnAction())
    # Trigger threshold: Strike on CB
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    assert state.monsters[0].intent == "Stun"
    # Stun resolves (skip), next should be Beast Cry
    state = apply(state, EndTurnAction())
    assert state.monsters[0].intent == "Beast Cry", (
        f"after Stun, expected Beast Cry, got {state.monsters[0].intent}"
    )
    # Beast Cry resolves, next should be Stomp
    state = apply(state, EndTurnAction())
    assert state.monsters[0].intent == "Stomp"
    # Stomp resolves (15 damage), next should be Crush
    state = apply(state, EndTurnAction())
    assert state.monsters[0].intent == "Crush"
    # Crush resolves (17 dmg + 3 Str), next should be Beast Cry (loops back)
    state = apply(state, EndTurnAction())
    assert state.monsters[0].intent == "Beast Cry"


def test_phase2_cycle_repeats():
    """Phase 2 cycles correctly through multiple iterations."""
    from sts_sim import PlayCardAction, SelectTargetAction

    state = _cb_state(hp=151, seed=42)
    # Stamp → Plow → trigger threshold
    state = apply(state, EndTurnAction())
    state = apply(state, EndTurnAction())
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    # Skip Stun
    state = apply(state, EndTurnAction())
    # Run 3 full cycles (Beast Cry → Stomp → Crush × 3)
    for cycle in range(1, 4):
        assert state.monsters[0].intent == "Beast Cry", (
            f"cycle {cycle}: expected Beast Cry, got {state.monsters[0].intent}"
        )
        state = apply(state, EndTurnAction())
        assert state.monsters[0].intent == "Stomp"
        state = apply(state, EndTurnAction())
        assert state.monsters[0].intent == "Crush"
        state = apply(state, EndTurnAction())
    # Should loop back to Beast Cry
    assert state.monsters[0].intent == "Beast Cry"


def test_beast_cry_applies_ringing():
    """Beast Cry applies Ringing to the player."""
    from sts_sim import PlayCardAction, SelectTargetAction

    state = _cb_state(hp=151, seed=42)
    # Stamp → Plow → trigger threshold → Stun
    state = apply(state, EndTurnAction())
    state = apply(state, EndTurnAction())
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    # Stun (skip) → Beast Cry
    state = apply(state, EndTurnAction())
    state = apply(state, EndTurnAction())
    # Beast Cry should apply Ringing to the player
    ringing = [s for s in state.player_statuses if s == "Ringing"]
    assert len(ringing) >= 1, (
        f"expected Ringing on player, got: {state.player_statuses}"
    )


def test_ringing_limits_one_card_per_turn():
    """While Ringing is active, player can only play one card per turn."""
    from sts_sim import PlayCardAction, SelectTargetAction, legal_actions

    # Setup: get Ringing applied
    state = _cb_state(hp=151, seed=42)
    state = apply(state, EndTurnAction())  # Stamp
    state = apply(state, EndTurnAction())  # Plow
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))  # trigger → Stun
    state = apply(state, EndTurnAction())  # Stun skip
    state = apply(state, EndTurnAction())  # Beast Cry → Ringing applied
    # Player's turn: cards + Ringing active
    actions = legal_actions(state)
    card_actions = [a for a in actions if "PlayCard" in a]
    assert len(card_actions) > 0, "should have PlayCard actions before first card"
    # Play one card
    state = apply(state, PlayCardAction("Defend"))
    # After one card, PlayCard options should be removed
    actions = legal_actions(state)
    card_actions = [a for a in actions if "PlayCard" in a]
    assert len(card_actions) == 0, (
        f"Ringing should block further PlayCards, got: {card_actions}"
    )


def test_ringing_decays_at_end_of_player_turn():
    """Ringing should be gone by the start of the next player turn."""
    from sts_sim import PlayCardAction, SelectTargetAction

    state = _cb_state(hp=151, seed=42)
    state = apply(state, EndTurnAction())  # Stamp
    state = apply(state, EndTurnAction())  # Plow
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))  # trigger → Stun
    state = apply(state, EndTurnAction())  # Stun
    state = apply(state, EndTurnAction())  # Beast Cry → Ringing
    # Player plays one card and ends turn
    state = apply(state, PlayCardAction("Defend"))
    state = apply(state, EndTurnAction())
    # CB's turn resolves (Stomp), player's next turn starts
    # Ringing should be gone
    ringing = [s for s in state.player_statuses if s == "Ringing"]
    assert len(ringing) == 0, (
        f"Ringing should decay after player's turn, got: {state.player_statuses}"
    )


def test_phase2_strength_accumulates_from_crush():
    """In Phase 2, Crush's +3 Strength accumulates over cycles."""
    from sts_sim import PlayCardAction, SelectTargetAction

    state = _cb_state(hp=151, seed=42)
    # Stamp → Plow → trigger threshold → Stun → Beast Cry → Stomp
    state = apply(state, EndTurnAction())  # Stamp
    state = apply(state, EndTurnAction())  # Plow
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    state = apply(state, EndTurnAction())  # Stun
    state = apply(state, EndTurnAction())  # Beast Cry
    state = apply(state, EndTurnAction())  # Stomp
    # Now Crush resolves → +3 Strength
    state = apply(state, EndTurnAction())  # Crush
    cb = state.monsters[0]
    # Should have Strength from first Crush
    str_stacks = [s for s in cb.statuses if s == "Strength"]
    assert len(str_stacks) == 1, f"expected Strength, got: {cb.statuses}"
