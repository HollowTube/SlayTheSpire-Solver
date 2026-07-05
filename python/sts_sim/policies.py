"""Pluggable run-level policies, built entirely on RunState's public
interface (run_legal_actions/run_apply/run_is_terminal) — no Rust changes.
`simulate_run_outcome` hardcodes a uniform-random-legal policy; this module
is for callers who want different decision-making at specific node kinds
while still falling back to random-legal everywhere else.
"""

import random
import statistics

from . import RunState, run_apply, run_is_terminal, run_legal_actions


def _is_reward_decision(actions: list[str]) -> bool:
    return any(a.startswith("Take:") for a in actions) or actions == ["Skip"]


def _rollout_final_hp(state: RunState, rng: random.Random, iterations: int) -> int:
    """Plays `state` to completion with the uniform-random-legal policy
    (matching `simulate_run_outcome`'s own default), using `rng` for every
    choice along the way — including any further reward decisions, which
    are *not* re-evaluated by the lookahead (that would be recursively
    expensive); only the reward decision under evaluation gets the
    lookahead treatment, the rest of the rollout is a plain random walk."""
    current = state
    while not run_is_terminal(current):
        actions = run_legal_actions(current)
        action = rng.choice(actions)
        current = run_apply(current, action, iterations)
    return current.hp


def evaluate_reward_options(
    state: RunState, num_sims: int = 5, iterations: int = 30, seed: int = 0
) -> dict[str, float]:
    """For every action currently on offer at `state` (expected to be a
    pending card-reward decision — `Take:<name>` per offered card, plus
    `Skip`), apply it and run `num_sims` random-policy rollouts of the rest
    of the run from the resulting state, against whatever combat nodes
    actually come next on `state`'s own path. Returns each action's mean
    final HP across those rollouts — higher is better.

    Uses common random numbers across candidates: the same `num_sims`
    rollout seeds are reused for every action, so `Take:A` rollout *i* and
    `Take:B` rollout *i* face the same downstream reward/rest-site/combat
    randomness and differ only in the card under test — without this, the
    comparison is dominated by which candidate happened to get luckier
    rollouts rather than by the card itself, especially at small
    `num_sims` (mirrors `bench.compare_decks`'s existing paired-comparison
    convention, just applied to rollouts instead of seeds).

    `iterations` defaults low (30, not the usual 200) since these rollouts
    are already estimates under a random policy, not the real run's own
    combat resolution — the full MCTS budget buys little extra precision
    here for a large constant-factor slowdown.

    Cost note: this resolves `num_sims * len(actions) * (remaining nodes)`
    embedded combats per call — expensive at full-run scale, intentionally
    so it's most useful for evaluating a single reward decision, not for
    wrapping every decision in a full run blindly.
    """
    master_rng = random.Random(seed)
    rollout_seeds = [master_rng.randrange(2**63) for _ in range(num_sims)]
    scores: dict[str, float] = {}
    for action in run_legal_actions(state):
        after = run_apply(state, action)
        outcomes = [
            _rollout_final_hp(after, random.Random(rollout_seed), iterations)
            for rollout_seed in rollout_seeds
        ]
        scores[action] = statistics.mean(outcomes)
    return scores


def lookahead_reward_policy(
    state: RunState, num_sims: int = 5, iterations: int = 30, seed: int = 0
) -> str:
    """The action (`Take:<name>` or `Skip`) with the highest mean final HP
    across `num_sims` rollouts of the rest of the run — see
    `evaluate_reward_options`. `Skip` is evaluated identically to every
    `Take:<name>` option, so it wins whenever skipping really does save
    more HP on average; there's no special-casing in its favor."""
    scores = evaluate_reward_options(state, num_sims, iterations, seed)
    return max(scores, key=scores.__getitem__)


def simulate_run_with_reward_lookahead(
    state: RunState,
    num_sims: int = 5,
    rollout_iterations: int = 30,
    seed: int = 0,
) -> tuple[bool, int, int]:
    """Plays `state` to completion, using `lookahead_reward_policy` at every
    card-reward decision and uniform-random-legal everywhere else (combat
    nodes are forced regardless and always resolve at `run_apply`'s own
    fixed MCTS budget — `rollout_iterations` only ever controls the
    lookahead's internal rollouts, never the real run's own combats; rest-
    site choices aren't second-guessed here either). Returns `(won,
    final_hp, nodes_completed)`, matching `simulate_run_outcome`'s shape."""
    rng = random.Random(seed)
    current = state
    nodes_completed = 0
    while not run_is_terminal(current):
        actions = run_legal_actions(current)
        is_reward = _is_reward_decision(actions)
        if is_reward:
            action = lookahead_reward_policy(
                current, num_sims, rollout_iterations, rng.randrange(2**31)
            )
        else:
            action = rng.choice(actions)
        current = run_apply(current, action)
        # Reward decisions (Take:/Skip) resolve in place without advancing
        # the run's path position; every other action kind does.
        if not is_reward:
            nodes_completed += 1
    return (current.hp > 0, current.hp, nodes_completed)
