"""Behavioural tests for RunState (HOL-59): a minimal end-to-end run loop
driving a fixed sequence of combat nodes, with combat resolved opaquely via
the existing MCTS engine and a pluggable run-level policy."""

from sts_sim import (
    RunState,
    run_apply,
    run_is_terminal,
    run_legal_actions,
    simulate_run_outcome,
)


def test_legal_actions_offers_resolve_combat_at_a_fresh_combat_node():
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=["Nibbit"],
    )
    assert run_legal_actions(run) == ["ResolveCombat"]


def test_resolving_the_only_combat_node_ends_the_run_as_a_win():
    """A starter deck at full HP vs. a single weak Nibbit should win. With a
    card-reward decision now pending (HOL-64), the run is not yet terminal
    until that decision is resolved — Skip leaves no further nodes."""
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=["Nibbit"],
    )
    after_combat = run_apply(run, "ResolveCombat")
    assert not run_is_terminal(after_combat)
    after_reward = run_apply(after_combat, "Skip")
    assert run_is_terminal(after_reward)
    assert after_reward.hp > 0


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
        path=["Nibbit"],
    )
    after = run_apply(run, "ResolveCombat")
    assert after.hp <= 10


def test_an_unsurvivable_fight_ends_the_run_as_a_loss_with_hp_clamped_at_zero():
    """For this seed, starting a node at 1 HP loses to Nibbit's opening
    12-damage Butt — the run must end terminal as a loss with HP at exactly
    0 (never negative), even though a second node is still left unresolved
    on the path."""
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=1,
        path=["Nibbit", "Nibbit"],
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
            path=["Nibbit"],
        )

    first = run_apply(make_run(), "ResolveCombat")
    second = run_apply(make_run(), "ResolveCombat")
    assert first.hp == second.hp


def test_simulate_run_outcome_plays_a_multi_node_path_to_completion():
    """The default run-policy (random-legal, seeded) drives a run with more
    than one node all the way to a terminal state, traversing both weak
    early-game monsters rather than stopping short — the bench-style entry
    point a future eval aggregator builds on."""
    run = RunState(
        seed=3,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=["Nibbit", "Fuzzy Wurm Crawler"],
    )
    won, final_hp, nodes_completed = simulate_run_outcome(run, iterations=200, seed=3)
    assert won is True
    assert nodes_completed == 2
    assert final_hp > 0


def test_deck_is_unchanged_immediately_after_combat_before_any_reward_choice():
    """Resolving combat alone (before the pending reward decision is acted
    on) must not itself mutate the deck — only `Take:<name>` does."""
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    run = RunState(seed=1, deck=deck, hp=80, path=["Nibbit"])
    after = run_apply(run, "ResolveCombat")
    assert sorted(after.deck) == sorted(deck)


def test_a_won_combat_offers_a_card_reward_choice():
    """After winning, `legal_actions` must offer exactly 3 distinct cards to
    take plus Skip — before the run's path position has moved on."""
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=["Nibbit"],
    )
    after = run_apply(run, "ResolveCombat")
    actions = run_legal_actions(after)
    assert "Skip" in actions
    take_actions = [a for a in actions if a.startswith("Take:")]
    assert len(take_actions) == 3
    assert len(set(take_actions)) == 3


def test_status_cards_are_never_offered_as_rewards():
    """Dazed/Wound/Slimed/Infection are monster-inflicted junk cards, not
    legitimate rewards — `card_data`'s `CardType::Status` excludes them from
    the reward pool regardless of seed."""
    excluded = {"Dazed", "Wound", "Slimed", "Infection"}
    for seed in range(20):
        run = RunState(
            seed=seed,
            deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
            hp=80,
            path=["Nibbit"],
        )
        after = run_apply(run, "ResolveCombat")
        offered = {
            a.removeprefix("Take:")
            for a in run_legal_actions(after)
            if a.startswith("Take:")
        }
        assert offered.isdisjoint(excluded)


def test_skip_leaves_the_deck_unchanged_and_advances_the_run():
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    run = RunState(seed=1, deck=deck, hp=80, path=["Nibbit"])
    after_combat = run_apply(run, "ResolveCombat")
    after_skip = run_apply(after_combat, "Skip")
    assert sorted(after_skip.deck) == sorted(deck)
    assert run_is_terminal(after_skip)


def test_take_adds_the_chosen_card_to_the_deck_unupgraded():
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    run = RunState(seed=1, deck=deck, hp=80, path=["Nibbit"])
    after_combat = run_apply(run, "ResolveCombat")
    take_action = next(
        a for a in run_legal_actions(after_combat) if a.startswith("Take:")
    )
    chosen = take_action.removeprefix("Take:")
    after_take = run_apply(after_combat, take_action)
    assert sorted(after_take.deck) == sorted(deck + [chosen])
    assert run_is_terminal(after_take)


def test_card_reward_offering_is_deterministic_given_the_same_seed():
    def offer():
        run = RunState(
            seed=9,
            deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
            hp=80,
            path=["Nibbit"],
        )
        after = run_apply(run, "ResolveCombat")
        return sorted(run_legal_actions(after))

    assert offer() == offer()


def test_run_win_rate_reports_a_win_rate_over_many_seeded_runs():
    """`sts_sim.bench.run_win_rate` is the bench-style entry point: given N
    seeded runs, report the fraction that ended in a win — mirroring
    `BenchResult.win_rate`'s single-fight aggregation one level up."""
    from sts_sim.bench import run_win_rate

    runs = [
        RunState(
            seed=seed,
            deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
            hp=80,
            path=["Nibbit"],
        )
        for seed in range(10)
    ]
    assert run_win_rate(runs, iterations=200) > 0.5
