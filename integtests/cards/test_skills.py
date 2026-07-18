"""Live sim-vs-game parity tests for implemented Ironclad skill cards.

Run with:
    pytest integtests/cards/test_skills.py -v -s -m live
"""

import pytest

from sts_sim.sim.names import CardName

from .conftest import CardSpec, _both, _draw, _run_card_parity

pytestmark = pytest.mark.live

CARDS: list[CardName | CardSpec] = [
    *_both(CardName.DEFEND),  # 5 / 8 block
    *_draw(CardName.SHRUG_IT_OFF),  # 8 / 11 block + draw 1 / 2 cards
    *_both(CardName.IMPERVIOUS),  # 30 / 40 block, exhausts
    *_both(CardName.BLOOD_WALL),  # lose 2 HP + 16 / 20 block
    *_both(CardName.BLOODLETTING),  # lose 3 HP + gain 2 / 3 energy
]


@pytest.mark.parametrize("card", CARDS, ids=str)
def test_sim_matches_live(card):
    _run_card_parity(card)
