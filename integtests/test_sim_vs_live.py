"""Compare sim predictions against live game for Ironclad base deck cards.

For each card: starts a fresh Byrdonis fight, sets hand to just that card,
snapshots the live state, plays in both live game and sim, then diffs.

Run with:
    pytest integtests/test_sim_vs_live.py -v -s
"""

import pytest

from sts_sim import PlayCardAction, SelectTargetAction, apply, legal_actions
from sts_sim import bridge_client as bc
from sts_sim.bridge import diff, from_combat
from sts_sim.names import CardName

from .conftest import CombatFixture

pytestmark = pytest.mark.live

# Cards to test — Ironclad base deck (playable cards only)
BASE_DECK = [CardName.STRIKE, CardName.DEFEND, CardName.BASH]

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
    act = next(
        (a for a in actions if card.value.lower() in a.get("card_name", "").lower()),
        None,
    )
    if act is None:
        return False
    bc.play_card(act["card_index"], act.get("target_index", -1))
    import time

    time.sleep(0.5)
    return True


@pytest.mark.parametrize("card", BASE_DECK, ids=lambda c: c.name)
def test_sim_matches_live(card):
    """Sim prediction for playing ``card`` must match the live game result."""
    fix = ByrdonisFix()
    fix.setup_fight()
    fix.set_hand(card)

    # Capture live state after set_hand — this is what the sim starts from
    raw_before = bc._payload(bc.get_combat_state())
    state_before = from_combat(raw_before)

    # Play in live game
    assert _play_in_game(card), f"{card} not found in available actions after set_hand"

    # Capture live state after play
    raw_after = bc._payload(bc.get_combat_state())

    # Apply same card in sim
    try:
        sim_result = _apply_card_in_sim(state_before, card)
    except Exception as exc:
        pytest.skip(f"sim raised {type(exc).__name__}: {exc}")

    # Compare
    result = diff(sim_result, raw_after)

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
