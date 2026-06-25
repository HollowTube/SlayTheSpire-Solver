"""Proves the minimal end-to-end run actually composes: the Overgrowth
weak/normal monster-pool draw (HOL-61) feeding RunState's path (HOL-59),
with card rewards (HOL-64) along the way — no elites, no rest sites, no
skeleton-assembly issue (HOL-65) required."""

from sts_sim import RunState, draw_overgrowth_monster_sequence, simulate_run_outcome
from sts_sim.scenarios import (
    IRONCLAD_STARTING_DECK,
    MONSTER_STARTING_HP,
    PLAYER_STARTING_HP,
)


def build_overgrowth_monster_only_run(seed: int, slots: int) -> RunState:
    """A monsters-and-card-rewards-only run: draw `slots` monster names from
    the Overgrowth pools and look up each one's canonical starting HP."""
    names = draw_overgrowth_monster_sequence(seed=seed, slots=slots)
    path = [(name, MONSTER_STARTING_HP[name]) for name in names]
    return RunState(
        seed=seed,
        deck=list(IRONCLAD_STARTING_DECK),
        hp=PLAYER_STARTING_HP,
        path=path,
    )


def test_every_drawn_monster_name_has_a_known_starting_hp():
    """The composition's one real assumption — that the pool draw never
    returns a name `MONSTER_STARTING_HP` doesn't know — checked directly
    rather than left implicit."""
    names = draw_overgrowth_monster_sequence(seed=1, slots=40)
    missing = set(names) - set(MONSTER_STARTING_HP)
    assert not missing


def test_a_drawn_overgrowth_path_runs_end_to_end():
    run = build_overgrowth_monster_only_run(seed=2, slots=4)
    won, final_hp, nodes_completed = simulate_run_outcome(run, iterations=200, seed=2)
    assert isinstance(won, bool)
    assert 0 <= final_hp <= PLAYER_STARTING_HP
    assert 0 <= nodes_completed <= 4
