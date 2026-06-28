"""HOL-69: apply() accepts typed Action objects as well as strings.

Tests use sts_sim._sts_sim.apply (the raw Rust function) so the Rust-level
dispatch is exercised directly, not the Python wrapper's str(action) shim.
"""

from sts_sim import CombatState, EndTurnAction, Monster, PlayCardAction, SelectTargetAction, apply
from sts_sim._sts_sim import apply as _apply_rust


def _strike_in_hand():
    return CombatState(
        player_hp=70,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
    )


def test_apply_play_card_action_matches_string():
    state = _strike_in_hand()
    via_typed = _apply_rust(state, PlayCardAction("Strike"))
    via_string = _apply_rust(state, "PlayCard:Strike")
    assert via_typed.player_energy == via_string.player_energy
    assert [str(c) for c in via_typed.hand] == [str(c) for c in via_string.hand]


def test_apply_end_turn_action_matches_string():
    state = _strike_in_hand()
    via_typed = _apply_rust(state, EndTurnAction())
    via_string = _apply_rust(state, "EndTurn")
    assert via_typed.turn == via_string.turn
    assert via_typed.player_hp == via_string.player_hp


def test_apply_select_target_action_matches_string():
    # Play Strike to get into pending SelectTarget state, then resolve via typed action.
    state = _strike_in_hand()
    pending = apply(state, PlayCardAction("Strike"))
    via_typed = _apply_rust(pending, SelectTargetAction(0))
    via_string = _apply_rust(pending, "SelectTarget:Monster:0")
    assert via_typed.monsters[0].hp == via_string.monsters[0].hp


def test_apply_str_shim_still_works():
    state = _strike_in_hand()
    result = _apply_rust(state, "PlayCard:Strike")
    assert result.player_energy == 2  # Strike costs 1


def test_apply_invalid_type_raises_type_error():
    import pytest

    state = _strike_in_hand()
    with pytest.raises(TypeError):
        _apply_rust(state, 42)
