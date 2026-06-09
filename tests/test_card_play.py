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
    state = CombatState(
        player_hp=80,
        player_energy=1,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        hand=["Bash"],
    )

    assert "PlayCard:Bash" not in legal_actions(state)
    assert legal_actions(state) == ["EndTurn"]


def test_playing_a_card_the_player_cannot_afford_is_rejected():
    state = CombatState(
        player_hp=80,
        player_energy=1,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        hand=["Bash"],
    )

    with pytest.raises(ValueError):
        apply(state, "PlayCard:Bash")


def test_playing_a_power_card_moves_it_to_the_exhaust_pile_not_discard():
    # Powers are played once per combat and never cycle back — they go to the
    # exhaust pile, not discard, so they can never be redrawn.
    state = make_state(hand=["Inflame"])

    resolved = apply(state, "PlayCard:Inflame")

    assert "Inflame" not in resolved.discard_pile
    assert "Inflame" in resolved.exhaust_pile


def test_exhausted_power_is_not_available_in_subsequent_turns():
    # Even after the discard reshuffles into the draw pile, exhausted cards
    # must not reappear — the exhaust pile is a permanent one-way sink.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        deck=["Inflame"],
    )
    # Play Inflame from the opening hand.
    after_inflame = apply(state, "PlayCard:Inflame")
    # End the turn — discard reshuffles, new hand drawn.
    after_turn = apply(after_inflame, "EndTurn")

    assert "Inflame" not in after_turn.hand
    assert "Inflame" not in after_turn.draw_pile
    assert "Inflame" not in after_turn.discard_pile
    assert "Inflame" in after_turn.exhaust_pile


# ── New cards ─────────────────────────────────────────────────────────────────


def test_ascenders_bane_cannot_be_played():
    # Ascender's Bane is a Curse — unplayable filler that clogs the hand.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        hand=["Ascender's Bane"],
    )
    actions = legal_actions(state)
    assert "PlayCard:Ascender's Bane" not in actions


def test_sword_boomerang_deals_three_hits_of_three_damage_each():
    # Sword Boomerang hits a random enemy 3 times for 3 each = 9 total.
    # Against a single monster the hits always land on the same target.
    state = make_state(hand=["Sword Boomerang"])
    resolved = apply(apply(state, "PlayCard:Sword Boomerang"), "SelectTarget:Monster")
    assert resolved.monster_hp == state.monster_hp - 9


def test_thunderclap_deals_damage_and_applies_vulnerable_without_selecting_a_target():
    # Thunderclap hits all enemies for 4 and applies 1 Vulnerable to each —
    # it resolves immediately (no SelectTarget step).
    state = make_state(hand=["Thunderclap"])
    resolved = apply(state, "PlayCard:Thunderclap")
    assert resolved.pending is None
    assert resolved.monster_hp == state.monster_hp - 4
    assert "Vulnerable" in resolved.monster_statuses


def test_rage_grants_block_when_an_attack_is_played_afterward():
    # Playing Rage (a Skill) installs the Rage status. Each subsequent
    # Attack played should grant 2 Block to the player.
    state = make_state(hand=["Rage", "Strike"])
    after_rage = apply(state, "PlayCard:Rage")
    assert "Rage" in after_rage.player_statuses

    after_strike = apply(apply(after_rage, "PlayCard:Strike"), "SelectTarget:Monster")
    assert after_strike.player_block == 2


def test_rage_does_not_grant_block_for_skills():
    # Only Attacks trigger Rage — playing another Skill must not give block.
    state = make_state(hand=["Rage", "Defend"])
    after_rage = apply(state, "PlayCard:Rage")
    after_defend = apply(after_rage, "PlayCard:Defend")
    # Block comes from Defend (5), not Rage — Rage adds nothing for Skills.
    assert after_defend.player_block == 5


def test_pommel_strike_deals_9_damage_and_draws_1_card():
    # Pommel Strike: 1 energy Attack — deal 9 damage, draw 1 card.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=0,
        seed=42,
        hand=["Pommel Strike", "Defend", "Defend", "Defend", "Defend"],
    )
    hand_size_before = len(state.hand)
    awaiting = apply(state, "PlayCard:Pommel Strike")
    resolved = apply(awaiting, "SelectTarget:Monster")

    assert resolved.monster_hp == state.monster_hp - 9
    assert len(resolved.hand) == hand_size_before - 1 + 1  # spent Pommel Strike, drew 1
