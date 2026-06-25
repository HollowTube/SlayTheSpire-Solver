"""Behavioural tests for the Elite node kind on RunState (HOL-62): resolved
the same opaque way a regular combat node is, just sourced from a separate
elite pool — marked via the elite_indices constructor parameter so existing
all-regular-monster paths (HOL-59/HOL-61/HOL-64) are unaffected."""

from sts_sim import RunState, run_apply, run_is_terminal, run_legal_actions


def test_an_elite_node_offers_resolve_combat_just_like_a_regular_node():
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=[("Byrdonis", 84)],
        elite_indices=[0],
    )
    assert run_legal_actions(run) == ["ResolveCombat"]


def test_an_elite_node_runs_through_the_standard_win_reward_skip_flow():
    """An elite node resolves through the same opaque win -> pending-reward
    -> Skip flow a regular combat node does (HOL-64) — the resolution path
    is identical by construction (`NodeKind::monster()` hands the name to
    `Monster::new` the same way for both variants), so this checks the
    flow, not the moveset itself."""
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=[("Byrdonis", 84)],
        elite_indices=[0],
    )
    after_combat = run_apply(run, "ResolveCombat")
    assert not run_is_terminal(after_combat)  # reward pending if it won
    after_reward = run_apply(after_combat, "Skip")
    assert run_is_terminal(after_reward)


def test_an_unmarked_path_is_unaffected_by_the_new_parameter():
    """Existing all-regular-monster paths (no elite_indices) keep working
    exactly as before — the parameter is purely additive."""
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=[("Nibbit", 24)],
    )
    assert run_legal_actions(run) == ["ResolveCombat"]
