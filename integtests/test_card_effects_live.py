"""Live-game integration tests for basic Ironclad card effects.

Verifies card behaviours against STS2 via the bridge mod dev console.
Enemy HP is pinned well above the damage dealt so the enemy never dies,
avoiding the REWARD screen between tests.

Run with:
    pytest integtests/test_card_effects_live.py -v
"""

import pytest

from integtests.conftest import _console


pytestmark = pytest.mark.live


def test_strike_deals_6_damage(fix):
    fix.pin_enemy_hp(40)
    fix.set_hand("STRIKE_IRONCLAD")
    hp_before = fix.enemy_hp()
    assert fix.play("strike"), "Strike not found in available actions"
    assert hp_before - fix.enemy_hp() == 6


def test_defend_gives_5_block(fix):
    fix._ensure_in_combat()
    fix.set_hand("DEFEND_IRONCLAD")
    block_before = fix.player_block()
    assert fix.play("defend"), "Defend not found in available actions"
    assert fix.player_block() - block_before == 5


def test_bash_deals_8_damage_and_applies_2_vulnerable(fix):
    fix.pin_enemy_hp(40)
    _console("energy 3")
    fix.set_hand("BASH")
    hp_before = fix.enemy_hp()
    assert fix.play("bash"), "Bash not found in available actions"
    assert hp_before - fix.enemy_hp() == 8
    assert fix.has_power("VulnerablePower", target="enemy") == 2
