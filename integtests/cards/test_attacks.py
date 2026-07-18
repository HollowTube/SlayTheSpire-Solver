"""Live sim-vs-game parity tests for implemented Ironclad attack cards.

Run with:
    pytest integtests/cards/test_attacks.py -v -s -m live
"""

import pytest

from sts_sim.sim.names import CardName

from .conftest import CardSpec, _aoe, _both, _draw, _run_card_parity

pytestmark = pytest.mark.live

CARDS: list[CardName | CardSpec] = [
    *_both(CardName.STRIKE),  # 6 / 9 damage
    *_both(CardName.BASH),  # 8 / 10 damage + 2 / 3 Vulnerable
    *_both(CardName.IRON_WAVE),  # 5 / 7 block + 5 / 7 damage
    *_both(CardName.TWIN_STRIKE),  # 5×2 / 7×2 damage
    *_draw(CardName.POMMEL_STRIKE),  # 9 / 10 damage + draw 1 card
    *_aoe(CardName.THUNDERCLAP),  # 4 / 7 AoE + 1 Vulnerable
    *_both(CardName.UPPERCUT),  # 13 / 17 damage + Weak + Vulnerable
    *_both(CardName.ANGER),  # 6 / 8 damage + copy to discard
    *_both(CardName.BLUDGEON),  # 32 / 42 damage
    *_both(CardName.BREAK),  # 2 Frail / Break+: +10 damage
    *_both(CardName.HEMOKINESIS),  # lose 2 HP + 15 / 20 damage
    *_aoe(CardName.BREAKTHROUGH),  # lose 1 HP + 9 / 13 AoE
]


@pytest.mark.parametrize("card", CARDS, ids=str)
def test_sim_matches_live(card):
    _run_card_parity(card)
