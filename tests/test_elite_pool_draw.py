"""Behavioural tests for the Overgrowth elite-pool draw (HOL-62): a separate
pool from the weak/normal monster pools, drawn uniform-random-with-seed."""

from sts_sim import draw_overgrowth_elite

ELITE_POOL = {"Byrdonis", "Phrog Parasite", "Bygone Effigy"}


def test_draw_returns_a_name_from_the_elite_pool():
    assert draw_overgrowth_elite(seed=1) in ELITE_POOL


def test_draw_is_deterministic_given_the_same_seed():
    assert draw_overgrowth_elite(seed=7) == draw_overgrowth_elite(seed=7)


def test_draw_visits_every_elite_across_many_seeds():
    """Pins the pool's exact membership — a hand-copied expected set could
    otherwise drift silently from what the pool actually contains."""
    drawn = {draw_overgrowth_elite(seed=seed) for seed in range(50)}
    assert drawn == ELITE_POOL
