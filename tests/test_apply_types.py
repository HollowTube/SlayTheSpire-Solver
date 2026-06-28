"""HOL-69/70: apply() accepts typed Action objects; str is rejected after HOL-70.

All tests use sts_sim.apply directly — HOL-69 removed the Python wrapper so
this is the raw Rust function.
"""

import pytest

from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
)


def _strike_in_hand():
    return CombatState(
        player_hp=70,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
    )


def test_apply_play_card_action():
    state = _strike_in_hand()
    result = apply(state, PlayCardAction("Strike"))
    assert result.player_energy == 2  # Strike costs 1


def test_apply_end_turn_action():
    state = _strike_in_hand()
    result = apply(state, EndTurnAction())
    assert result.turn == 1


def test_apply_select_target_action():
    state = _strike_in_hand()
    pending = apply(state, PlayCardAction("Strike"))
    result = apply(pending, SelectTargetAction(0))
    assert result.monsters[0].hp < 44  # Strike dealt damage


def test_apply_str_raises_type_error():
    state = _strike_in_hand()
    with pytest.raises(TypeError):
        apply(state, "EndTurn")


def test_apply_invalid_type_raises_type_error():
    state = _strike_in_hand()
    with pytest.raises(TypeError):
        apply(state, 42)
