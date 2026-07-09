"""Regression test: MCTS should prefer Strike+Strike+Stomp over Whirlwind
when the beetle is at 13 HP and the player has Shrink.

Exact state from a live game session (2026-07-08):
  hand: Defend, Whirlwind, Stomp, Strike, Strike
  player: 3 energy, Shrink debuff (-30% damage)
  monster: Shrinker Beetle 13/39 HP, Chomp intent
  attacks_played_this_turn: 0 (start of turn)

With Shrink:
  Whirlwind (3 energy): 3 hits × floor(5×0.7)=3 = 9 dmg → beetle 4 HP (no kill)
  Strike+Strike+Stomp: floor(6×0.7)=4 + 4 + floor(12×0.7)=8 = 16 dmg → kill
  Stomp costs max(3 - attacks_played, 0); after 2 Strikes → costs 1.
"""

import pytest
from sts_sim import CombatState, Monster
from sts_sim import mcts


def _state(attacks_played=0):
    return CombatState(
        player_hp=80,
        player_max_hp=80,
        player_energy=3,
        player_block=0,
        player_statuses=[("SHRINK_POWER", -1)],
        hand=[
            "DEFEND_IRONCLAD",
            "WHIRLWIND",
            "STOMP",
            "STRIKE_IRONCLAD",
            "STRIKE_IRONCLAD",
        ],
        monsters=[
            Monster(
                name="Shrinker Beetle",
                hp=13,
                max_hp=39,
                block=0,
                intent="Chomp",
                attack=0,
                statuses=[],
                last_move=None,
                move_streak=0,
            )
        ],
        seed=42,
        turn=2,
        draw_pile=[],
        discard_pile=[],
        exhaust_pile=[],
        attacks_played_this_turn=attacks_played,
    )


def test_strike_beats_whirlwind_with_shrink():
    """Strike should be recommended over Whirlwind when Shrink makes Whirlwind
    unable to kill the beetle but Strike+Strike+Stomp can."""
    vals = mcts.action_values(_state(attacks_played=0), iterations=2000, determinizations=8)
    best = max(vals, key=vals.get)
    # Strike is the correct first move; Whirlwind only does 9 dmg (no kill)
    assert best == "PlayCard:Strike", (
        f"Expected Strike as best first move, got {best}. Values: {vals}"
    )


def test_stomp_cost_after_two_attacks():
    """After 2 attacks played, Stomp should cost 1 (base 3 - 2 = 1)."""
    from sts_sim import PlayCardAction, SelectTargetAction, legal_actions, apply

    state = _state(attacks_played=0)
    # Play Strike (1st attack)
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    # Play Strike (2nd attack)
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    # Now Stomp should cost 1 (3 - 2 = 1) and be legal with 1 energy left
    actions = [str(a) for a in legal_actions(state)]
    assert any("Stomp" in a for a in actions), (
        f"Stomp should be legal after 2 Strikes (costs 1), got: {actions}"
    )


def test_stomp_kills_beetle():
    """Strike+Strike+Stomp should kill the 13 HP beetle under Shrink."""
    from sts_sim import PlayCardAction, SelectTargetAction, apply

    state = _state(attacks_played=0)
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    state = apply(state, PlayCardAction("Strike"))
    state = apply(state, SelectTargetAction(0))
    state = apply(state, PlayCardAction("Stomp"))
    # Beetle should be dead
    assert state.monsters[0].hp <= 0, (
        f"Beetle should be dead after Strike+Strike+Stomp, hp={state.monsters[0].hp}"
    )
