//! Rust port of `python/sts_sim/mcts.py`'s tree search and PIMC
//! (redeterminization) ensemble. Same algorithm, same UCB1 constant, same
//! `DEFAULT_DETERMINIZATIONS` — kept here so it can run end-to-end (an entire
//! fight, many fights) without per-node Python/Rust FFI crossings, for things
//! like "how does adding this card to my deck change my average HP lost over
//! the next N fights" run live at a card-reward screen.
use crate::state::CombatState;
use crate::{apply, is_terminal, legal_actions_str, random_rollout, reward};
use pyo3::prelude::*;
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;
use rayon::prelude::*;
use std::collections::HashMap;

const DEFAULT_DETERMINIZATIONS: u32 = 8;

/// One decision point in the search tree. Stored in a flat `Vec<Node>` arena
/// and referenced by index rather than `Rc<RefCell<_>>`, since the tree never
/// outlives a single `build_tree` call and indices sidestep Rust's aliasing
/// rules for the select/expand/backprop mutation pattern.
struct Node {
    state: CombatState,
    parent: Option<usize>,
    action: Option<String>,
    children: Vec<usize>,
    untried_actions: Vec<String>,
    visits: u32,
    total_value: f64,
}

impl Node {
    fn new(state: CombatState, parent: Option<usize>, action: Option<String>) -> Self {
        let untried_actions = if is_terminal(&state) {
            Vec::new()
        } else {
            legal_actions_str(&state)
        };
        Node {
            state,
            parent,
            action,
            children: Vec::new(),
            untried_actions,
            visits: 0,
            total_value: 0.0,
        }
    }
}

fn ucb1(node: &Node, parent_visits: u32) -> f64 {
    let exploitation = node.total_value / node.visits as f64;
    let exploration =
        std::f64::consts::SQRT_2 * ((parent_visits as f64).ln() / node.visits as f64).sqrt();
    exploitation + exploration
}

/// Run `iterations` of select -> expand -> simulate -> backpropagate from
/// `state` and return the resulting arena (root at index 0).
fn build_tree(state: &CombatState, iterations: u32, rng: &mut Pcg32) -> Vec<Node> {
    let mut arena = vec![Node::new(state.clone(), None, None)];
    for _ in 0..iterations {
        let mut idx = 0usize;
        while arena[idx].untried_actions.is_empty() && !arena[idx].children.is_empty() {
            let parent_visits = arena[idx].visits;
            idx = *arena[idx]
                .children
                .iter()
                .max_by(|&&a, &&b| {
                    ucb1(&arena[a], parent_visits)
                        .partial_cmp(&ucb1(&arena[b], parent_visits))
                        .unwrap()
                })
                .unwrap();
        }
        if !arena[idx].untried_actions.is_empty() {
            let pos = rng.gen_range(0..arena[idx].untried_actions.len());
            let action = arena[idx].untried_actions.remove(pos);
            let next_state = apply(&arena[idx].state, &action).expect("legal action is always valid");
            let child = Node::new(next_state, Some(idx), Some(action));
            let child_idx = arena.len();
            arena.push(child);
            arena[idx].children.push(child_idx);
            idx = child_idx;
        }
        let value = if is_terminal(&arena[idx].state) {
            reward(&arena[idx].state)
        } else {
            random_rollout(&arena[idx].state, rng.gen())
        };
        let mut cur = Some(idx);
        while let Some(i) = cur {
            arena[i].visits += 1;
            arena[i].total_value += value;
            cur = arena[i].parent;
        }
    }
    arena
}

fn single_tree_action_values(state: &CombatState, iterations: u32, rng: &mut Pcg32) -> HashMap<String, f64> {
    let arena = build_tree(state, iterations, rng);
    arena[0]
        .children
        .iter()
        .map(|&i| (arena[i].action.clone().unwrap(), arena[i].total_value / arena[i].visits as f64))
        .collect()
}

/// See `mcts.action_values` in `python/sts_sim/mcts.py` for the full
/// `determinize`/`determinizations` semantics: `determinize=true` (the
/// default) averages `determinizations` independent trees, each built
/// against a `redeterminized` copy of `state` (PIMC); `determinize=false`
/// solves the single deterministic tree implied by `state` as given.
fn action_values_impl(
    state: &CombatState,
    iterations: u32,
    rng: &mut Pcg32,
    determinize: bool,
    determinizations: u32,
) -> HashMap<String, f64> {
    if !determinize {
        return single_tree_action_values(state, iterations, rng);
    }
    let mut totals: HashMap<String, f64> = HashMap::new();
    let mut counts: HashMap<String, u32> = HashMap::new();
    for _ in 0..determinizations {
        let seed: u64 = rng.gen();
        let sample = state.redeterminized(seed);
        for (action, value) in single_tree_action_values(&sample, iterations, rng) {
            *totals.entry(action.clone()).or_insert(0.0) += value;
            *counts.entry(action).or_insert(0) += 1;
        }
    }
    totals
        .into_iter()
        .map(|(action, total)| {
            let count = counts[&action];
            (action, total / count as f64)
        })
        .collect()
}

