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


def test_fresh_state_only_legal_action_is_end_turn():
    state = make_state()

    assert legal_actions(state) == ["EndTurn"]


def test_end_turn_advances_the_turn_counter():
    state = make_state()

    next_state = apply(state, "EndTurn")

    assert next_state.turn == state.turn + 1


def test_end_turn_applies_the_monsters_fixed_attack_to_the_player():
    state = make_state()

    next_state = apply(state, "EndTurn")

    assert next_state.player_hp == state.player_hp - state.monster_attack


def test_fresh_state_is_not_terminal():
    state = make_state()

    assert is_terminal(state) is False


def test_state_is_terminal_once_player_hp_reaches_zero():
    state = CombatState(player_hp=5, player_energy=3, monster_hp=44, monster_attack=6, seed=42)

    next_state = apply(state, "EndTurn")

    assert next_state.player_hp <= 0
    assert is_terminal(next_state) is True


def test_state_is_terminal_once_monster_hp_reaches_zero():
    state = CombatState(player_hp=80, player_energy=3, monster_hp=0, monster_attack=6, seed=42)

    assert is_terminal(state) is True


def test_reward_is_positive_when_the_player_wins():
    won = CombatState(player_hp=80, player_max_hp=80, monster_hp=0, monster_max_hp=44,
                      player_energy=3, monster_attack=6, seed=42)

    assert reward(won) > 0


def test_reward_is_negative_when_the_player_loses():
    lost = CombatState(player_hp=0, player_max_hp=80, monster_hp=44, monster_max_hp=44,
                       player_energy=3, monster_attack=6, seed=42)

    assert reward(lost) < 0


def test_reward_rewards_decisive_wins_more_than_narrow_wins():
    decisive_win = CombatState(player_hp=80, player_max_hp=80, monster_hp=0, monster_max_hp=44,
                               player_energy=3, monster_attack=6, seed=42)
    narrow_win = CombatState(player_hp=1, player_max_hp=80, monster_hp=0, monster_max_hp=44,
                             player_energy=3, monster_attack=6, seed=42)

    assert reward(decisive_win) > reward(narrow_win)


def test_reward_is_zero_for_a_non_terminal_state():
    state = make_state()

    assert reward(state) == 0


# Per HOL-9, the shaped reward is `(+1 if win else -1) * hp_fraction`, where
# hp_fraction is the *winning* side's remaining HP as a fraction of its max —
# this is what an actual STS run optimizes for (player HP carries between
# fights) and what an eventual RL value head learns to predict.
def test_reward_matches_the_shaped_formula_for_a_win_at_partial_hp():
    won_at_half_hp = CombatState(player_hp=40, player_max_hp=80, monster_hp=0, monster_max_hp=44,
                                 player_energy=3, monster_attack=6, seed=42)

    assert reward(won_at_half_hp) == 0.5


def test_reward_matches_the_shaped_formula_for_a_loss_with_the_monster_at_partial_hp():
    # Chip the monster down before the player dies, so the monster is at a
    # known, *reachable* fraction of its starting HP at the moment of loss —
    # then check the shaped reward against that fraction directly, rather than
    # constructing an artificial "pre-damaged" monster (which can't occur in
    # real play, since monsters always start an encounter at full HP).
    weak_player = CombatState(player_hp=5, player_energy=3, monster_hp=MONSTER_STARTING_HP,
                              monster_attack=6, seed=42, hand=["Strike"])
    chipped = apply(apply(weak_player, "PlayCard:Strike"), "SelectTarget:Monster")
    lost = apply(chipped, "EndTurn")

    assert is_terminal(lost) and lost.player_hp <= 0 and lost.monster_hp > 0
    assert reward(lost) == -1 * (lost.monster_hp / MONSTER_STARTING_HP)


def test_evaluate_favours_the_side_with_relatively_more_remaining_hp():
    armed = CombatState(player_hp=80, player_energy=3, monster_hp=MONSTER_STARTING_HP,
                        monster_attack=6, seed=42, hand=["Strike"])

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
    state = CombatState(player_hp=80, player_energy=3, monster_hp=MONSTER_STARTING_HP,
                        monster_attack=6, seed=42, hand=["Strike", "Defend"])

    awaiting_target = apply(state, "PlayCard:Strike")
    struck = apply(awaiting_target, "SelectTarget:Monster")
    opening = apply(struck, "PlayCard:Defend")

    final_state = opening
    while not is_terminal(final_state):
        final_state = apply(final_state, "EndTurn")

    assert final_state.player_hp <= 0
    assert final_state.monster_hp == struck.monster_hp
    assert reward(final_state) == -1 * (final_state.monster_hp / MONSTER_STARTING_HP)
