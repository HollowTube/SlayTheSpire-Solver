import copy

from sts_sim import CombatState, apply, evaluate, is_terminal, legal_actions, reward


def make_state():
    return CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
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
    won = CombatState(player_hp=80, player_energy=3, monster_hp=0, monster_attack=6, seed=42)

    assert reward(won) > 0


def test_reward_is_negative_when_the_player_loses():
    lost = CombatState(player_hp=0, player_energy=3, monster_hp=44, monster_attack=6, seed=42)

    assert reward(lost) < 0


def test_reward_rewards_decisive_wins_more_than_narrow_wins():
    decisive_win = CombatState(player_hp=80, player_energy=3, monster_hp=0, monster_attack=6, seed=42)
    narrow_win = CombatState(player_hp=1, player_energy=3, monster_hp=0, monster_attack=6, seed=42)

    assert reward(decisive_win) > reward(narrow_win)


def test_reward_is_zero_for_a_non_terminal_state():
    state = make_state()

    assert reward(state) == 0


def test_evaluate_favours_the_side_with_more_remaining_hp():
    player_ahead = CombatState(player_hp=80, player_energy=3, monster_hp=10, monster_attack=6, seed=42)
    monster_ahead = CombatState(player_hp=10, player_energy=3, monster_hp=80, monster_attack=6, seed=42)

    assert evaluate(player_ahead) > evaluate(monster_ahead)


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
