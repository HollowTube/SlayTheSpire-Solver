"""HOL-68: typed Action classes at the combat seam."""

from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
    legal_actions,
)


def test_end_turn_action_str():
    assert str(EndTurnAction()) == "EndTurn"


def test_play_card_action_str():
    assert str(PlayCardAction("Strike")) == "PlayCard:Strike"


def test_play_card_action_str_upgraded():
    assert str(PlayCardAction("Strike+")) == "PlayCard:Strike+"


def test_select_target_action_str():
    assert str(SelectTargetAction(0)) == "SelectTarget:Monster:0"


def test_select_target_action_str_nonzero_index():
    assert str(SelectTargetAction(2)) == "SelectTarget:Monster:2"


def test_play_card_action_card_property():
    assert PlayCardAction("Strike+").card == "Strike+"


def test_select_target_action_monster_index_property():
    assert SelectTargetAction(1).monster_index == 1


def test_action_eq_matching_string():
    assert EndTurnAction() == "EndTurn"
    assert PlayCardAction("Strike") == "PlayCard:Strike"
    assert SelectTargetAction(0) == "SelectTarget:Monster:0"


def test_action_eq_wrong_string_is_false():
    assert not (EndTurnAction() == "PlayCard:Strike")
    assert not (PlayCardAction("Strike") == "EndTurn")


def test_string_in_action_list():
    assert "EndTurn" in [EndTurnAction()]
    assert "PlayCard:Strike" in [PlayCardAction("Strike")]


def test_actions_are_hashable():
    d = {EndTurnAction(): 1, PlayCardAction("Strike"): 2, SelectTargetAction(0): 3}
    assert len(d) == 3


def test_actions_sortable():
    actions = [EndTurnAction(), PlayCardAction("Strike"), SelectTargetAction(0)]
    result = sorted(actions)
    assert [str(a) for a in result] == sorted(
        ["EndTurn", "PlayCard:Strike", SelectTargetAction(0)]
    )


# --- legal_actions integration ---


def _empty_hand_state():
    """State with no playable cards — only EndTurn is legal. No deck so no opening draw."""
    return CombatState(
        player_hp=70,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=42,
        hand=[],
    )


def _strike_in_hand_state():
    """State with Strike in hand, scripted directly (no deck)."""
    return CombatState(
        player_hp=70,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
    )


def test_legal_actions_returns_action_objects():
    state = _empty_hand_state()
    actions = legal_actions(state)
    assert all(
        isinstance(a, (EndTurnAction, PlayCardAction, SelectTargetAction))
        for a in actions
    )


def test_legal_actions_empty_hand_returns_end_turn_action():
    state = _empty_hand_state()
    actions = legal_actions(state)
    assert len(actions) == 1
    assert isinstance(actions[0], EndTurnAction)


def test_legal_actions_with_card_returns_play_card_action():
    state = _strike_in_hand_state()
    actions = legal_actions(state)
    play_actions = [a for a in actions if isinstance(a, PlayCardAction)]
    assert len(play_actions) == 1
    assert play_actions[0].card == "Strike"


def test_legal_actions_backward_compat_string_membership():
    """Existing code like '"EndTurn" in legal_actions(state)' must still work."""
    state = _empty_hand_state()
    assert "EndTurn" in legal_actions(state)


def test_legal_actions_backward_compat_list_equality():
    """Existing code like 'legal_actions(state) == ["EndTurn"]' must still work."""
    state = _empty_hand_state()
    assert legal_actions(state) == ["EndTurn"]


def test_legal_actions_backward_compat_play_card_string():
    state = _strike_in_hand_state()
    assert "PlayCard:Strike" in legal_actions(state)


def test_legal_actions_targeting_state_returns_select_target_actions():
    state = _strike_in_hand_state()
    # Strike is targeted — playing it produces a PendingDecision
    pending = apply(state, PlayCardAction("Strike"))
    actions = legal_actions(pending)
    assert len(actions) == 1
    assert isinstance(actions[0], SelectTargetAction)
    assert actions[0].monster_index == 0
    assert "SelectTarget:Monster:0" in actions


def test_legal_actions_actions_hashable_as_dict_keys():
    state = _strike_in_hand_state()
    values = {a: 0.5 for a in legal_actions(state)}
    assert len(values) == 2  # PlayCard:Strike + EndTurn


def test_legal_actions_sorted():
    state = _strike_in_hand_state()
    result = sorted(legal_actions(state))
    assert [str(a) for a in result] == sorted(["PlayCard:Strike", "EndTurn"])
