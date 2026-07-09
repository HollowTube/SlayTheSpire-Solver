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
    """Strike should be valued above Whirlwind when Shrink makes Whirlwind
    unable to kill the beetle but Strike+Strike+Stomp can."""
    vals = mcts.action_values(
        _state(attacks_played=0), iterations=2000, determinizations=8
    )
    # The key regression: Whirlwind (9 dmg, no kill) should not beat Strike
    # (part of a 3-card kill sequence). Defend may outscore Strike marginally
    # due to MCTS noise — that's fine as long as Whirlwind stays below Strike.
    assert vals["PlayCard:Strike"] > vals["PlayCard:Whirlwind"], (
        f"Strike should outscore Whirlwind; got Strike={vals['PlayCard:Strike']:.4f} "
        f"Whirlwind={vals['PlayCard:Whirlwind']:.4f}"
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


def test_strike_beats_whirlwind_raw_sts2_ids():
    """Regression: MCTS should pick Strike even when the mod sends raw STS2
    monster name ("SHRINKER_BEETLE") and intent ("CHOMP_MOVE") instead of the
    friendly forms. build_state's _translate_intent must normalise them before
    constructing the Monster, otherwise Chomp is unrecognised and the beetle
    appears harmless — making Whirlwind look deceptively attractive."""
    from sts_sim.server import build_state

    payload = {
        "cmd": "analyze",
        "iterations": 2000,
        "seed": 42,
        "state": {
            "player": {
                "hp": 80,
                "max_hp": 80,
                "energy": 3,
                "max_energy": 3,
                "block": 0,
                "statuses": [["SHRINK_POWER", -1]],
            },
            "hand": [
                "DEFEND_IRONCLAD",
                "WHIRLWIND",
                "STOMP",
                "STRIKE_IRONCLAD",
                "STRIKE_IRONCLAD",
            ],
            "draw_pile": [],
            "discard_pile": [],
            "exhaust_pile": [],
            "turn": 2,
            "attacks_played_this_turn": 0,
            "monsters": [
                {
                    "name": "SHRINKER_BEETLE",
                    "hp": 13,
                    "max_hp": 39,
                    "block": 0,
                    "intent": "CHOMP_MOVE",
                    "attack": 7,
                    "statuses": [],
                }
            ],
        },
    }

    state = build_state(payload)
    assert state.monsters[0].intent == "Chomp", (
        f"_translate_intent should have mapped CHOMP_MOVE → Chomp, got {state.monsters[0].intent!r}"
    )
    vals = mcts.action_values(state, iterations=2000, determinizations=8)
    # The regression: with Chomp correctly modelled (7 dmg), kill-focused plays
    # (Strike → kill sequence) should beat the non-killing Whirlwind (9 dmg, leaves
    # 4 HP). If Chomp were silently 0 dmg, Whirlwind would look free and win.
    assert vals["PlayCard:Strike"] > vals["PlayCard:Whirlwind"], (
        f"Strike should outscore Whirlwind (Chomp modelled correctly); "
        f"got Strike={vals['PlayCard:Strike']:.4f} Whirlwind={vals['PlayCard:Whirlwind']:.4f}"
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
