from sts_sim import (
    hp_lost_per_fight,
    legal_actions,
    mcts_action_values,
    mcts_search,
    simulate_hp_lost,
)
from sts_sim.bench import PRESETS
from sts_sim.sim.scenarios import ironclad_starter_deck_vs_jaw_worm


def test_mcts_action_values_returns_a_value_for_every_legal_action():
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)

    values = mcts_action_values(state, iterations=50, seed=42)

    for action in legal_actions(state):
        assert action in values
        assert -1.0 <= values[action] <= 1.0


def test_mcts_search_returns_a_legal_action():
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)

    action = mcts_search(state, iterations=50, seed=42)

    assert action in legal_actions(state)


def test_mcts_action_values_ranks_bash_before_end_turn_from_opening():
    # Same scenario as the Python mcts.py equivalent test — seed=0's opening
    # hand contains Bash, the strongest opener.
    state = ironclad_starter_deck_vs_jaw_worm(seed=0)

    values = mcts_action_values(state, iterations=100, seed=0)

    bash_action = next(a for a in values if "Bash" in a)
    assert values[bash_action] > values["EndTurn"]


def test_simulate_hp_lost_is_within_starting_hp():
    state = ironclad_starter_deck_vs_jaw_worm(seed=1)

    hp_lost = simulate_hp_lost(state, iterations=30, seed=1)

    assert 0 <= hp_lost <= state.player_hp


def test_hp_lost_per_fight_matches_starter_vs_jaw_worm_ballpark():
    # Cross-check against the experiment run via `sts_sim.bench` for the same
    # matchup (starter deck vs. Jaw Worm, ~11 HP lost on average) — this is
    # the entry point a future "does adding this card help?" tool would call.
    states = [ironclad_starter_deck_vs_jaw_worm(seed=seed) for seed in range(10)]

    outcomes = hp_lost_per_fight(states, iterations=50)

    assert len(outcomes) == 10
    avg = sum(outcomes) / len(outcomes)
    assert 5 < avg < 17


def test_hp_lost_per_fight_detects_removing_bash_is_worse():
    # Paired comparison: same seeds (hence common random numbers) for the
    # starter deck vs. "cut-bash" (starter deck with Bash removed). Losing
    # the deck's strongest card should clearly increase average HP lost
    # against Jaw Worm — this is the "does adding/removing this card help?"
    # question the card-pick tool answers. (`bench.compare` shows this is a
    # large, noise-dominating effect at these settings — ~11.5 vs ~15.2 over
    # 50 seeds — unlike smaller single-card swaps such as "add-strike".)
    seeds = range(20)
    starter_states = [
        ironclad_starter_deck_vs_jaw_worm(seed=seed, deck=PRESETS["starter"])
        for seed in seeds
    ]
    cut_bash_states = [
        ironclad_starter_deck_vs_jaw_worm(seed=seed, deck=PRESETS["cut-bash"])
        for seed in seeds
    ]

    starter_outcomes = hp_lost_per_fight(starter_states, iterations=50)
    cut_bash_outcomes = hp_lost_per_fight(cut_bash_states, iterations=50)

    starter_avg = sum(starter_outcomes) / len(starter_outcomes)
    cut_bash_avg = sum(cut_bash_outcomes) / len(cut_bash_outcomes)
    assert cut_bash_avg > starter_avg
