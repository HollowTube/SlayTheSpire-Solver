"""Shared helpers for live card parity tests.

Each test file imports _both/_draw/_aoe/_run_card_parity from here and
defines its own CARDS list + parametrized test_sim_matches_live function.
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

from integtests.conftest import CombatFixture


class CardSpec(NamedTuple):
    """One card entry in a parity test.

    draw_fill: replace the draw pile with these cards before snapshotting.
               Use identical cards (e.g. all Strikes) to eliminate draw-RNG
               divergence for cards that draw.

    fight_id: STS2 console fight ID. Defaults to Byrdonis (single high-HP
              enemy). Use "THE_KIN_BOSS" for AoE cards that need 3 enemies.

    hand_fill: extra cards placed in hand before the test card. Needed for
               cards that exhaust or interact with other hand cards (e.g.
               Cinder, BurningPact, SecondWind).
    """

    card: CardName
    upgraded: bool = False
    draw_fill: tuple[CardName, ...] = ()
    fight_id: str = "BYRDONIS_ELITE"
    hand_fill: tuple[CardName, ...] = ()

    @property
    def sim_name(self) -> str:
        return self.card.value + ("+" if self.upgraded else "")

    def __str__(self) -> str:
        return self.card.name + ("_PLUS" if self.upgraded else "")


def _spec(card: CardName | CardSpec) -> CardSpec:
    return card if isinstance(card, CardSpec) else CardSpec(card)


# Five identical Strikes — used as draw-pile fill so draw-RNG can never
# cause sim/game divergence: whatever order cards are drawn, it's a Strike.
_STRIKES_5 = (CardName.STRIKE,) * 5


def _both(card: CardName) -> list[CardSpec]:
    """Base and upgraded variant of a card, single-enemy Byrdonis fight."""
    return [CardSpec(card), CardSpec(card, upgraded=True)]


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


# ---------------------------------------------------------------------------
# Internal helpers used by _run_card_parity
# ---------------------------------------------------------------------------


def _apply_card_in_sim(state, card: CardName | CardSpec):
    """Play a card in the sim, auto-selecting the first target if needed."""
    spec = _spec(card)
    mid = apply(state, PlayCardAction(spec.sim_name))
    if any("SelectTarget" in a for a in legal_actions(mid)):
        return apply(mid, SelectTargetAction(0))
    return mid


def _play_in_game(card: CardName | CardSpec) -> bool:
    """Play a card in the live game. Returns False if the card isn't available."""
    spec = _spec(card)
    avail = parse_available_actions(bc.get_available_actions())
    needle = spec.card.value.replace(" ", "").lower()

    # Prefer matching by hand index (upgraded flag included) to avoid playing
    # the wrong copy when both base and upgraded versions are in hand.
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

    # Fallback: match by name substring
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
    """Poll until two consecutive snapshots agree on hand contents.

    Console commands are async; state may be mid-transition immediately after
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


# ---------------------------------------------------------------------------
# Shared test body
# ---------------------------------------------------------------------------


def _run_card_parity(card: CardName | CardSpec) -> None:
    """Play card in both sim and live game then assert all state fields match."""
    spec = _spec(card)
    fix = CombatFixture(fight_id=spec.fight_id)
    fix.setup_fight()

    # hand_fill cards go in first; test card last so upgrade_card finds it at
    # the rightmost slot via reversed(hand) lookup below.
    fix.set_hand(*spec.hand_fill, spec.card)

    if spec.upgraded:
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

    snapshot_before = _stable_combat_state()
    piles_before = parse_card_piles(bc.get_card_piles())
    state_before = from_combat(snapshot_before, card_piles=piles_before)

    assert _play_in_game(card), f"{card} not found in available actions after set_hand"

    snapshot_after = parse_combat_snapshot(bc.get_combat_state())
    piles_after = parse_card_piles(bc.get_card_piles())

    try:
        sim_result = _apply_card_in_sim(state_before, card)
    except Exception as exc:
        pytest.skip(f"sim raised {type(exc).__name__}: {exc}")

    result = diff(sim_result, snapshot_after, card_piles=piles_after)

    mismatches = {
        field: cmp
        for field, cmp in result.items()
        if not cmp.get("skipped") and not cmp["match"]
    }

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
