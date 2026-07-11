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
from enum import Enum
from typing import TYPE_CHECKING, cast

if TYPE_CHECKING:
    from collections.abc import Callable

    from .. import RunState

from ..scenarios import (
    PLAYER_STARTING_HP,
    CardName,
    ironclad_starter_deck_vs_assassin_ruby_raider,
    ironclad_starter_deck_vs_axe_ruby_raider,
    ironclad_starter_deck_vs_brute_ruby_raider,
    ironclad_starter_deck_vs_bygone_effigy,
    ironclad_starter_deck_vs_byrdonis,
    ironclad_starter_deck_vs_ceremonial_beast,
    ironclad_starter_deck_vs_crossbow_ruby_raider,
    ironclad_starter_deck_vs_cubex_construct,
    ironclad_starter_deck_vs_flyconid,
    ironclad_starter_deck_vs_fogmog,
    ironclad_starter_deck_vs_fuzzy_wurm_crawler,
    ironclad_starter_deck_vs_gremlin_nob,
    ironclad_starter_deck_vs_inklet,
    ironclad_starter_deck_vs_inklets,
    ironclad_starter_deck_vs_jaw_worm,
    ironclad_starter_deck_vs_leaf_slime_m,
    ironclad_starter_deck_vs_leaf_slime_s,
    ironclad_starter_deck_vs_mawler,
    ironclad_starter_deck_vs_nibbit,
    ironclad_starter_deck_vs_phrog_parasite,
    ironclad_starter_deck_vs_ruby_raiders,
    ironclad_starter_deck_vs_shrinker_beetle,
    ironclad_starter_deck_vs_slimes_weak,
    ironclad_starter_deck_vs_slimes_weak_twig,
    ironclad_starter_deck_vs_slithering_strangler,
    ironclad_starter_deck_vs_snapping_jaxfruit,
    ironclad_starter_deck_vs_the_kin,
    ironclad_starter_deck_vs_tracker_ruby_raider,
    ironclad_starter_deck_vs_twig_slime_m,
    ironclad_starter_deck_vs_twig_slime_s,
    ironclad_starter_deck_vs_vantom,
    ironclad_starter_deck_vs_vine_shambler,
    ironclad_starter_deck_vs_wriggler,
)


class Encounter(str, Enum):
    """Canonical keys for `run_deck`/`compare`'s `monster=` argument — one
    per scenario in `sts_sim.scenarios`. A `str` subclass, so existing code
    passing plain strings (e.g. `monster="jaw-worm"`) keeps working —
    `Encounter.JAW_WORM == "jaw-worm"` is true and hashes the same, so dict
    lookups and equality checks behave identically either way. Exists so
    notebook/bench callers get autocomplete/typo-checking instead of
    repeating string literals."""

    JAW_WORM = "jaw-worm"
    GREMLIN_NOB = "gremlin-nob"
    NIBBIT = "nibbit"
    FUZZY_WURM_CRAWLER = "fuzzy-wurm-crawler"
    TWIG_SLIME_S = "twig-slime-s"
    SHRINKER_BEETLE = "shrinker-beetle"
    LEAF_SLIME_S = "leaf-slime-s"
    LEAF_SLIME_M = "leaf-slime-m"
    TWIG_SLIME_M = "twig-slime-m"
    SLIMES_WEAK = "slimes-weak"
    SLIMES_WEAK_TWIG = "slimes-weak-twig"
    ASSASSIN_RUBY_RAIDER = "assassin-ruby-raider"
    AXE_RUBY_RAIDER = "axe-ruby-raider"
    BRUTE_RUBY_RAIDER = "brute-ruby-raider"
    BYRDONIS = "byrdonis"
    CROSSBOW_RUBY_RAIDER = "crossbow-ruby-raider"
    CUBEX_CONSTRUCT = "cubex-construct"
    INKLET = "inklet"
    INKLETS = "inklets"
    PHROG_PARASITE = "phrog-parasite"
    RUBY_RAIDERS = "ruby-raiders"
    SLITHERING_STRANGLER = "slithering-strangler"
    SNAPPING_JAXFRUIT = "snapping-jaxfruit"
    THE_KIN = "the-kin"
    VANTOM = "vantom"
    TRACKER_RUBY_RAIDER = "tracker-ruby-raider"
    MAWLER = "mawler"
    VINE_SHAMBLER = "vine-shambler"
    WRIGGLER = "wriggler"
    BYGONE_EFFIGY = "bygone-effigy"
    FLYCONID = "flyconid"
    FOGMOG = "fogmog"
    CEREMONIAL_BEAST = "ceremonial-beast"


