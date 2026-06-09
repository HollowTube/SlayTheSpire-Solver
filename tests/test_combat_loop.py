import copy

from sts_sim import CombatState, apply, evaluate, is_terminal, legal_actions, reward


# The toy monster's HP, as configured by `make_state` and reused directly by
# fixtures below — not a wiki value, just this scenario's starting point.
MONSTER_STARTING_HP = 44


def make_state():
    return CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=MONSTER_STARTING_HP,
        monster_attack=6,
        seed=42,
    )


# Per the Slay the Spire wiki, the Ironclad opens combat with a 5-card hand.
HAND_SIZE = 5


def test_constructing_with_a_deck_shuffles_it_into_the_draw_pile_and_deals_an_opening_hand():
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]

    state = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=MONSTER_STARTING_HP,
        monster_attack=6,
        seed=42,
        deck=list(deck),
    )

    assert len(state.hand) == HAND_SIZE
    assert len(state.draw_pile) == len(deck) - HAND_SIZE
    assert sorted(state.hand + state.draw_pile) == sorted(deck)
    assert state.discard_pile == []


def test_fresh_state_exposes_empty_draw_and_discard_piles_by_default():
    state = make_state()

    assert state.draw_pile == []
    assert state.discard_pile == []


def test_fresh_state_only_legal_action_is_end_turn():
    state = make_state()

    assert legal_actions(state) == ["EndTurn"]


def test_end_turn_discards_the_remaining_hand_then_draws_a_fresh_one_from_the_draw_pile():
    # A full 10-card deck leaves enough in the draw pile that the post-end-turn
    # redraw doesn't need to reshuffle the just-discarded hand back in — so the
    # discarded cards stay observable in discard_pile, isolating "discard the
    # old hand" from "redraw a fresh one" (the latter checked by hand size).
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        deck=list(deck),
    )
    discarded_hand = list(state.hand)

    next_state = apply(state, "EndTurn")

    assert sorted(next_state.discard_pile) == sorted(discarded_hand)
    assert len(next_state.hand) == HAND_SIZE
    assert sorted(
        next_state.hand + next_state.draw_pile + next_state.discard_pile
    ) == sorted(deck)


def test_drawing_reshuffles_the_discard_pile_into_the_draw_pile_once_it_empties():
    # A 10-card deck deals a 5-card opening hand, leaving exactly 5 in the
    # draw pile — just enough for one more full draw, and no more: ending the
    # first turn drains the draw pile to empty, so ending the second turn must
    # reshuffle the (by-then 10-card) discard pile back in mid-draw to deal a
    # fresh hand. This must happen without error, and every card must remain
    # present across the piles (nothing vanishes or duplicates).
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        deck=list(deck),
    )

    state = apply(state, "EndTurn")
    assert state.draw_pile == []

    state = apply(state, "EndTurn")

    assert len(state.hand) == HAND_SIZE
    assert sorted(state.hand + state.draw_pile + state.discard_pile) == sorted(deck)


def test_end_turn_advances_the_turn_counter():
    state = make_state()

    next_state = apply(state, "EndTurn")

    assert next_state.turn == state.turn + 1


def test_end_turn_applies_the_monsters_fixed_attack_to_the_player():
    state = make_state()

    next_state = apply(state, "EndTurn")

    assert next_state.player_hp == state.player_hp - state.monster_attack


# Per HOL-10: a real fight spans multiple turns, and the original
# CombatState (HOL-6) only ever set energy once at construction — nothing
# replenished it between turns. That cap (3 energy/fight = 18 max damage)
# made the sim unwinnable against anything but the most trivial monster, so
# `EndTurn` must refresh energy back to its starting amount each turn, the
# same way Slay the Spire does.
def test_end_turn_refreshes_player_energy_to_its_starting_amount():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        hand=["Strike"],
    )
    spent = apply(apply(state, "PlayCard:Strike"), "SelectTarget:Monster")
    assert spent.player_energy < state.player_energy

    next_state = apply(spent, "EndTurn")

    assert next_state.player_energy == state.player_energy


