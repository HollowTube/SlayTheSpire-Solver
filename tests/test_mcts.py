import random

from sts_sim import apply, is_terminal, legal_actions
from sts_sim.scenarios import ironclad_starter_deck_vs_jaw_worm
from sts_sim import mcts


def test_search_returns_a_legal_action_for_the_opening_decision():
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)

    action = mcts.search(state)

    assert action in legal_actions(state)


def test_search_returns_a_legal_action_mid_target_selection():
    # Per HOL-12's AC, search must work at *any* decision point — including
    # mid-resolution PendingDecision states like "select a target for the
    # Strike that's already been committed to" — not just the top-level
    # play-card-or-end-turn menu.
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)
    awaiting_target = apply(state, "PlayCard:Strike")
    assert awaiting_target.pending == "SelectTarget"

    action = mcts.search(awaiting_target)

    assert action == "SelectTarget:Monster"
    assert legal_actions(awaiting_target) == ["SelectTarget:Monster"]


def play_with_mcts_to_terminal(state, rng, iterations=50):
    while not is_terminal(state):
        action = mcts.search(state, iterations=iterations, rng=rng)
        state = apply(state, action)
    return state


def test_search_wins_the_fixed_scenario_at_a_reasonable_rate():
    # Per HOL-12's AC — the parent PRD's actual exit criterion: MCTS-driven
    # play must beat the fixed "starter deck vs. Jaw Worm" scenario at a
    # "reasonable rate" across repeated runs. HOL-11 established random play
    # wins only ~2.5% of the time (it takes deliberate Vulnerable-then-Strike
    # sequencing to clear 44 HP from a 38-raw-damage hand) — even a shallow
    # 50-iterations-per-decision search should crush that baseline by
    # actually evaluating sequencing rather than flailing randomly.
    #
    # Note: this isn't gambling against variance — the monster's RNG lives
    # inside the (cloned) CombatState, so from any given state the future is
    # fully determined by the actions chosen. The search is therefore solving
    # a deterministic, perfect-information tree per scenario seed, and wins
    # all 15/15 sampled seeds with total consistency run-to-run. The 60%
    # threshold over 10 scenario seeds isn't headroom against flakiness —
    # there isn't any — it's a meaningful bar that a dumb/broken search
    # (e.g. one that ignores Vulnerable sequencing, like random play's ~2.5%)
    # would fail outright.
    wins = 0
    runs = 10
    for seed in range(runs):
        state = ironclad_starter_deck_vs_jaw_worm(seed=seed)
        final = play_with_mcts_to_terminal(state, rng=random.Random(seed))
        if final.monster_hp <= 0:
            wins += 1

    assert wins / runs >= 0.6