_SCENARIOS = {
    Encounter.ASSASSIN_RUBY_RAIDER: ironclad_starter_deck_vs_assassin_ruby_raider,
    Encounter.AXE_RUBY_RAIDER: ironclad_starter_deck_vs_axe_ruby_raider,
    Encounter.BRUTE_RUBY_RAIDER: ironclad_starter_deck_vs_brute_ruby_raider,
    Encounter.BYGONE_EFFIGY: ironclad_starter_deck_vs_bygone_effigy,
    Encounter.BYRDONIS: ironclad_starter_deck_vs_byrdonis,
    Encounter.CEREMONIAL_BEAST: ironclad_starter_deck_vs_ceremonial_beast,
    Encounter.CROSSBOW_RUBY_RAIDER: ironclad_starter_deck_vs_crossbow_ruby_raider,
    Encounter.CUBEX_CONSTRUCT: ironclad_starter_deck_vs_cubex_construct,
    Encounter.FLYCONID: ironclad_starter_deck_vs_flyconid,
    Encounter.FOGMOG: ironclad_starter_deck_vs_fogmog,
    Encounter.FUZZY_WURM_CRAWLER: ironclad_starter_deck_vs_fuzzy_wurm_crawler,
    Encounter.GREMLIN_NOB: ironclad_starter_deck_vs_gremlin_nob,
    Encounter.INKLET: ironclad_starter_deck_vs_inklet,
    Encounter.INKLETS: ironclad_starter_deck_vs_inklets,
    Encounter.JAW_WORM: ironclad_starter_deck_vs_jaw_worm,
    Encounter.LEAF_SLIME_M: ironclad_starter_deck_vs_leaf_slime_m,
    Encounter.LEAF_SLIME_S: ironclad_starter_deck_vs_leaf_slime_s,
    Encounter.MAWLER: ironclad_starter_deck_vs_mawler,
    Encounter.NIBBIT: ironclad_starter_deck_vs_nibbit,
    Encounter.PHROG_PARASITE: ironclad_starter_deck_vs_phrog_parasite,
    Encounter.RUBY_RAIDERS: ironclad_starter_deck_vs_ruby_raiders,
    Encounter.SHRINKER_BEETLE: ironclad_starter_deck_vs_shrinker_beetle,
    Encounter.SLIMES_WEAK: ironclad_starter_deck_vs_slimes_weak,
    Encounter.SLIMES_WEAK_TWIG: ironclad_starter_deck_vs_slimes_weak_twig,
    Encounter.SLITHERING_STRANGLER: ironclad_starter_deck_vs_slithering_strangler,
    Encounter.SNAPPING_JAXFRUIT: ironclad_starter_deck_vs_snapping_jaxfruit,
    Encounter.THE_KIN: ironclad_starter_deck_vs_the_kin,
    Encounter.TRACKER_RUBY_RAIDER: ironclad_starter_deck_vs_tracker_ruby_raider,
    Encounter.TWIG_SLIME_M: ironclad_starter_deck_vs_twig_slime_m,
    Encounter.TWIG_SLIME_S: ironclad_starter_deck_vs_twig_slime_s,
    Encounter.VANTOM: ironclad_starter_deck_vs_vantom,
    Encounter.VINE_SHAMBLER: ironclad_starter_deck_vs_vine_shambler,
    Encounter.WRIGGLER: ironclad_starter_deck_vs_wriggler,
}

