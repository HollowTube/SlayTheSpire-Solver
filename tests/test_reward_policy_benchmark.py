"""Shape tests for the run-level reward-policy A/B benchmark
(sts_sim.bench.compare_reward_policies): the lookahead card-reward picker vs.
the default random-legal choice, both played through the same `run_apply`
combat path so only the reward decision differs. Kept to tiny params — each
lookahead run resolves many embedded combats, so this is deliberately a
smoke/shape test, not a strength assertion (lookahead > random can flip at
low num_sims and is not guaranteed)."""

from sts_sim.bench import _REWARD_RUN_BUILDERS, compare_reward_policies


def test_compare_reward_policies_returns_paired_results_of_expected_shape():
    seeds = 2
    random_result, lookahead_result = compare_reward_policies(
        "overgrowth-monsters",
        seeds=seeds,
        num_sims=2,
        rollout_iterations=6,
        workers=2,
        builder_kwargs={"slots": 2},
    )

    for result in (random_result, lookahead_result):
        assert len(result.hp_outcomes) == seeds
        assert result.total == seeds
        assert 0.0 <= result.win_rate <= 1.0
        # Final HP is bounded: dead (0) through full starting HP.
        assert all(0 <= hp for hp in result.hp_outcomes)


def test_compare_reward_policies_rejects_unknown_builder():
    import pytest

    with pytest.raises(ValueError, match="unknown builder"):
        compare_reward_policies("not-a-builder", seeds=1)


def test_reward_run_builders_are_all_callable():
    assert "overgrowth-monsters" in _REWARD_RUN_BUILDERS
    assert "overgrowth-skeleton" in _REWARD_RUN_BUILDERS
    assert all(callable(fn) for fn in _REWARD_RUN_BUILDERS.values())