def test_fresh_state_is_not_terminal():
    state = make_state()

    assert is_terminal(state) is False


def test_state_is_terminal_once_player_hp_reaches_zero():
    state = CombatState(
        player_hp=5, player_energy=3, monster_hp=44, monster_attack=6, seed=42
    )

    next_state = apply(state, "EndTurn")

    assert next_state.player_hp <= 0
    assert is_terminal(next_state) is True


def test_state_is_terminal_once_monster_hp_reaches_zero():
    state = CombatState(
        player_hp=80, player_energy=3, monster_hp=0, monster_attack=6, seed=42
    )

    assert is_terminal(state) is True


def test_reward_is_positive_when_the_player_wins():
    won = CombatState(
        player_hp=80,
        player_max_hp=80,
        monster_hp=0,
        monster_max_hp=44,
        player_energy=3,
        monster_attack=6,
        seed=42,
    )

    assert reward(won) > 0


def test_reward_is_negative_when_the_player_loses():
    lost = CombatState(
        player_hp=0,
        player_max_hp=80,
        monster_hp=44,
        monster_max_hp=44,
        player_energy=3,
        monster_attack=6,
        seed=42,
    )

    assert reward(lost) < 0


def test_reward_rewards_decisive_wins_more_than_narrow_wins():
    decisive_win = CombatState(
        player_hp=80,
        player_max_hp=80,
        monster_hp=0,
        monster_max_hp=44,
        player_energy=3,
        monster_attack=6,
        seed=42,
    )
    narrow_win = CombatState(
        player_hp=1,
        player_max_hp=80,
        monster_hp=0,
        monster_max_hp=44,
        player_energy=3,
        monster_attack=6,
        seed=42,
    )

    assert reward(decisive_win) > reward(narrow_win)


def test_reward_is_zero_for_a_non_terminal_state():
    state = make_state()

    assert reward(state) == 0


# Per HOL-9, the shaped reward is `(+1 if win else -1) * hp_fraction`, where
# hp_fraction is the *winning* side's remaining HP as a fraction of its max —
# this is what an actual STS run optimizes for (player HP carries between
# fights) and what an eventual RL value head learns to predict.
def test_reward_matches_the_shaped_formula_for_a_win_at_partial_hp():
    won_at_half_hp = CombatState(
        player_hp=40,
        player_max_hp=80,
        monster_hp=0,
        monster_max_hp=44,
        player_energy=3,
        monster_attack=6,
        seed=42,
    )

    assert reward(won_at_half_hp) == 0.5


def test_reward_matches_the_shaped_formula_for_a_loss_with_the_monster_at_partial_hp():
    # Chip the monster down before the player dies, so the monster is at a
    # known, *reachable* fraction of its starting HP at the moment of loss —
    # then check the shaped reward against that fraction directly, rather than
    # constructing an artificial "pre-damaged" monster (which can't occur in
    # real play, since monsters always start an encounter at full HP).
    weak_player = CombatState(
        player_hp=5,
        player_energy=3,
        monster_hp=MONSTER_STARTING_HP,
        monster_attack=6,
        seed=42,
        hand=["Strike"],
    )
    chipped = apply(apply(weak_player, "PlayCard:Strike"), "SelectTarget:Monster")
    lost = apply(chipped, "EndTurn")

    assert is_terminal(lost) and lost.player_hp <= 0 and lost.monster_hp > 0
    assert reward(lost) == -1 * (lost.monster_hp / MONSTER_STARTING_HP)


def test_evaluate_favours_the_side_with_relatively_more_remaining_hp():
    armed = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=MONSTER_STARTING_HP,
        monster_attack=6,
        seed=42,
        hand=["Strike"],
    )

    # Deal Strike damage to the monster within a single turn, taking no return
    # damage — the monster ends up relatively worse off than the player.
    monster_hurt = apply(apply(armed, "PlayCard:Strike"), "SelectTarget:Monster")

    # Take repeated attacks without retaliating — the player ends up relatively
    # worse off than the monster.
    player_hurt = apply(apply(armed, "EndTurn"), "EndTurn")

    assert evaluate(monster_hurt) > evaluate(player_hurt)