/// See `mcts.search`: `determinize=false` returns the most-visited root
/// child of a single tree; `determinize=true` (the default) returns the
/// argmax of the PIMC-averaged `action_values_impl`.
fn search_impl(state: &CombatState, iterations: u32, rng: &mut Pcg32, determinize: bool, determinizations: u32) -> String {
    if !determinize {
        let arena = build_tree(state, iterations, rng);
        let best = arena[0]
            .children
            .iter()
            .max_by_key(|&&i| arena[i].visits)
            .expect("root has children after iterations > 0");
        return arena[*best].action.clone().unwrap();
    }
    let values = action_values_impl(state, iterations, rng, true, determinizations);
    values
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .expect("at least one legal action")
        .0
}

#[pyfunction]
#[pyo3(signature = (state, iterations=200, seed=0, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS))]
pub(crate) fn mcts_action_values(
    state: &CombatState,
    iterations: u32,
    seed: u64,
    determinize: bool,
    determinizations: u32,
) -> HashMap<String, f64> {
    let mut rng = Pcg32::seed_from_u64(seed);
    action_values_impl(state, iterations, &mut rng, determinize, determinizations)
}

#[pyfunction]
#[pyo3(signature = (state, iterations=200, seed=0, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS))]
pub(crate) fn mcts_search(
    state: &CombatState,
    iterations: u32,
    seed: u64,
    determinize: bool,
    determinizations: u32,
) -> String {
    let mut rng = Pcg32::seed_from_u64(seed);
    search_impl(state, iterations, &mut rng, determinize, determinizations)
}

/// Play `state` to terminal using `mcts_search` at every decision and return
/// the player's HP lost (clamped at 0, i.e. a loss never reads as "negative
/// HP lost"). `max_actions` bounds the number of decisions taken as a
/// safety net against decks that can stalemate (e.g. out-blocking a
/// monster's chip damage forever) — if reached, returns the HP lost so far.
///
/// Note for callers comparing decks: hitting `max_actions` makes a
/// perpetually-stalling deck look *better* than one that wins outright (HP
/// lost so far is near 0), even though it never actually beats the monster.
/// Not an issue for any deck currently reachable in this codebase, but a
/// deck-comparison tool should treat a `max_actions` cutoff as "no result"
/// rather than "0 HP lost" if non-terminating decks ever become possible.
/// Shared implementation behind `simulate_hp_lost` and
/// `fight_outcomes_per_fight`: plays `state` to terminal via `search_impl`
/// at every decision and returns `(hp lost, turns taken)`. See
/// `simulate_hp_lost` for the `max_actions` stalemate caveat.
pub(crate) fn simulate_fight_outcome(
    state: &CombatState,
    iterations: u32,
    seed: u64,
    determinize: bool,
    determinizations: u32,
    max_actions: u32,
) -> (i32, u32) {
    let mut s = state.clone();
    let mut rng = Pcg32::seed_from_u64(seed);
    let starting_hp = s.player.hp;
    for _ in 0..max_actions {
        if is_terminal(&s) {
            break;
        }
        let action = search_impl(&s, iterations, &mut rng, determinize, determinizations);
        s = apply(&s, &action).expect("legal action is always valid");
    }
    (starting_hp - s.player.hp.max(0), s.turn)
}

#[pyfunction]
#[pyo3(signature = (state, iterations=200, seed=0, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS, max_actions=1000))]
pub(crate) fn simulate_hp_lost(
    state: &CombatState,
    iterations: u32,
    seed: u64,
    determinize: bool,
    determinizations: u32,
    max_actions: u32,
) -> i32 {
    simulate_fight_outcome(state, iterations, seed, determinize, determinizations, max_actions).0
}

/// Run `simulate_hp_lost` once per `states` entry (in parallel, off the GIL)
/// and return the per-fight HP lost — the raw data behind "given this deck,
/// what's my average HP loss over these N fights" for a card-pick
/// comparison. Each entry of `states` is expected to be an
/// independently-seeded fight (e.g. the same deck/monster matchup
/// constructed with different `seed`s), so the index alone is reused as
/// `simulate_hp_lost`'s internal search seed — two decks compared with the
/// same seeds share common random numbers, so per-index differences are
/// directly comparable (paired) and have much lower variance than the raw
/// means.
#[pyfunction]
#[pyo3(signature = (states, iterations=100, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS, max_actions=1000))]
pub(crate) fn hp_lost_per_fight(
    py: Python<'_>,
    states: Vec<CombatState>,
    iterations: u32,
    determinize: bool,
    determinizations: u32,
    max_actions: u32,
) -> Vec<i32> {
    py.allow_threads(|| {
        states
            .par_iter()
            .enumerate()
            .map(|(i, state)| {
                simulate_hp_lost(state, iterations, i as u64, determinize, determinizations, max_actions)
            })
            .collect()
    })
}

/// Like `hp_lost_per_fight`, but also returns each fight's turn count —
/// `(hp lost, turns taken)` per `states` entry. This is the entry point
/// `sts_sim.bench.run_deck`'s `policy="mcts"` path uses: it runs all `seeds`
/// fights in one rayon-parallel, GIL-released call instead of spawning a
/// worker process per fight.
#[pyfunction]
#[pyo3(signature = (states, iterations=100, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS, max_actions=1000))]
pub(crate) fn fight_outcomes_per_fight(
    py: Python<'_>,
    states: Vec<CombatState>,
    iterations: u32,
    determinize: bool,
    determinizations: u32,
    max_actions: u32,
) -> Vec<(i32, u32)> {
    py.allow_threads(|| {
        states
            .par_iter()
            .enumerate()
            .map(|(i, state)| {
                simulate_fight_outcome(state, iterations, i as u64, determinize, determinizations, max_actions)
            })
            .collect()
    })
}
