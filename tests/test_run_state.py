"""Behavioural tests for RunState (HOL-59): a minimal end-to-end run loop
driving a fixed sequence of combat nodes, with combat resolved opaquely via
the existing MCTS engine and a pluggable run-level policy."""

from sts_sim import (
    RunState,
    run_apply,
    run_is_terminal,
    run_legal_actions,
    simulate_run_outcome,
    simulate_run_outcomes,
)


def test_legal_actions_offers_resolve_combat_at_a_fresh_combat_node():
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=[("Nibbit", 24)],
    )
    assert run_legal_actions(run) == ["ResolveCombat"]


def test_resolving_the_only_combat_node_ends_the_run_as_a_win():
    """A starter deck at full HP vs. a single weak Nibbit should win, leaving
    no further nodes — the run becomes terminal with player HP intact."""
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=[("Nibbit", 24)],
    )
    after = run_apply(run, "ResolveCombat")
    assert run_is_terminal(after)
    assert after.hp > 0


def test_hp_carries_into_the_combat_node_rather_than_resetting_to_max_hp():
    """Starting a node at less than max HP must feed that reduced HP into
    the embedded fight, not the run's max_hp — proven by starting well below
    max_hp and confirming the result never exceeds the carried-in HP (the
    engine has no healing, so HP can only go down from where it started)."""
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=10,
        max_hp=80,
        path=[("Nibbit", 24)],
    )
    after = run_apply(run, "ResolveCombat")
    assert after.hp <= 10


def test_an_unsurvivable_fight_ends_the_run_as_a_loss_with_hp_clamped_at_zero():
    """Starting a node at 1 HP makes Nibbit's opening 12-damage Butt
    unsurvivable regardless of play — the run must end terminal as a loss
    with HP at exactly 0 (never negative), even though a second node is
    still left unresolved on the path."""
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=1,
        path=[("Nibbit", 24), ("Nibbit", 24)],
    )
    after = run_apply(run, "ResolveCombat")
    assert after.hp == 0
    assert run_is_terminal(after)
    assert run_legal_actions(after) == []


def test_same_seed_reproduces_the_same_outcome():
    """A run's seed must fully determine the embedded combat's MCTS play —
    replaying the identical run from scratch gives an identical result."""

    def make_run():
        return RunState(
            seed=7,
            deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
            hp=80,
            path=[("Nibbit", 24)],
        )

    first = run_apply(make_run(), "ResolveCombat")
    second = run_apply(make_run(), "ResolveCombat")
    assert first.hp == second.hp


def test_simulate_run_outcome_plays_a_multi_node_path_to_completion():
    """The default run-policy (random-legal, seeded) drives a run with more
    than one node all the way to a terminal state and reports a sane
    (won, final_hp, nodes_completed) outcome — the bench-style entry point
    a future eval aggregator builds on."""
    run = RunState(
        seed=3,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=[("Nibbit", 24), ("Fuzzy Wurm Crawler", 24)],
    )
    won, final_hp, nodes_completed = simulate_run_outcome(run, iterations=200, seed=3)
    assert isinstance(won, bool)
    assert 0 <= final_hp <= 80
    assert 0 <= nodes_completed <= 2
    assert won == (final_hp > 0)


def test_deck_carries_into_the_combat_node_unchanged_with_no_rewards_yet():
    """No card-reward node exists yet (HOL-64) — the persistent deck must
    pass through a resolved combat node completely unchanged."""
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    run = RunState(seed=1, deck=deck, hp=80, path=[("Nibbit", 24)])
    after = run_apply(run, "ResolveCombat")
    assert sorted(after.deck) == sorted(deck)


def test_simulate_run_outcomes_reports_a_win_rate_over_many_seeded_runs():
    """The batch primitive a bench-style win-rate aggregator builds on —
    mirrors `fight_outcomes_per_fight`'s per-seed parallel pattern one level
    up, at the run granularity instead of the single-fight granularity."""
    runs = [
        RunState(
            seed=seed,
            deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
            hp=80,
            path=[("Nibbit", 24)],
        )
        for seed in range(10)
    ]
    outcomes = simulate_run_outcomes(runs, iterations=200)
    assert len(outcomes) == 10
    win_rate = sum(1 for won, _, _ in outcomes if won) / len(outcomes)
    assert win_rate > 0.5