def play_out(state, num_turns):
    for _ in range(num_turns):
        state = apply(state, "EndTurn")
    return state


def test_apply_is_pure_replaying_the_same_actions_from_a_clone_yields_identical_results():
    state = make_state()
    clone = copy.deepcopy(state)

    assert play_out(state, 5) == play_out(clone, 5)


def test_apply_is_pure_with_draw_and_reshuffle_in_play():
    # Mirrors the test above, but over a real deck — driving enough turns to
    # force at least one reshuffle (per the dedicated reshuffle test) — so the
    # embedded-PRNG-driven shuffle/draw/reshuffle stays exactly as replayable
    # as every other random event the engine models.
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        deck=list(deck),
    )
    clone = copy.deepcopy(state)

    replayed = play_out(state, 4)
    replayed_clone = play_out(clone, 4)

    assert replayed == replayed_clone
    assert replayed.hand == replayed_clone.hand
    assert replayed.draw_pile == replayed_clone.draw_pile
    assert replayed.discard_pile == replayed_clone.discard_pile


def test_apply_does_not_mutate_its_input():
    state = make_state()
    clone = copy.deepcopy(state)

    apply(state, "EndTurn")

    assert state == clone


def test_a_full_toy_fight_of_nothing_but_end_turn_reaches_a_terminal_state():
    state = make_state()

    while not is_terminal(state):
        assert legal_actions(state) == ["EndTurn"]
        state = apply(state, "EndTurn")

    assert is_terminal(state)
    assert state.player_hp <= 0 or state.monster_hp <= 0
    assert reward(state) != 0


def test_a_scripted_fight_with_strike_and_defend_runs_the_whole_stack_to_a_terminal_reward():
    # Per HOL-9: a single coherent run exercising the whole stack — pending
    # decisions/SelectTarget, the effect-op/event-bus damage pipeline, block
    # absorption, termination, and the shaped reward — from start to terminal.
    #
    # One Strike (targeted, deals damage) and one Defend (untargeted, grants
    # block) is all the starting energy allows; riding the rest out on EndTurn
    # alone can't kill a 44-HP monster, so this scripted opening always ends in
    # a loss, with the monster frozen at whatever HP the opening Strike left it.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=MONSTER_STARTING_HP,
        monster_attack=6,
        seed=42,
        hand=["Strike", "Defend"],
    )

    awaiting_target = apply(state, "PlayCard:Strike")
    struck = apply(awaiting_target, "SelectTarget:Monster")
    opening = apply(struck, "PlayCard:Defend")

    final_state = opening
    while not is_terminal(final_state):
        final_state = apply(final_state, "EndTurn")

    assert final_state.player_hp <= 0
    assert final_state.monster_hp == struck.monster_hp
    assert reward(final_state) == -1 * (final_state.monster_hp / MONSTER_STARTING_HP)


def test_with_rng_seed_produces_different_draws_for_different_seeds():
    # Use a 5-card deck so that the opening draw exhausts it entirely; EndTurn
    # then forces a reshuffle of the discard pile to refill the draw pile — the
    # shuffle uses the embedded PRNG, so different seeds produce different draw
    # orders on the very next turn.
    deck = ["Strike", "Strike", "Defend", "Defend", "Bash"]
    state = CombatState(
        player_hp=80, player_energy=3, monster_hp=44, monster_attack=6,
        seed=42, deck=list(deck),
    )

    from sts_sim import apply

    hands = set()
    for seed in range(20):
        reseeded = state.with_rng_seed(seed)
        after = apply(reseeded, "EndTurn")
        # Don't sort — we're checking that draw *order* differs, not just content.
        hands.add(tuple(after.hand))

    assert len(hands) > 1
