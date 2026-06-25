"""Behavioural tests for the assembled Overgrowth v1 run skeleton (HOL-65):
a fixed, hand-picked node sequence (3 weak monsters -> 2 normal monsters ->
elite -> 2 normal monsters -> rest site -> elite -> rest site -> Vantom
boss) wiring together the monster-pool draw (HOL-61), elite pool (HOL-62),
rest sites (HOL-63), and card rewards (HOL-64) — none of which had to be
hand-edited to build this; it's pure composition on top of their already-
merged, isolated mechanisms."""

from sts_sim import run_apply, run_is_terminal, run_legal_actions, simulate_run_outcome
from sts_sim.bench import build_overgrowth_run, run_overgrowth_skeleton_win_rate

SKELETON_LENGTH = 12


def test_skeleton_traverses_all_twelve_fixed_nodes():
    """With an absurdly high HP pool (so the run structurally can't end
    early from a single fight's damage), the run must still traverse every
    node in the fixed 12-node skeleton — proving the skeleton's length and
    shape, independent of how any individual fight plays out."""
    run = build_overgrowth_run(seed=1, hp=999_999)
    _won, _final_hp, nodes_completed = simulate_run_outcome(run, iterations=200, seed=1)
    assert nodes_completed == SKELETON_LENGTH


def test_skeleton_offers_exactly_two_rest_sites_and_ten_combats():
    """Asserting the traversal *length* alone can't distinguish a correctly
    shaped skeleton from a same-length one with the wrong node kinds (e.g.
    a rest site silently swapped for a regular monster) — count the actual
    decision-type vocabulary instead. A Rest Site is the only node kind
    that ever offers `Heal`; Combat/Elite nodes only ever offer
    `ResolveCombat`. Reward decisions (`Take:`/`Skip`) interleave and don't
    advance `position`, so counting by action vocabulary rather than
    position index is what stays correct regardless of how many rewards
    fire along the way."""
    state = build_overgrowth_run(seed=1, hp=999_999)
    heal_offers = 0
    resolve_combat_offers = 0
    while not run_is_terminal(state):
        actions = run_legal_actions(state)
        if "Heal" in actions:
            heal_offers += 1
            action = "Heal"
        elif actions == ["ResolveCombat"]:
            resolve_combat_offers += 1
            action = "ResolveCombat"
        else:
            action = "Skip"  # a pending card-reward decision
        state = run_apply(state, action)
    assert heal_offers == 2
    assert resolve_combat_offers == 10


def test_same_seed_reproduces_an_identical_full_run():
    def play(seed):
        run = build_overgrowth_run(seed=seed)
        return simulate_run_outcome(run, iterations=200, seed=seed)

    assert play(5) == play(5)


def test_different_seeds_produce_different_first_node_outcomes():
    """If the skeleton hardcoded monster names instead of drawing from the
    seeded pools, every seed would resolve the first combat node against
    the identical monster, and (combined with a fixed starting deck) the
    embedded fight would behave identically too. Across enough seeds, at
    least one pair must differ — this fails if monster selection were
    hardcoded instead of seeded."""
    from sts_sim import run_apply

    outcomes = {
        run_apply(build_overgrowth_run(seed=seed), "ResolveCombat").hp
        for seed in range(8)
    }
    assert len(outcomes) > 1


def test_run_overgrowth_skeleton_win_rate_reports_a_sane_win_rate():
    rate = run_overgrowth_skeleton_win_rate(seeds=10, iterations=200)
    assert 0.0 <= rate <= 1.0
