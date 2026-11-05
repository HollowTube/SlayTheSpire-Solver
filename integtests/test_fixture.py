"""Live checks for CombatFixture's pile helpers.

Run with:
    pytest integtests/test_fixture.py -v -s -m live
"""

import pytest

from sts_sim.sim.names import CardName

from integtests.conftest import CombatFixture, _stable_piles

pytestmark = pytest.mark.live


def _names(pile) -> list[str]:
    return sorted(c.name.lower() for c in pile.cards)


def test_set_discard_pile_replaces_existing_cards():
    fix = CombatFixture()
    fix.setup_fight()
    fix.set_hand()
    fix.set_draw_pile(CardName.STRIKE)
    fix.set_discard_pile(CardName.STRIKE, CardName.STRIKE)
    before = _stable_piles()
    assert len(before.discard_pile.cards) == 2
    draw_before = _names(before.draw_pile)

    fix.set_discard_pile(CardName.DEFEND)

    after = _stable_piles()
    discard = _names(after.discard_pile)
    assert len(discard) == 1 and "defend" in discard[0]
    assert _names(after.draw_pile) == draw_before
    assert after.hand.cards == []
