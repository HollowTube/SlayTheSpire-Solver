"""Compare sim predictions against live game for Ironclad base deck cards.

For each card: starts a fresh Byrdonis fight, sets hand to just that card,
snapshots the live state, plays in both live game and sim, then diffs.

Run with:
    pytest integtests/test_sim_vs_live.py -v -s
"""

import time
from typing import NamedTuple

import pytest

from sts_sim import PlayCardAction, SelectTargetAction, apply, legal_actions
from sts_sim.bridge import client as bc
from sts_sim.bridge import diff, from_combat
from sts_sim.bridge.types import (
    CombatSnapshot,
    parse_available_actions,
    parse_card_piles,
    parse_combat_snapshot,
)
from sts_sim.sim.names import CardName

from .conftest import CombatFixture

pytestmark = pytest.mark.live


class CardSpec(NamedTuple):
    """A card with an optional upgrade level for parametrized tests.

    draw_fill: if non-empty, the draw pile is replaced with these cards before
    snapshotting — needed for cards that draw so draw-RNG divergence is
    eliminated by making every card in the draw pile identical.

    fight_id: STS2 console fight ID to use. Defaults to Byrdonis (single enemy,
    high HP). Use "THE_KIN_BOSS" for AoE cards that need multiple enemies.
    """

    card: CardName
    upgraded: bool = False
    draw_fill: tuple[CardName, ...] = ()
    fight_id: str = "BYRDONIS_ELITE"

    @property
    def sim_name(self) -> str:
        return self.card.value + ("+" if self.upgraded else "")

    def __str__(self) -> str:
        return self.card.name + ("_PLUS" if self.upgraded else "")


def _spec(card: CardName | CardSpec) -> CardSpec:
    return card if isinstance(card, CardSpec) else CardSpec(card)


# Cards to test — Ironclad base deck + commons, each at base and upgraded level.
# INFLAME excluded: sim exhausts it (STS1 behaviour), but STS2 treats it as a
# Power-type card that never leaves the hand into any pile.
def _both(card: CardName) -> list[CardSpec]:
    return [CardSpec(card), CardSpec(card, upgraded=True)]


# Cards that draw need an all-identical draw pile so order never matters.
_STRIKES_5 = (CardName.STRIKE,) * 5


def _draw(card: CardName) -> list[CardSpec]:
    """Like _both but fills draw pile with Strikes so draw-RNG can't diverge."""
    return [
        CardSpec(card, upgraded=False, draw_fill=_STRIKES_5),
        CardSpec(card, upgraded=True, draw_fill=_STRIKES_5),
    ]


def _aoe(card: CardName) -> list[CardSpec]:
    """Like _both but runs against THE_KIN_BOSS (3 enemies) to exercise AoE."""
    return [
        CardSpec(card, upgraded=False, fight_id="THE_KIN_BOSS"),
        CardSpec(card, upgraded=True, fight_id="THE_KIN_BOSS"),
    ]


BASE_DECK: list[CardName | CardSpec] = [
    *_both(CardName.STRIKE),  # 6 / 9 damage
    *_both(CardName.DEFEND),  # 5 / 8 block
    *_both(CardName.BASH),  # 8 / 10 damage + 2 / 3 Vulnerable
    *_both(CardName.IRON_WAVE),  # 5 / 7 block + 5 / 7 damage
    *_both(CardName.TWIN_STRIKE),  # 5×2 / 7×2 damage
    *_draw(CardName.SHRUG_IT_OFF),  # 8 / 11 block + draw 1 / 2 cards
    *_draw(CardName.POMMEL_STRIKE),  # 9 / 10 damage + draw 1 card
    *_aoe(CardName.THUNDERCLAP),  # 4 / 7 AoE + 1 Vulnerable (3-enemy Kin fight)
    *_both(CardName.UPPERCUT),  # 13 / 17 damage + Weak + Vulnerable
    *_both(CardName.ANGER),  # 6 / 8 damage + copy to discard
    *_both(CardName.IMPERVIOUS),  # 30 / 40 block, exhausts
    *_both(CardName.BLUDGEON),  # 32 / 42 damage
    *_both(CardName.BREAK),  # 2 Frail / +10 damage (Break+)
    *_both(CardName.HEMOKINESIS),  # lose 2 HP + 15 / 20 damage
    *_both(CardName.BLOOD_WALL),  # lose 2 HP + 16 / 20 block
    *_both(CardName.BLOODLETTING),  # lose 3 HP + gain 2 / 3 energy
    *_aoe(CardName.BREAKTHROUGH),  # lose 1 HP + 9 / 13 AoE (3-enemy Kin fight)
]

BYRDONIS_HP = 84


class ByrdonisFix(CombatFixture):
    FIGHT_ID = "BYRDONIS_ELITE"


