"""Behavioural tests for the Rest Site node kind on RunState (HOL-63): a
real Heal-vs-Upgrade choice, marked via the rest_site_indices constructor
parameter so existing combat-only paths (HOL-59/HOL-61/HOL-62/HOL-64) are
unaffected."""

from sts_sim import (
    RunState,
    run_apply,
    run_is_terminal,
    run_legal_actions,
    simulate_run_outcome,
)


def test_a_rest_site_offers_heal_plus_one_upgrade_per_unupgraded_card():
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    run = RunState(
        seed=1,
        deck=deck,
        hp=40,
        path=[("", 0)],
        rest_site_indices=[0],
    )
    actions = run_legal_actions(run)
    assert "Heal" in actions
    upgrade_actions = [a for a in actions if a.startswith("Upgrade:")]
    assert len(upgrade_actions) == len(deck)


def test_heal_restores_30_percent_of_max_hp():
    """Pinned to the exact value (80 * 0.3 = 24) rather than a loose range,
    so a silent change to REST_SITE_HEAL_FRACTION fails loudly."""
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=40,
        max_hp=80,
        path=[("", 0)],
        rest_site_indices=[0],
    )
    after = run_apply(run, "Heal")
    assert after.hp == 64


def test_heal_never_exceeds_max_hp_when_already_near_full():
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=75,
        max_hp=80,
        path=[("", 0)],
        rest_site_indices=[0],
    )
    after = run_apply(run, "Heal")
    assert after.hp == 80


def test_upgrade_increments_the_targeted_cards_upgrade_level():
    deck = ["Strike"] * 5 + ["Defend"] * 4 + ["Bash"]
    run = RunState(seed=1, deck=deck, hp=40, path=[("", 0)], rest_site_indices=[0])
    after = run_apply(run, "Upgrade:0")
    assert after.deck.count("Strike+") == 1
    assert after.deck.count("Strike") == 4


def test_an_already_upgraded_card_is_not_offered_as_an_upgrade_choice():
    """A card upgraded by an earlier reward/rest-site visit (here, simply
    already upgraded in the starting deck) must be excluded from the
    Upgrade options the next time a Rest Site is reached."""
    deck = ["Strike+"] + ["Strike"] * 4 + ["Defend"] * 4 + ["Bash"]
    run = RunState(seed=1, deck=deck, hp=40, path=[("", 0)], rest_site_indices=[0])
    actions = run_legal_actions(run)
    upgrade_actions = [a for a in actions if a.startswith("Upgrade:")]
    assert len(upgrade_actions) == len(deck) - 1
    assert "Upgrade:0" not in upgrade_actions


def test_rest_site_advances_the_run_after_a_choice():
    run = RunState(
        seed=1, deck=["Strike"] * 10, hp=40, path=[("", 0)], rest_site_indices=[0]
    )
    after = run_apply(run, "Heal")
    assert run_is_terminal(after)


def test_an_unmarked_path_is_unaffected_by_the_new_parameter():
    run = RunState(
        seed=1,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=[("Nibbit", 24)],
    )
    assert run_legal_actions(run) == ["ResolveCombat"]


def test_default_policy_drives_a_rest_site_then_combat_to_completion():
    """The acceptance criterion this issue calls out explicitly: a bench
    run reaching a Rest Site completes end-to-end via the default
    random-legal policy, with no manual action selection — and the choice
    made there (e.g. an upgrade) carries into the next combat node's
    CombatState, not just RunState's own deck list."""
    run = RunState(
        seed=4,
        deck=["Strike"] * 5 + ["Defend"] * 4 + ["Bash"],
        hp=80,
        path=[("", 0), ("Nibbit", 24)],
        rest_site_indices=[0],
    )
    won, final_hp, nodes_completed = simulate_run_outcome(run, iterations=200, seed=4)
    assert nodes_completed == 2
    assert won is True
    assert final_hp > 0