# Act 1 "easy pool" encounters — every non-elite, non-boss scenario (i.e.
# everything except the Gremlin Nob/Byrdonis elites and the Vantom boss).
EASY_POOL = [
    e
    for e in Encounter
    if e not in (Encounter.GREMLIN_NOB, Encounter.BYRDONIS, Encounter.VANTOM)
]

# Named deck configurations. Each is a list of card names passed as `deck` to
# the scenario constructor. None means "use the scenario's default deck."
PRESETS: dict[str, list[str] | None] = {
    "starter": None,
    "cut-defend": cast(
        "list[str]", [CardName.STRIKE] * 5 + [CardName.DEFEND] * 3 + [CardName.BASH]
    ),
    "cut-2-defends": cast(
        "list[str]", [CardName.STRIKE] * 5 + [CardName.DEFEND] * 2 + [CardName.BASH]
    ),
    "cut-bash": cast("list[str]", [CardName.STRIKE] * 5 + [CardName.DEFEND] * 4),
    "add-strike": cast(
        "list[str]", [CardName.STRIKE] * 6 + [CardName.DEFEND] * 4 + [CardName.BASH]
    ),
    "add-iron-wave": cast(
        "list[str]",
        [CardName.STRIKE] * 5
        + [CardName.DEFEND] * 4
        + [CardName.BASH, CardName.IRON_WAVE],
    ),
    "add-inflame": cast(
        "list[str]",
        [CardName.STRIKE] * 5
        + [CardName.DEFEND] * 4
        + [CardName.BASH, CardName.INFLAME],
    ),
    "swap-defend-for-iron-wave": cast(
        "list[str]",
        [CardName.STRIKE] * 5
        + [CardName.DEFEND] * 3
        + [CardName.BASH, CardName.IRON_WAVE],
    ),
}


@dataclass
class BenchResult:
    label: str
    hp_outcomes: list[int] = field(default_factory=list)
    optimal_hp_outcomes: list[int] = field(default_factory=list)
    turn_outcomes: list[int] = field(default_factory=list)

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

    @property
    def mean_turns(self) -> float:
        return statistics.mean(self.turn_outcomes) if self.turn_outcomes else 0.0

    @property
    def mean_turns_won(self) -> float:
        """Average fight length (in turns) among fights the player won."""
        won = [t for t, hp in zip(self.turn_outcomes, self.hp_outcomes) if hp > 0]
        return statistics.mean(won) if won else 0.0

    @property
    def mean_turns_lost(self) -> float:
        """Average fight length (in turns) among fights the player lost."""
        lost = [t for t, hp in zip(self.turn_outcomes, self.hp_outcomes) if hp <= 0]
        return statistics.mean(lost) if lost else 0.0

    def __str__(self) -> str:
        line = (
            f"{self.label:<38}"
            f"  wins={self.wins}/{self.total}"
            f"  avg_lost={self.mean_hp_lost:5.1f} ±{self.stderr_hp_lost:.1f}"
            f"  p25/p50/p75={self.p25:.0f}/{self.p50:.0f}/{self.p75:.0f}"
        )
        if self.turn_outcomes:
            line += f"  avg_turns={self.mean_turns:4.1f}"
            if self.wins < self.total:
                line += (
                    f" (won={self.mean_turns_won:.1f}/lost={self.mean_turns_lost:.1f})"
                )
        if self.optimal_hp_outcomes:
            line += (
                f"  ceiling={self.mean_hp_lost_optimal:5.1f}  regret={self.regret:5.1f}"
            )
        return line


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