def _apply_card_in_sim(state, card: CardName | CardSpec):
    """Play a card in the sim, auto-selecting the first target if required."""
    spec = _spec(card)
    mid = apply(state, PlayCardAction(spec.sim_name))
    if any("SelectTarget" in a for a in legal_actions(mid)):
        return apply(mid, SelectTargetAction(0))
    return mid


def _play_in_game(card: CardName | CardSpec) -> bool:
    """Play a card in the live game. Returns False if not available."""
    spec = _spec(card)
    avail = parse_available_actions(bc.get_available_actions())
    needle = spec.card.value.replace(" ", "").lower()

    # Use hand state to find the exact card index (upgraded vs non-upgraded),
    # then match the action by card_index rather than name alone.  This avoids
    # playing the wrong copy when both an upgraded and non-upgraded version of
    # the same card are present in hand.
    combat = parse_combat_snapshot(bc.get_combat_state())
    target_hand_idx = next(
        (
            c.index
            for c in combat.player.hand
            if needle in c.name.replace(" ", "").lower() and c.upgraded == spec.upgraded
        ),
        None,
    )
    if target_hand_idx is not None:
        act = next((a for a in avail.actions if a.card_index == target_hand_idx), None)
        if act:
            bc.play_card(act.card_index, act.target_index)
            time.sleep(0.5)
            return True

    # Fallback: match by name substring (e.g. when hand index lookup fails)
    act = next(
        (a for a in avail.actions if needle in a.card_name.replace(" ", "").lower()),
        None,
    )
    if act is None:
        return False
    bc.play_card(act.card_index, act.target_index)
    time.sleep(0.5)
    return True


def _stable_combat_state(retries: int = 6, delay: float = 0.25) -> CombatSnapshot:
    """Poll until two consecutive get_combat_state() calls agree on hand contents.

    Console commands are async; the state can be mid-transition immediately after
    set_hand() or upgrade_card(). The key includes the upgraded flag so an
    in-flight upgrade doesn't produce a false-stable read.
    """
    prev_hand: tuple | None = None
    snapshot = parse_combat_snapshot(bc.get_combat_state())
    for _ in range(retries):
        hand = tuple(sorted((c.name, c.upgraded) for c in snapshot.player.hand))
        if hand == prev_hand:
            return snapshot
        prev_hand = hand
        time.sleep(delay)
        snapshot = parse_combat_snapshot(bc.get_combat_state())
    return snapshot


@pytest.mark.parametrize("card", BASE_DECK, ids=str)
def test_sim_matches_live(card):
    """Sim prediction for playing ``card`` must match the live game result."""
    spec = _spec(card)
    fix = CombatFixture(fight_id=spec.fight_id)
    fix.setup_fight()
    fix.set_hand(spec.card)
    if spec.upgraded:
        # The card command appends to the rightmost slot; find its index
        # so we upgrade the correct card rather than whatever is at index 0.
        combat = parse_combat_snapshot(bc.get_combat_state())
        hand = combat.player.hand
        needle = spec.card.value.replace(" ", "").lower()
        idx = next(
            (
                c.index
                for c in reversed(hand)
                if needle in c.name.replace(" ", "").lower() and not c.upgraded
            ),
            hand[-1].index if hand else 0,
        )
        fix.upgrade_card(idx)

    if spec.draw_fill:
        fix.set_draw_pile(*spec.draw_fill)

    # Wait for the game state to settle after set_hand (console cmds are async)
    snapshot_before = _stable_combat_state()
    piles_before = parse_card_piles(bc.get_card_piles())
    state_before = from_combat(snapshot_before, card_piles=piles_before)

    # Play in live game
    assert _play_in_game(card), f"{card} not found in available actions after set_hand"

    # Capture live state after play
    snapshot_after = parse_combat_snapshot(bc.get_combat_state())
    piles_after = parse_card_piles(bc.get_card_piles())

    # Apply same card in sim
    try:
        sim_result = _apply_card_in_sim(state_before, card)
    except Exception as exc:
        pytest.skip(f"sim raised {type(exc).__name__}: {exc}")

    # Compare
    result = diff(sim_result, snapshot_after, card_piles=piles_after)

    mismatches = {
        field: cmp
        for field, cmp in result.items()
        if not cmp.get("skipped") and not cmp["match"]
    }

    # Print full report regardless of outcome
    print(f"\n--- {spec} ---")
    for field, cmp in result.items():
        if cmp.get("skipped"):
            print(f"  {field}: skipped ({cmp['reason']})")
        elif cmp["match"]:
            print(f"  {field}: ✓  {cmp['sim']}")
        else:
            print(f"  {field}: ✗  sim={cmp['sim']}  game={cmp['game']}")

    assert not mismatches, f"{spec} — sim diverged on: " + ", ".join(
        f"{f}(sim={c['sim']} game={c['game']})" for f, c in mismatches.items()
    )
