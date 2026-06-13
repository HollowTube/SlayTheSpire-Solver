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
    PLAYER_STARTING_HP,
    ironclad_starter_deck_vs_byrdonis,
    ironclad_starter_deck_vs_fuzzy_wurm_crawler,
    ironclad_starter_deck_vs_gremlin_nob,
    ironclad_starter_deck_vs_jaw_worm,
    ironclad_starter_deck_vs_nibbit,
)

_SCENARIOS = {
    "jaw-worm": ironclad_starter_deck_vs_jaw_worm,
    "gremlin-nob": ironclad_starter_deck_vs_gremlin_nob,
    "nibbit": ironclad_starter_deck_vs_nibbit,
    "fuzzy-wurm-crawler": ironclad_starter_deck_vs_fuzzy_wurm_crawler,
    "byrdonis": ironclad_starter_deck_vs_byrdonis,
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
    optimal_hp_outcomes: list[int] = field(default_factory=list)

    @property
    def hp_lost_outcomes(self) -> list[int]:
        return [PLAYER_STARTING_HP - hp for hp in self.hp_outcomes]

    @property
    def optimal_hp_lost_outcomes(self) -> list[int]:
        return [PLAYER_STARTING_HP - hp for hp in self.optimal_hp_outcomes]

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
    def mean_hp_lost(self) -> float:
        return statistics.mean(self.hp_lost_outcomes) if self.hp_lost_outcomes else 0.0

    @property
    def stderr_hp_lost(self) -> float:
        lost = self.hp_lost_outcomes
        if len(lost) < 2:
            return 0.0
        return statistics.stdev(lost) / math.sqrt(len(lost))

    @property
    def p25(self) -> float:
        lost = self.hp_lost_outcomes
        return statistics.quantiles(lost, n=4)[0] if lost else 0.0

    @property
    def p50(self) -> float:
        lost = self.hp_lost_outcomes
        return statistics.median(lost) if lost else 0.0

    @property
    def p75(self) -> float:
        lost = self.hp_lost_outcomes
        return statistics.quantiles(lost, n=4)[2] if lost else 0.0

    @property
    def mean_hp_lost_optimal(self) -> float:
        """Average HP lost under per-seed clairvoyant-optimal play (the
        `optimal_value` ceiling) — the best any sequence of actions could do
        given perfect foresight of each seed's RNG."""
        lost = self.optimal_hp_lost_outcomes
        return statistics.mean(lost) if lost else 0.0

    @property
    def regret(self) -> float:
        """`mean_hp_lost - mean_hp_lost_optimal`: how much extra HP the
        policy loses, on average, relative to the clairvoyant-optimal
        ceiling for the same seeds."""
        return self.mean_hp_lost - self.mean_hp_lost_optimal

    def __str__(self) -> str:
        line = (
            f"{self.label:<38}"
            f"  wins={self.wins}/{self.total}"
            f"  avg_lost={self.mean_hp_lost:5.1f} ±{self.stderr_hp_lost:.1f}"
            f"  p25/p50/p75={self.p25:.0f}/{self.p50:.0f}/{self.p75:.0f}"
        )
        if self.optimal_hp_outcomes:
            line += (
                f"  ceiling={self.mean_hp_lost_optimal:5.1f}"
                f"  regret={self.regret:5.1f}"
            )
        return line


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


def _run_one_optimal(args: tuple) -> int:
    """Clairvoyant-optimal ceiling worker — `optimal_value` on the starting
    state, converted to an equivalent final HP via the win-state identity
    `reward == player_hp_fraction` (a non-positive value means even perfect
    foresight can't avoid dying, i.e. final HP 0)."""
    seed, deck, monster_name = args
    from .. import optimal_value

    state = _SCENARIOS[monster_name](seed=seed, deck=deck)
    value = optimal_value(state)
    return round(max(value, 0.0) * PLAYER_STARTING_HP)


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
    ceiling: bool = False,
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
        ceiling: Also compute the per-seed `optimal_value` ceiling (exact
            branch-and-bound search, no rollouts) and report it alongside
            the policy's average HP loss as `regret`. Off by default since
            it's a separate search per seed on top of the policy runs.
    """
    if monster not in _SCENARIOS:
        raise ValueError(
            f"unknown monster {monster!r}; choices: {', '.join(_SCENARIOS)}"
        )

    result = BenchResult(label=label or repr(deck))

    arg_list: list[tuple]
    if policy == "mcts":
        worker_fn = _run_one_mcts
        arg_list = [(seed, deck, monster, iterations) for seed in range(seeds)]
    else:
        worker_fn = _run_one_random
        arg_list = [(seed, deck, monster) for seed in range(seeds)]

    hp_by_seed: dict[int, int] = {}
    optimal_hp_by_seed: dict[int, int] = {}
    with ProcessPoolExecutor(max_workers=workers) as pool:
        policy_futures = {pool.submit(worker_fn, args): args[0] for args in arg_list}
        ceiling_futures = (
            {
                pool.submit(_run_one_optimal, (seed, deck, monster)): seed
                for seed in range(seeds)
            }
            if ceiling
            else {}
        )
        for fut in as_completed(policy_futures):
            hp_by_seed[policy_futures[fut]] = fut.result()
        for fut in as_completed(ceiling_futures):
            optimal_hp_by_seed[ceiling_futures[fut]] = fut.result()

    result.hp_outcomes = [hp_by_seed[i] for i in range(seeds)]
    if ceiling:
        result.optimal_hp_outcomes = [optimal_hp_by_seed[i] for i in range(seeds)]
    return result


def compare(
    configs: dict[str, list[str] | None],
    monster: str = "jaw-worm",
    seeds: int = 50,
    iterations: int = 200,
    workers: int = 4,
    policy: str = "mcts",
    ceiling: bool = False,
) -> list[BenchResult]:
    """Benchmark each named deck in `configs` and print a comparison table.

    Args:
        configs: Dict mapping label → deck (card list or None for default).
        monster: Monster key.
        seeds: Fights per deck configuration.
        iterations: MCTS iterations per decision.
        workers: Parallel workers.
        policy: 'mcts' or 'random'.
        ceiling: Also report the `optimal_value` ceiling and regret — see
            `run_deck`.

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
            ceiling=ceiling,
        )
        print(f"{i:2}. {result}")
        results.append(result)
    return results
