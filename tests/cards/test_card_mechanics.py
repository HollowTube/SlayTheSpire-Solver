"""Core card mechanics: legal actions, energy costs, discard vs exhaust,
card instances (upgraded cards), and the Ethereal keyword."""

import copy

import pytest
from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
    legal_actions,
)


# ── Card play mechanics ───────────────────────────────────────────────────────


def test_a_fresh_state_offers_to_play_each_card_in_hand(make_state):
    state = make_state(hand=["Strike"])

    assert "PlayCard:Strike" in legal_actions(state)


def test_playing_strike_asks_the_player_to_select_a_target(make_state):
    state = make_state(hand=["Strike"])

    next_state = apply(state, PlayCardAction("Strike"))

    assert next_state.pending == "SelectTarget"


def test_while_selecting_a_target_the_only_legal_actions_are_valid_targets(make_state):
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, PlayCardAction("Strike"))

    assert legal_actions(awaiting_target) == ["SelectTarget:Monster:0"]


def test_selecting_the_monster_resolves_strike_dealing_damage_and_clearing_the_pending_decision(
    make_state,
):
    state = make_state(hand=["Strike"])
    awaiting_target = apply(state, PlayCardAction("Strike"))

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert resolved.monsters[0].hp == state.monsters[0].hp - 6
    assert resolved.pending is None


def test_playing_strike_spends_its_energy_cost_and_leaves_the_players_hand(make_state):
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, PlayCardAction("Strike"))

    assert awaiting_target.player_energy == state.player_energy - 1
    assert awaiting_target.hand == []


def test_playing_a_card_moves_it_from_hand_to_the_discard_pile_rather_than_vanishing(
    make_state,
):
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, PlayCardAction("Strike"))

    assert awaiting_target.discard_pile == ["Strike"]


# Per the Slay the Spire wiki, base (un-upgraded) Strike deals 6 damage.
STRIKE_DAMAGE = 6


def test_playing_strike_against_the_monster_deals_the_wiki_documented_damage(
    make_state,
):
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, PlayCardAction("Strike"))
    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert resolved.monsters[0].hp == state.monsters[0].hp - STRIKE_DAMAGE


def test_playing_a_card_does_not_mutate_its_input(make_state):
    state = make_state(hand=["Strike"])
    clone = copy.deepcopy(state)

    apply(state, PlayCardAction("Strike"))

    assert state == clone


def test_legal_actions_never_offers_play_card_or_end_turn_mid_target_selection(
    make_state,
):
    state = make_state(hand=["Strike"])

    awaiting_target = apply(state, PlayCardAction("Strike"))
    actions = legal_actions(awaiting_target)

    assert "PlayCard:Strike" not in actions
    assert "EndTurn" not in actions


def test_legal_actions_does_not_offer_cards_the_player_cannot_afford():
    # Bash costs 2 energy; with only 1 energy available it shouldn't be
    # offered as a play, mirroring how Slay the Spire greys out cards you
    # can't afford rather than letting you go into negative energy.
    state = CombatState(
        player_hp=80,
        player_energy=1,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["Bash"],
    )

    assert "PlayCard:Bash" not in legal_actions(state)
    assert legal_actions(state) == ["EndTurn"]


def test_playing_a_card_the_player_cannot_afford_is_rejected():
    state = CombatState(
        player_hp=80,
        player_energy=1,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["Bash"],
    )

    with pytest.raises(ValueError):
        apply(state, PlayCardAction("Bash"))


def test_playing_a_power_card_moves_it_to_the_exhaust_pile_not_discard(make_state):
    # Powers are played once per combat and never cycle back — they go to the
    # exhaust pile, not discard, so they can never be redrawn.
    state = make_state(hand=["Inflame"])

    resolved = apply(state, PlayCardAction("Inflame"))

    assert "Inflame" not in resolved.discard_pile
    assert "Inflame" in resolved.exhaust_pile


def test_exhausted_power_is_not_available_in_subsequent_turns():
    # Even after the discard reshuffles into the draw pile, exhausted cards
    # must not reappear — the exhaust pile is a permanent one-way sink.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        deck=["Inflame"],
    )
    after_inflame = apply(state, PlayCardAction("Inflame"))
    after_turn = apply(after_inflame, EndTurnAction())

    assert "Inflame" not in after_turn.hand
    assert "Inflame" not in after_turn.draw_pile
    assert "Inflame" not in after_turn.discard_pile
    assert "Inflame" in after_turn.exhaust_pile


def test_ascenders_bane_cannot_be_played():
    # Ascender's Bane is a Curse — unplayable filler that clogs the hand.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["Ascender's Bane"],
    )
    actions = legal_actions(state)
    assert "PlayCard:Ascender's Bane" not in actions


def test_sword_boomerang_deals_three_hits_of_three_damage_each(make_state):
    # Sword Boomerang hits a random enemy 3 times for 3 each = 9 total.
    # Against a single monster the hits always land on the same target.
    state = make_state(hand=["Sword Boomerang"])
    resolved = apply(
        apply(state, PlayCardAction("Sword Boomerang")), SelectTargetAction(0)
    )
    assert resolved.monsters[0].hp == state.monsters[0].hp - 9


