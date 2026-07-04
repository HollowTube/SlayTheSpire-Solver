"""Behavioural tests for the reward-lookahead policy (sts_sim.policies): at
a card-reward decision, simulate a few random rollouts of the rest of the
run for each candidate (every offered card, plus Skip) and pick whichever
leaves the highest average final HP. Built entirely on the existing public
RunState API (run_legal_actions/run_apply/run_is_terminal) — no Rust
changes."""

from sts_sim import RunState, run_apply
from sts_sim.policies import (
    evaluate_reward_options,
    lookahead_reward_policy,
    simulate_run_with_reward_lookahead,
)


def _run_at_a_reward_decision(seed=1):
    run = RunState(
        seed=seed,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=["Nibbit", "Nibbit"],
    )
    after_combat = run_apply(run, "ResolveCombat")
    return after_combat


def test_evaluate_reward_options_covers_every_offered_action():
    state = _run_at_a_reward_decision()
    scores = evaluate_reward_options(state, num_sims=2, iterations=100, seed=1)
    from sts_sim import run_legal_actions

    assert set(scores.keys()) == set(run_legal_actions(state))


def test_lookahead_reward_policy_returns_a_legal_action():
    from sts_sim import run_legal_actions

    state = _run_at_a_reward_decision()
    action = lookahead_reward_policy(state, num_sims=2, iterations=100, seed=1)
    assert action in run_legal_actions(state)


def test_lookahead_reward_policy_is_deterministic_given_the_same_seed():
    state = _run_at_a_reward_decision()
    first = lookahead_reward_policy(state, num_sims=2, iterations=100, seed=7)
    second = lookahead_reward_policy(state, num_sims=2, iterations=100, seed=7)
    assert first == second


def test_simulate_run_with_reward_lookahead_completes_and_is_deterministic():
    def play():
        run = RunState(
            seed=3,
            deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
            hp=80,
            path=["Nibbit", "Fuzzy Wurm Crawler"],
        )
        return simulate_run_with_reward_lookahead(
            run, num_sims=2, rollout_iterations=30, seed=3
        )

    first = play()
    second = play()
    assert first == second
    won, final_hp, nodes_completed = first
    assert isinstance(won, bool)
    assert 0 <= final_hp <= 80
    assert 0 <= nodes_completed <= 2


def test_lookahead_policy_beats_always_skip_on_average():
    """The policy's actual purpose — not just its shape. Deterministic
    (every seed and rollout is fixed), not a flaky coin flip: mean final
    HP over a fixed seed range must be at least as good as always
    skipping every reward, over the same seeds."""
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    path = ["Nibbit", "Fuzzy Wurm Crawler"]
    seeds = range(4)

    def lookahead_final_hp(seed):
        run = RunState(seed=seed, deck=deck, hp=80, path=path)
        _won, final_hp, _nodes = simulate_run_with_reward_lookahead(
            run, num_sims=2, rollout_iterations=15, seed=seed
        )
        return final_hp

    def always_skip_final_hp(seed):
        from sts_sim import run_is_terminal, run_legal_actions

        state = RunState(seed=seed, deck=deck, hp=80, path=path)
        while not run_is_terminal(state):
            actions = run_legal_actions(state)
            action = actions[0] if len(actions) == 1 else "Skip"
            state = run_apply(state, action)
        return state.hp

    lookahead_mean = sum(lookahead_final_hp(s) for s in seeds) / len(seeds)
    always_skip_mean = sum(always_skip_final_hp(s) for s in seeds) / len(seeds)
    assert lookahead_mean >= always_skip_mean