def _run_one_random(args: tuple) -> tuple[int, int]:
    """Random-policy fight worker — fast baseline.

    Returns (final player HP, turns taken)."""
    import random

    seed, deck, monster_name = args
    from .. import apply, is_terminal, legal_actions

    rng = random.Random(seed)
    state = _SCENARIOS[monster_name](seed=seed, deck=deck)
    while not is_terminal(state):
        state = apply(state, rng.choice(legal_actions(state)))
    return state.player_hp, state.turn


def run_deck(
    deck: list[str] | None,
    monster: Encounter | str = Encounter.JAW_WORM,
    scenario_fn=None,
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
        monster: Encounter key — see `Encounter` for the full list.
        scenario_fn: Optional callable ``(seed, deck) -> CombatState`` that
            overrides `monster`. Only supported with ``policy='mcts'``; lets
            callers (e.g. server.py) construct a scenario dynamically from a
            live monster list without registering a named `Encounter`.
        seeds: Number of independent seeds (fights) to run.
        iterations: MCTS iterations per decision (ignored when policy='random').
        workers: Parallel worker processes (ignored when policy='mcts', which
            runs all fights in one rayon-parallel, GIL-released Rust call).
        label: Human-readable name shown in reports.
        policy: 'mcts' (default) or 'random'.
        ceiling: Also compute the per-seed `optimal_value` ceiling (exact
            branch-and-bound search, no rollouts) and report it alongside
            the policy's average HP loss as `regret`. Off by default since
            it's a separate search per seed on top of the policy runs.
    """
    if scenario_fn is not None:
        if policy != "mcts":
            raise ValueError("scenario_fn is only supported with policy='mcts'")
        if ceiling:
            raise ValueError("scenario_fn is not supported with ceiling=True")
        the_scenario = scenario_fn
        encounter = None
    else:
        try:
            encounter = Encounter(monster)
        except ValueError:
            raise ValueError(
                f"unknown monster {monster!r}; choices: {', '.join(_SCENARIOS)}"
            ) from None
        the_scenario = _SCENARIOS[encounter]

    result = BenchResult(label=label or repr(deck))

    if policy == "mcts":
        from .. import fight_outcomes_per_fight

        states = [the_scenario(seed=seed, deck=deck) for seed in range(seeds)]
        outcomes = fight_outcomes_per_fight(states, iterations=iterations)
        result.hp_outcomes = [PLAYER_STARTING_HP - hp_lost for hp_lost, _ in outcomes]
        result.turn_outcomes = [turns for _, turns in outcomes]
    else:
        hp_by_seed: dict[int, int] = {}
        turns_by_seed: dict[int, int] = {}
        arg_list = [(seed, deck, encounter) for seed in range(seeds)]
        with ProcessPoolExecutor(max_workers=workers) as pool:
            policy_futures = {
                pool.submit(_run_one_random, args): args[0] for args in arg_list
            }
            for policy_fut in as_completed(policy_futures):
                seed = policy_futures[policy_fut]
                hp_by_seed[seed], turns_by_seed[seed] = policy_fut.result()
        result.hp_outcomes = [hp_by_seed[i] for i in range(seeds)]
        result.turn_outcomes = [turns_by_seed[i] for i in range(seeds)]

    if ceiling:
        optimal_hp_by_seed: dict[int, int] = {}
        with ProcessPoolExecutor(max_workers=workers) as pool:
            ceiling_futures = {
                pool.submit(_run_one_optimal, (seed, deck, encounter)): seed
                for seed in range(seeds)
            }
            for ceiling_fut in as_completed(ceiling_futures):
                optimal_hp_by_seed[ceiling_futures[ceiling_fut]] = ceiling_fut.result()
        result.optimal_hp_outcomes = [optimal_hp_by_seed[i] for i in range(seeds)]

    return result


def compare(
    configs: dict[str, list[str] | None],
    monster: Encounter | str = Encounter.JAW_WORM,
    seeds: int = 50,
    iterations: int = 200,
    workers: int = 4,
    policy: str = "mcts",
    ceiling: bool = False,
) -> list[BenchResult]:
    """Benchmark each named deck in `configs` and print a comparison table.

    Args:
        configs: Dict mapping label → deck (card list or None for default).
        monster: Encounter key — see `Encounter` for the full list.
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


def run_win_rate(runs: list, iterations: int = 200) -> float:
    """Win rate over a batch of seeded `RunState`s (HOL-59) — the run-level
    analog of `BenchResult.win_rate`, one granularity up from a single
    fight. Each run is played to completion with the default seeded
    random-legal policy via `simulate_run_outcomes`."""
    from .. import simulate_run_outcomes

    if not runs:
        return 0.0
    outcomes = simulate_run_outcomes(runs, iterations)
    return sum(1 for won, _, _ in outcomes if won) / len(outcomes)


def build_overgrowth_monster_only_run(seed: int, slots: int, deck: list | None = None):
    """A monsters-and-card-rewards-only run (HOL-59/HOL-61/HOL-64 composed,
    no elites/rest sites/skeleton-assembly): draw `slots` monster names from
    the Overgrowth weak/normal pools and look up each one's canonical
    starting HP."""
    from .. import RunState, draw_overgrowth_monster_sequence
    from ..scenarios import (
        IRONCLAD_STARTING_DECK,
        PLAYER_STARTING_HP,
    )

    path = draw_overgrowth_monster_sequence(seed=seed, slots=slots)
    return RunState(
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
        hp=PLAYER_STARTING_HP,
        path=path,
    )


def run_overgrowth_win_rate(
    seeds: int = 50, slots: int = 4, iterations: int = 200, deck: list | None = None
) -> float:
    """Win rate over `seeds` independently-drawn Overgrowth monsters-and-
    rewards runs — the run-level analog of `run_deck`'s convenience for a
    single scenario, built on `build_overgrowth_monster_only_run` and
    `run_win_rate`."""
    runs = [
        build_overgrowth_monster_only_run(seed, slots, deck) for seed in range(seeds)
    ]
    return run_win_rate(runs, iterations)


# The Overgrowth v1 run skeleton (HOL-65): a fixed, hand-picked node order
# exercising every node kind HOL-61/HOL-62/HOL-63 established at least once,
# ending in the Vantom boss (the only Overgrowth boss sts_sim implements).
# Not derived from any placement algorithm — counts/ordering are a judgment
# call, matching the issue's own example shape. "weak"/"normal" entries are
# placeholders filled in from the seeded pool draw; "elite" entries are
# filled in from the seeded elite draw; "rest" entries become RestSite
# nodes (monster_name/hp ignored for those).
_OVERGROWTH_SKELETON = [
    "weak",
    "weak",
    "weak",
    "normal",
    "normal",
    "elite",
    "normal",
    "normal",
    "rest",
    "elite",
    "rest",
    "boss",
]


def build_overgrowth_run(seed: int, deck: list | None = None, hp: int | None = None):
    """Assembles the fixed Overgrowth v1 skeleton into a `RunState` for
    `seed`: monster/elite selection still goes through the seeded pool
    draws (HOL-61/HOL-62) — only the *node-kind sequence itself* is fixed.
    None of HOL-61/62/63/64 needed to hand-edit anything for this; it's
    pure composition on top of their already-isolated mechanisms."""
    from .. import RunState, draw_overgrowth_elite, draw_overgrowth_monster_sequence
    from ..scenarios import (
        IRONCLAD_STARTING_DECK,
        PLAYER_STARTING_HP,
    )

    monster_slots = sum(
        1 for kind in _OVERGROWTH_SKELETON if kind in ("weak", "normal")
    )
    monster_names = iter(
        draw_overgrowth_monster_sequence(seed=seed, slots=monster_slots)
    )
    elite_names = iter(
        [
            draw_overgrowth_elite(seed=seed + i)
            for i in range(_OVERGROWTH_SKELETON.count("elite"))
        ]
    )

    path: list[str] = []
    elite_indices: list[int] = []
    rest_site_indices: list[int] = []
    for kind in _OVERGROWTH_SKELETON:
        if kind in ("weak", "normal"):
            name = next(monster_names)
            path.append(name)
        elif kind == "elite":
            elite_indices.append(len(path))
            name = next(elite_names)
            path.append(name)
        elif kind == "rest":
            rest_site_indices.append(len(path))
            path.append("RestSite")
        elif kind == "boss":
            path.append("Vantom")

    return RunState(
        seed=seed,
        deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
        hp=hp if hp is not None else PLAYER_STARTING_HP,
        path=path,
        elite_indices=elite_indices,
        rest_site_indices=rest_site_indices,
    )


def run_overgrowth_skeleton_win_rate(
    seeds: int = 50, iterations: int = 200, deck: list | None = None
) -> float:
    """Win rate over `seeds` full Overgrowth v1 skeleton runs (HOL-65) — the
    run-level analog of `run_deck`'s convenience, one level up from
    `run_overgrowth_win_rate`'s monsters-only slice."""
    runs = [build_overgrowth_run(seed, deck) for seed in range(seeds)]
    return run_win_rate(runs, iterations)


# Named run builders the reward-policy A/B can seed, keyed for the CLI/tests
# so the process-pool workers only ever ship a *string* (RunState itself is
# unpicklable — the same reason `_run_one_random` rebuilds from seed rather
# than shipping the CombatState).
_REWARD_RUN_BUILDERS: "dict[str, Callable[..., RunState]]" = {
    "overgrowth-monsters": build_overgrowth_monster_only_run,
    "overgrowth-skeleton": build_overgrowth_run,
}


def _run_one_reward_ab(args: tuple) -> tuple[int, int, int]:
    """Paired A/B worker for one seed. Rebuilds the run (RunState is
    unpicklable — pass the builder key + kwargs, not the object) and plays
    it twice through the *same* `run_apply` combat path: once random-legal,
    once with the lookahead reward picker. Because both arms resolve real
    combats via `run_apply` (fixed 200-iter budget, `RESOLVE_COMBAT_ITERATIONS`)
    and share the seed, combat strength is identical by construction and the
    only thing that varies between arms is the card-reward decision. Returns
    `(seed, random_final_hp, lookahead_final_hp)`."""
    import random

    from .. import run_apply, run_is_terminal, run_legal_actions
    from ..policies import simulate_run_with_reward_lookahead

    builder, kwargs, seed, num_sims, rollout_iterations = args
    build = _REWARD_RUN_BUILDERS[builder]

    rng = random.Random(seed)
    current = build(seed=seed, **kwargs)
    while not run_is_terminal(current):
        current = run_apply(current, rng.choice(run_legal_actions(current)))
    random_hp = current.hp

    _, lookahead_hp, _ = simulate_run_with_reward_lookahead(
        build(seed=seed, **kwargs), num_sims, rollout_iterations, seed
    )
    return seed, random_hp, lookahead_hp


def compare_reward_policies(
    builder: str = "overgrowth-monsters",
    *,
    seeds: int = 20,
    num_sims: int = 5,
    rollout_iterations: int = 30,
    workers: int = 4,
    builder_kwargs: dict | None = None,
) -> tuple[BenchResult, BenchResult]:
    """Run-level A/B: the lookahead card-reward picker
    (`policies.lookahead_reward_policy`) vs. the default random-legal reward
    choice, over `seeds` seeded runs of `builder` (one of `_REWARD_RUN_BUILDERS`).

    Both arms play through the same `run_apply` combat path (fixed 200-iter
    MCTS budget), so combat strength is identical by construction and the
    only variable is the reward decision. Each seed's two arms share that
    seed, so the comparison is *paired* (same monster/reward draws),
    matching this module's `compare_decks`/`evaluate_reward_options`
    convention — that pairing is what lets a modest `seeds` show signal.

    COMPUTE COST: each lookahead run resolves roughly
    `num_sims * offered_cards * remaining_nodes` embedded combats *per
    reward node* — seconds per run. Defaults are deliberately small; scale
    `seeds`/`num_sims` up together with `workers` (and patience). The random
    arm is cheap and rides along in the same worker.

    Returns `(random_result, lookahead_result)` as `BenchResult`s whose
    `hp_outcomes` hold per-seed final HP (0 = death), so `win_rate`,
    `mean_hp_lost`, and the percentiles all apply. `turn_outcomes` is left
    empty — run-level nodes aren't combat turns.
    """
    if builder not in _REWARD_RUN_BUILDERS:
        raise ValueError(
            f"unknown builder {builder!r}; choices: {', '.join(_REWARD_RUN_BUILDERS)}"
        )
    kwargs = dict(builder_kwargs or {})
    if builder == "overgrowth-monsters":
        kwargs.setdefault("slots", 4)

    random_hp: dict[int, int] = {}
    lookahead_hp: dict[int, int] = {}
    arg_list = [
        (builder, kwargs, seed, num_sims, rollout_iterations) for seed in range(seeds)
    ]
    with ProcessPoolExecutor(max_workers=workers) as pool:
        futures = [pool.submit(_run_one_reward_ab, args) for args in arg_list]
        for fut in as_completed(futures):
            seed, r_hp, l_hp = fut.result()
            random_hp[seed] = r_hp
            lookahead_hp[seed] = l_hp

    random_result = BenchResult(
        label="random reward", hp_outcomes=[random_hp[i] for i in range(seeds)]
    )
    lookahead_result = BenchResult(
        label="lookahead reward", hp_outcomes=[lookahead_hp[i] for i in range(seeds)]
    )
    print(random_result)
    print(lookahead_result)
    hp_delta = lookahead_result.mean_hp_lost - random_result.mean_hp_lost
    wr_delta = lookahead_result.win_rate - random_result.win_rate
    print(
        f"{'lookahead − random':<38}"
        f"  avg_lost {hp_delta:+5.1f} (negative ⇒ lookahead better)"
        f"  win_rate {wr_delta:+.2f}"
    )
    return random_result, lookahead_result


def compare_decks(
    decks: dict[str, list[str] | None],
    encounters: list[Encounter | str],
    seeds: int = 50,
    iterations: int = 200,
    policy: str = "mcts",
) -> dict[Encounter, list[BenchResult]]:
    """Compare named decks against each other across multiple encounters.

    Each deck is run with the same `seeds` against each encounter, so
    fights are paired (same monster RNG per seed across decks), keeping the
    per-encounter differential low-variance. Prints one line per encounter:
    each deck's avg HP lost, followed by its differential vs. the first deck
    in `decks`.

    Args:
        decks: Dict mapping label → deck (card list or None for default).
            The first entry is the baseline that differentials are computed
            against.
        encounters: Encounters to run each deck against, e.g. `EASY_POOL +
            [Encounter.BYRDONIS]`.
        seeds: Fights per deck per encounter.
        iterations: MCTS iterations per decision.
        policy: 'mcts' or 'random'.

    Returns:
        Dict mapping each encounter to the list of BenchResult (one per
        deck, in `decks` order).
    """
    results: dict[Encounter, list[BenchResult]] = {}
    for enc in encounters:
        encounter = Encounter(enc)
        row = [
            run_deck(
                deck,
                monster=encounter,
                seeds=seeds,
                iterations=iterations,
                policy=policy,
                label=label,
            )
            for label, deck in decks.items()
        ]
        results[encounter] = row

        baseline = row[0].mean_hp_lost
        cells = [f"{row[0].label}={baseline:5.1f}"]
        for r in row[1:]:
            diff = r.mean_hp_lost - baseline
            cells.append(f"{r.label}={r.mean_hp_lost:5.1f} (Δ{diff:+5.1f})")
        print(f"{encounter.value:<20} " + "  ".join(cells))

    return results
