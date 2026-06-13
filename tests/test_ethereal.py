"""Behavioural tests for the Ethereal keyword and the Dazed status card:
Ethereal cards exhaust (rather than discard) if still in hand at end of
turn, and Dazed is additionally Unplayable."""

from sts_sim import CombatState, Monster, apply, legal_actions


def make_state(hand=("Strike",)):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=list(hand),
        draw_pile=["Defend"] * 5,
    )


def test_dazed_is_never_a_legal_action_even_with_full_energy():
    state = make_state(hand=["Dazed", "Strike"])

    actions = legal_actions(state)

    assert "PlayCard:Dazed" not in actions
    assert "PlayCard:Strike" in actions


def test_ethereal_card_exhausts_at_end_of_turn_if_still_in_hand():
    state = make_state(hand=["Dazed", "Strike"])

    after_turn = apply(state, "EndTurn")

    assert "Dazed" in after_turn.exhaust_pile
    assert "Dazed" not in after_turn.discard_pile


def test_non_ethereal_card_left_in_hand_is_discarded_not_exhausted():
    state = make_state(hand=["Dazed", "Strike"])

    after_turn = apply(state, "EndTurn")

    assert "Strike" in after_turn.discard_pile
    assert "Strike" not in after_turn.exhaust_pile
