"""Live-game integration tests for basic Ironclad card effects.

Each test calls setup_fight() which uses `fight NIBBITS_WEAK` to jump directly
into a fresh combat — no Neow, no map navigation, no REWARD screen to handle.

Run with:
    pytest integtests/test_card_effects_live.py -v
"""

import pytest

from sts_sim.bridge import StatusName
from sts_sim.names import CardName


pytestmark = pytest.mark.live


def test_strike_deals_6_damage(fix):
    fix.setup_fight()
    fix.set_hand(CardName.STRIKE)
    hp_before = fix.enemy_hp()
    assert fix.play(CardName.STRIKE), "Strike not found in available actions"
    assert hp_before - fix.enemy_hp() == 6


def test_defend_gives_5_block(fix):
    fix.setup_fight()
    fix.set_hand(CardName.DEFEND)
    block_before = fix.player_block()
    assert fix.play(CardName.DEFEND), "Defend not found in available actions"
    assert fix.player_block() - block_before == 5


def test_bash_deals_8_damage_and_applies_2_vulnerable(fix):
    fix.setup_fight()
    fix.set_hand(CardName.BASH)
    hp_before = fix.enemy_hp()
    assert fix.play(CardName.BASH), "Bash not found in available actions"
    assert hp_before - fix.enemy_hp() == 8
    assert fix.has_power(StatusName.VULNERABLE, target="enemy") == 2