def test_thunderclap_deals_damage_and_applies_vulnerable_without_selecting_a_target(
    make_state,
):
    # Thunderclap hits all enemies for 4 and applies 1 Vulnerable to each —
    # it resolves immediately (no SelectTarget step).
    state = make_state(hand=["Thunderclap"])
    resolved = apply(state, PlayCardAction("Thunderclap"))
    assert resolved.pending is None
    assert resolved.monsters[0].hp == state.monsters[0].hp - 4
    assert "Vulnerable" in resolved.monsters[0].statuses


def test_rage_grants_block_when_an_attack_is_played_afterward(make_state):
    # Playing Rage (a Skill) installs the Rage status. Each subsequent
    # Attack played should grant 3 Block to the player.
    state = make_state(hand=["Rage", "Strike"])
    after_rage = apply(state, PlayCardAction("Rage"))
    assert "Rage" in after_rage.player_statuses

    after_strike = apply(
        apply(after_rage, PlayCardAction("Strike")), SelectTargetAction(0)
    )
    assert after_strike.player_block == 3


def test_rage_does_not_grant_block_for_skills(make_state):
    # Only Attacks trigger Rage — playing another Skill must not give block.
    state = make_state(hand=["Rage", "Defend"])
    after_rage = apply(state, PlayCardAction("Rage"))
    after_defend = apply(after_rage, PlayCardAction("Defend"))
    # Block comes from Defend (5), not Rage — Rage adds nothing for Skills.
    assert after_defend.player_block == 5


def test_pommel_strike_deals_9_damage_and_draws_1_card():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["Pommel Strike", "Defend", "Defend", "Defend", "Defend"],
    )
    hand_size_before = len(state.hand)
    awaiting = apply(state, PlayCardAction("Pommel Strike"))
    resolved = apply(awaiting, SelectTargetAction(0))

    assert resolved.monsters[0].hp == state.monsters[0].hp - 9
    assert len(resolved.hand) == hand_size_before - 1 + 1  # spent Pommel Strike, drew 1


def test_slimed_costs_1_draws_a_card_and_exhausts():
    # Slimed is the Status card slime monsters stick the player with: 1
    # energy, draws 1 card on play, and exhausts rather than going to discard.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["Slimed", "Defend"],
    )

    resolved = apply(state, PlayCardAction("Slimed"))

    assert resolved.player_energy == 2
    assert "Defend" in resolved.hand
    assert "Slimed" not in resolved.hand
    assert "Slimed" in resolved.exhaust_pile
    assert "Slimed" not in resolved.discard_pile


# ── Card instances (upgraded cards) ──────────────────────────────────────────


def _card_instance_state(hand, seed=42, player_energy=3, **kwargs):
    return CombatState(
        player_hp=80,
        player_energy=player_energy,
        monsters=[Monster(hp=44, attack=0)],
        seed=seed,
        hand=list(hand),
        **kwargs,
    )


def test_upgraded_card_round_trips_through_hand_and_legal_actions():
    state = _card_instance_state(hand=["Strike+", "Defend"])

    assert state.hand == ["Strike+", "Defend"]
    assert "PlayCard:Strike+" in legal_actions(state)


def test_playing_an_upgraded_card_uses_base_card_data_and_lands_in_discard():
    state = _card_instance_state(hand=["Strike+"])

    after_play = apply(state, PlayCardAction("Strike+"))
    after = apply(after_play, SelectTargetAction(0))

    # Strike+ deals 9 damage (UpgradeDelta mechanism applies the upgrade).
    assert after.monsters[0].hp == 44 - 9
    assert after.discard_pile == ["Strike+"]
    assert after.hand == []


def test_playing_one_copy_leaves_the_other_copys_upgrade_level_intact():
    state = _card_instance_state(hand=["Strike", "Strike+"], player_energy=4)

    after = apply(state, PlayCardAction("Strike"))

    assert after.hand == ["Strike+"]
    assert after.discard_pile == ["Strike"]


def test_upgraded_targeted_card_goes_through_select_target():
    state = _card_instance_state(hand=["Bash+"])

    after_play = apply(state, PlayCardAction("Bash+"))
    assert after_play.pending == "SelectTarget"
    assert legal_actions(after_play) == ["SelectTarget:Monster:0"]

    after_target = apply(after_play, SelectTargetAction(0))
    assert after_target.discard_pile == ["Bash+"]
    assert after_target.monsters[0].hp < 44


# ── Ethereal keyword ──────────────────────────────────────────────────────────


def _ethereal_state(hand=("Strike",)):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=list(hand),
        draw_pile=["Defend"] * 5,
    )


def test_dazed_is_never_a_legal_action_even_with_full_energy():
    state = _ethereal_state(hand=["Dazed", "Strike"])

    actions = legal_actions(state)

    assert "PlayCard:Dazed" not in actions
    assert "PlayCard:Strike" in actions


def test_ethereal_card_exhausts_at_end_of_turn_if_still_in_hand():
    state = _ethereal_state(hand=["Dazed", "Strike"])

    after_turn = apply(state, EndTurnAction())

    assert "Dazed" in after_turn.exhaust_pile
    assert "Dazed" not in after_turn.discard_pile


def test_non_ethereal_card_left_in_hand_is_discarded_not_exhausted():
    state = _ethereal_state(hand=["Dazed", "Strike"])

    after_turn = apply(state, EndTurnAction())

    assert "Strike" in after_turn.discard_pile
    assert "Strike" not in after_turn.exhaust_pile
