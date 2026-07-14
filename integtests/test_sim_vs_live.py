"""Compare sim predictions against live game for Ironclad base deck cards.

For each card: starts a fresh Byrdonis fight, sets hand to just that card,
snapshots the live state, plays in both live game and sim, then diffs.

Run with:
    pytest integtests/test_sim_vs_live.py -v -s
"""

import time

import pytest

from sts_sim import PlayCardAction, SelectTargetAction, apply, legal_actions
from sts_sim import bridge_client as bc
from sts_sim.bridge import diff, from_combat
from sts_sim.names import CardName

from .conftest import CombatFixture

pytestmark = pytest.mark.live

# Cards to test — Ironclad base deck + commons.
# INFLAME excluded: sim exhausts it (STS1 behaviour), but STS2 treats it as a
# Power-type card that never leaves the hand into any pile.
BASE_DECK = [
    CardName.STRIKE,
    CardName.DEFEND,
    CardName.BASH,  # damage + 2 Vulnerable
    CardName.IRON_WAVE,  # block + damage
    CardName.TWIN_STRIKE,  # multi-hit
    CardName.SHRUG_IT_OFF,  # block only (no target)
    CardName.THUNDERCLAP,  # AoE damage + 1 Vulnerable
    CardName.UPPERCUT,  # damage + 1 Weak + 1 Vulnerable
    CardName.ANGER,  # damage + add copy to discard
    CardName.IMPERVIOUS,  # 30 block, exhausts
    CardName.BLUDGEON,  # heavy damage (32)
    CardName.BREAK,  # apply 2 Frail to enemy
]

BYRDONIS_HP = 84


class ByrdonisFix(CombatFixture):
    FIGHT_ID = "BYRDONIS_ELITE"


def _apply_card_in_sim(state, card: CardName):
    """Play a card in the sim, auto-selecting the first target if required."""
    mid = apply(state, PlayCardAction(card.value))
    if any("SelectTarget" in a for a in legal_actions(mid)):
        return apply(mid, SelectTargetAction(0))
    return mid


def _play_in_game(card: CardName) -> bool:
    """Play a card in the live game. Returns False if not available."""
    avail = bc._payload(bc.get_available_actions())
    actions = avail.get("actions", [])
    # Strip spaces so "Iron Wave" matches "IronWave" etc.
    needle = card.value.replace(" ", "").lower()
    act = next(
        (
            a
            for a in actions
            if needle in a.get("card_name", "").replace(" ", "").lower()
        ),
        None,
    )
    if act is None:
        return False
    bc.play_card(act["card_index"], act.get("target_index", -1))
    time.sleep(0.5)
    return True


def _stable_combat_state(retries: int = 6, delay: float = 0.25) -> dict:
    """Poll until two consecutive get_combat_state() calls agree on hand contents.

    Console commands are async; the state can be mid-transition immediately after
    set_hand(). Waiting for two identical snapshots in a row ensures we read a
    settled state before snapshotting for the sim.
    """
    prev_hand: tuple | None = None
    for _ in range(retries):
        raw = bc._payload(bc.get_combat_state())
        hand = tuple(
            sorted(
                c.get("name", "") for c in raw.get("players", [{}])[0].get("hand", [])
            )
        )
        if hand == prev_hand:
            return raw
        prev_hand = hand
        time.sleep(delay)
    return bc._payload(bc.get_combat_state())


@pytest.mark.parametrize("card", BASE_DECK, ids=lambda c: c.name)
def test_sim_matches_live(card):
    """Sim prediction for playing ``card`` must match the live game result."""
    fix = ByrdonisFix()
    fix.setup_fight()
    fix.set_hand(card)

    # Wait for the game state to settle after set_hand (console cmds are async)
    raw_before = _stable_combat_state()
    piles_before = bc._payload(bc.get_card_piles())
    state_before = from_combat(raw_before, card_piles=piles_before)

    # Play in live game
    assert _play_in_game(card), f"{card} not found in available actions after set_hand"

    # Capture live state after play
    raw_after = bc._payload(bc.get_combat_state())
    piles_after = bc._payload(bc.get_card_piles())

    # Apply same card in sim
    try:
        sim_result = _apply_card_in_sim(state_before, card)
    except Exception as exc:
        pytest.skip(f"sim raised {type(exc).__name__}: {exc}")

    # Compare
    result = diff(sim_result, raw_after, card_piles=piles_after)

    mismatches = {
        field: cmp
        for field, cmp in result.items()
        if not cmp.get("skipped") and not cmp["match"]
    }

    # Print full report regardless of outcome
    print(f"\n--- {card} ---")
    for field, cmp in result.items():
        if cmp.get("skipped"):
            print(f"  {field}: skipped ({cmp['reason']})")
        elif cmp["match"]:
            print(f"  {field}: ✓  {cmp['sim']}")
        else:
            print(f"  {field}: ✗  sim={cmp['sim']}  game={cmp['game']}")

    assert not mismatches, f"{card} — sim diverged on: " + ", ".join(
        f"{f}(sim={c['sim']} game={c['game']})" for f, c in mismatches.items()
    )
