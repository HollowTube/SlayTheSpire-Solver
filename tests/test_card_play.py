import pytest
import copy

from sts_sim import CombatState, apply, legal_actions


def make_state(hand=("Strike",)):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        hand=list(hand),
    )


def test_a_fresh_state_offers_to_play_each_card_in_hand():
    state = make_state(hand=["Strike"])

    assert "PlayCard:Strike" in legal_actions(state)


def test_playing_strike_asks_the_player_to_select_a_target():
    state = make_state(hand=["Strike"])

    next_state = apply(state, "PlayCard:Strike")

    assert next_state.pending == "SelectTarget"


def test_while_selecting_a_target_the_only_legal_actions_are_valid_targets():
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, "PlayCard:Strike")

    assert legal_actions(awaiting_target) == ["SelectTarget:Monster"]


def test_selecting_the_monster_resolves_strike_dealing_damage_and_clearing_the_pending_decision():
    state = make_state(hand=["Strike"])
    awaiting_target = apply(state, "PlayCard:Strike")

    resolved = apply(awaiting_target, "SelectTarget:Monster")

    assert resolved.monster_hp == state.monster_hp - 6
    assert resolved.pending is None


def test_playing_strike_spends_its_energy_cost_and_leaves_the_players_hand():
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, "PlayCard:Strike")

    assert awaiting_target.player_energy == state.player_energy - 1
    assert awaiting_target.hand == []


def test_playing_a_card_moves_it_from_hand_to_the_discard_pile_rather_than_vanishing():
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, "PlayCard:Strike")

    assert awaiting_target.discard_pile == ["Strike"]


# Per the Slay the Spire wiki, base (un-upgraded) Strike deals 6 damage.
STRIKE_DAMAGE = 6


def test_playing_strike_against_the_monster_deals_the_wiki_documented_damage():
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, "PlayCard:Strike")
    resolved = apply(awaiting_target, "SelectTarget:Monster")

    assert resolved.monster_hp == state.monster_hp - STRIKE_DAMAGE


def test_playing_a_card_does_not_mutate_its_input():
    state = make_state(hand=["Strike"])
    clone = copy.deepcopy(state)

    apply(state, "PlayCard:Strike")

    assert state == clone


def test_legal_actions_never_offers_play_card_or_end_turn_mid_target_selection():
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, "PlayCard:Strike")
    actions = legal_actions(awaiting_target)

    assert "PlayCard:Strike" not in actions
    assert "EndTurn" not in actions


def test_legal_actions_does_not_offer_cards_the_player_cannot_afford():
    # Bash costs 2 energy; with only 1 energy available it shouldn't be
    # offered as a play, mirroring how Slay the Spire greys out cards you
    # can't afford rather than letting you go into negative energy.
    state = CombatState(player_hp=80, player_energy=1, monster_hp=44, monster_attack=6,
                        seed=42, hand=["Bash"])

    assert "PlayCard:Bash" not in legal_actions(state)
    assert legal_actions(state) == ["EndTurn"]


def test_playing_a_card_the_player_cannot_afford_is_rejected():
    state = CombatState(player_hp=80, player_energy=1, monster_hp=44, monster_attack=6,
                        seed=42, hand=["Bash"])

    with pytest.raises(ValueError):
        apply(state, "PlayCard:Bash")
