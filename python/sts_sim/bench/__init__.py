"""Deck benchmarking toolkit for sts_sim.

Usage:
    from sts_sim.bench import run_deck, compare, PRESETS

    result = run_deck(PRESETS["starter"], monster="jaw-worm")
    print(result)

    compare(PRESETS, monster="gremlin-nob")
"""

import math
import statistics
from concurrent.futures import ProcessPoolExecutor, as_completed
from dataclasses import dataclass, field

from ..scenarios import (
    ironclad_starter_deck_vs_gremlin_nob,
    ironclad_starter_deck_vs_jaw_worm,
)

_SCENARIOS = {
    "jaw-worm": ironclad_starter_deck_vs_jaw_worm,
    "gremlin-nob": ironclad_starter_deck_vs_gremlin_nob,
}

# Named deck configurations. Each is a list of card names passed as `deck` to
# the scenario constructor. None means "use the scenario's default deck."
PRESETS: dict[str, list[str] | None] = {
    "starter": None,
    "cut-defend": ["Strike"] * 5 + ["Defend"] * 3 + ["Bash"],
    "cut-2-defends": ["Strike"] * 5 + ["Defend"] * 2 + ["Bash"],
    "cut-bash": ["Strike"] * 5 + ["Defend"] * 4,
    "add-strike": ["Strike"] * 6 + ["Defend"] * 4 + ["Bash"],
    "add-iron-wave": ["Strike"] * 5 + ["Defend"] * 4 + ["Bash", "Iron Wave"],
    "add-inflame": ["Strike"] * 5 + ["Defend"] * 4 + ["Bash", "Inflame"],
    "swap-defend-for-iron-wave": ["Strike"] * 5
    + ["Defend"] * 3
    + ["Bash", "Iron Wave"],
}


@dataclass
class BenchResult:
    label: str
    hp_outcomes: list[int] = field(default_factory=list)

    @property
    def total(self) -> int:
        return len(self.hp_outcomes)

    @property
    def wins(self) -> int:
        return sum(1 for hp in self.hp_outcomes if hp > 0)

    @property
    def win_rate(self) -> float:
        return self.wins / self.total if self.total else 0.0

    @property
    def winning_hp(self) -> list[int]:
        return [hp for hp in self.hp_outcomes if hp > 0]

    @property
    def mean_hp(self) -> float:
        return statistics.mean(self.hp_outcomes) if self.hp_outcomes else 0.0

    @property
    def stderr_hp(self) -> float:
        if len(self.hp_outcomes) < 2:
            return 0.0
        return statistics.stdev(self.hp_outcomes) / math.sqrt(len(self.hp_outcomes))

    @property
    def p25(self) -> float:
        return (
            statistics.quantiles(self.hp_outcomes, n=4)[0] if self.hp_outcomes else 0.0
        )

    @property
    def p50(self) -> float:
        return statistics.median(self.hp_outcomes) if self.hp_outcomes else 0.0

    @property
    def p75(self) -> float:
        return (
            statistics.quantiles(self.hp_outcomes, n=4)[2] if self.hp_outcomes else 0.0
        )

    def __str__(self) -> str:
        winning = self.winning_hp
        avg_win_hp = f"{statistics.mean(winning):.1f}" if winning else "n/a"
        return (
            f"{self.label:<38}"
            f"  wins={self.wins}/{self.total}"
            f"  avg_hp={self.mean_hp:5.1f} ±{self.stderr_hp:.1f}"
            f"  win_hp={avg_win_hp}"
            f"  p25/p50/p75={self.p25:.0f}/{self.p50:.0f}/{self.p75:.0f}"
        )


def _run_one_mcts(args: tuple) -> int:
    """MCTS-guided fight worker — must be a top-level function for pickling."""
    seed, deck, monster_name, iterations = args
    from .. import apply, is_terminal
    from ..mcts import search

    state = _SCENARIOS[monster_name](seed=seed, deck=deck)
    while not is_terminal(state):
        action = search(state, iterations=iterations)
        state = apply(state, action)
    return state.player_hp


def _run_one_random(args: tuple) -> int:
    """Random-policy fight worker — fast baseline."""
    import random

    seed, deck, monster_name = args
    from .. import apply, is_terminal, legal_actions

    rng = random.Random(seed)
    state = _SCENARIOS[monster_name](seed=seed, deck=deck)
    while not is_terminal(state):
        state = apply(state, rng.choice(legal_actions(state)))
    return state.player_hp


def run_deck(
    deck: list[str] | None,
    monster: str = "jaw-worm",
    seeds: int = 50,
    iterations: int = 200,
    workers: int = 4,
    label: str = "",
    policy: str = "mcts",
) -> BenchResult:
    """Run `seeds` fights with the given deck and return a BenchResult.

    Args:
        deck: Card list (or None for the scenario default).
        monster: Monster key — one of 'jaw-worm', 'gremlin-nob'.
        seeds: Number of independent seeds (fights) to run.
        iterations: MCTS iterations per decision (ignored when policy='random').
        workers: Parallel worker processes.
        label: Human-readable name shown in reports.
        policy: 'mcts' (default) or 'random'.
    """
    if monster not in _SCENARIOS:
        raise ValueError(
            f"unknown monster {monster!r}; choices: {', '.join(_SCENARIOS)}"
        )

    result = BenchResult(label=label or repr(deck))

    if policy == "mcts":
        worker_fn = _run_one_mcts
        arg_list = [(seed, deck, monster, iterations) for seed in range(seeds)]
    else:
        worker_fn = _run_one_random
        arg_list = [(seed, deck, monster) for seed in range(seeds)]

    outcomes = [None] * seeds
    with ProcessPoolExecutor(max_workers=workers) as pool:
        futures = {pool.submit(worker_fn, args): args[0] for args in arg_list}
        for fut in as_completed(futures):
            outcomes[futures[fut]] = fut.result()

    result.hp_outcomes = outcomes
    return result


def compare(
    configs: dict[str, list[str] | None],
    monster: str = "jaw-worm",
    seeds: int = 50,
    iterations: int = 200,
    workers: int = 4,
    policy: str = "mcts",
) -> list[BenchResult]:
    """Benchmark each named deck in `configs` and print a comparison table.

    Args:
        configs: Dict mapping label → deck (card list or None for default).
        monster: Monster key.
        seeds: Fights per deck configuration.
        iterations: MCTS iterations per decision.
        workers: Parallel workers.
        policy: 'mcts' or 'random'.

    Returns:
        List of BenchResult in insertion order.
    """
    results = []
    for i, (label, deck) in enumerate(configs.items(), 1):
        result = run_deck(
            deck,
            monster=monster,
            seeds=seeds,
            iterations=iterations,
            workers=workers,
            label=label,
            policy=policy,
        )
        print(f"{i:2}. {result}")
        results.append(result)
    return results
