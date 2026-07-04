"""Proves the minimal end-to-end run actually composes: the Overgrowth
weak/normal monster-pool draw (HOL-61) feeding RunState's path (HOL-59),
with card rewards (HOL-64) along the way — no elites, no rest sites, no
skeleton-assembly issue (HOL-65) required."""

from sts_sim import simulate_run_outcome
from sts_sim.bench import build_overgrowth_monster_only_run, run_overgrowth_win_rate
from sts_sim.scenarios import PLAYER_STARTING_HP


def test_a_drawn_overgrowth_path_runs_end_to_end():
    run = build_overgrowth_monster_only_run(seed=2, slots=4)
    won, final_hp, nodes_completed = simulate_run_outcome(run, iterations=200, seed=2)
    assert isinstance(won, bool)
    assert 0 <= final_hp <= PLAYER_STARTING_HP
    assert 0 <= nodes_completed <= 4


def test_run_overgrowth_win_rate_aggregates_many_seeded_overgrowth_runs():
    """The bench-style entry point for this e2e slice: build N seeded
    Overgrowth monsters-and-rewards runs and report win rate, mirroring
    `run_deck`'s convenience for single fights one level up."""
    win_rate = run_overgrowth_win_rate(seeds=10, slots=4, iterations=200)
    assert 0.0 <= win_rate <= 1.0
