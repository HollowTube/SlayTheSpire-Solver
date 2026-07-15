"""MCTS-driven combat autoplay against a live STS2 game via the bridge."""

from __future__ import annotations

import time

from .. import legal_actions, mcts_search
from . import client as bc
from .translate import from_combat, sim_action_to_bridge
from .types import (
    parse_available_actions,
    parse_card_piles,
    parse_combat_snapshot,
)


def play_combat(iterations: int = 300, verbose: bool = True) -> str:
    """Play through the current combat using MCTS.

    Assumes the game is already at COMBAT_PLAYER_TURN. Each iteration:
    reads live state → converts to sim CombatState → runs MCTS → translates
    the best action to a bridge call → executes it. Loops until the screen
    leaves COMBAT.

    Args:
        iterations: MCTS rollout count per decision.
        verbose: Print round/action summaries to stdout.

    Returns:
        The screen name when combat ended (e.g. "REWARD", "GAME_OVER").
    """
    round_num = 0
    while True:
        round_num += 1
        if verbose:
            print(f"Round {round_num}")

        combat_ended = _play_turn(iterations, verbose)
        if combat_ended:
            break

        if verbose:
            print("  [enemy turn]")
        still_alive = _wait_for_player_turn()
        if not still_alive:
            break

    final = bc.get_screen().get("screen", "UNKNOWN")
    if verbose:
        print(f"Combat ended — screen: {final}")
    return final


def _play_turn(iterations: int, verbose: bool) -> bool:
    """Execute one full player turn. Returns True when combat ends."""
    while True:
        screen = bc.get_screen().get("screen", "UNKNOWN")

        if "LOADING" in screen or "ENEMY" in screen:
            time.sleep(0.3)
            continue

        if "COMBAT" not in screen:
            return True

        if "PLAYER_TURN" not in screen:
            time.sleep(0.3)
            continue

        snapshot = parse_combat_snapshot(bc.get_combat_state())
        piles = parse_card_piles(bc.get_card_piles())
        sim_state = from_combat(snapshot, card_piles=piles)
        avail = parse_available_actions(bc.get_available_actions())

        best_str = mcts_search(sim_state, iterations=iterations)
        best = next(a for a in legal_actions(sim_state) if str(a) == best_str)

        method, kwargs = sim_action_to_bridge(best, snapshot, avail)

        if verbose:
            p = snapshot.player
            enemies = [e for e in snapshot.enemies if e.is_alive]
            hand = [c.name + ("+" if c.upgraded else "") for c in p.hand]
            print(f"  HP:{p.hp}/{p.max_hp} E:{p.energy} hand:{hand}")
            for e in enemies:
                print(f"  enemy {e.name} HP:{e.hp}/{e.max_hp} block:{e.block}")
            print(f"  → {best_str}")

        if method == "end_turn":
            bc.end_turn()
            time.sleep(0.5)
            return False

        if method == "play_card":
            bc.play_card(kwargs["card_index"], kwargs["target_index"])
            time.sleep(0.5)
            screen = bc.get_screen().get("screen", "UNKNOWN")
            if "COMBAT" not in screen and "LOADING" not in screen:
                return True

        if method == "unknown":
            bc.end_turn()
            return False


def _wait_for_player_turn(timeout: float = 12.0) -> bool:
    """Wait for COMBAT_PLAYER_TURN after the enemy turn. Returns False if combat ended."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        screen = bc.get_screen().get("screen", "UNKNOWN")
        if "COMBAT_PLAYER_TURN" in screen:
            return True
        if "COMBAT" not in screen and "LOADING" not in screen:
            return False
        time.sleep(0.4)
    return False
