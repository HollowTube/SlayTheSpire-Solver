//! `RunState`: a multi-fight Slay the Spire run (HOL-59). Mirrors
//! `CombatState`'s `legal_actions`/`apply` shape so the same kind of code
//! that drives a single fight can drive a sequence of them. Combat nodes are
//! resolved opaquely — `run_apply`'s "ResolveCombat" action constructs a
//! fresh `CombatState` from the run's persistent deck/HP, plays it to
//! completion via the existing MCTS engine, and folds the outcome back in.
//! A won combat immediately offers a card-reward decision (HOL-64) before
//! the run advances to its next node.
use crate::cards::{card_data, CardType, ALL_CARD_NAMES};
use crate::mcts::simulate_fight_outcome;
use crate::state::{CardInstance, CombatState, Monster};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_pcg::Pcg32;
use rayon::prelude::*;

/// MCTS iterations per combat-node resolution. A fixed default for now —
/// matches `simulate_hp_lost`'s own default, plenty for the weak early
/// encounters this issue's fixed path uses.
const RESOLVE_COMBAT_ITERATIONS: u32 = 200;

/// How many distinct cards a reward decision offers, before `Skip`.
const REWARD_CHOICE_COUNT: usize = 3;

/// One stop on a run's path. Only combat nodes exist for now (HOL-59) — rest
/// sites are a separate follow-up issue (HOL-63) that adds a variant here
/// without changing `RunState`'s core loop.
#[derive(Clone, PartialEq)]
pub(crate) enum NodeKind {
    Combat { monster_name: String, monster_hp: i32 },
}

#[pyclass]
#[derive(Clone)]
pub(crate) struct RunState {
    pub(crate) seed: u64,
    pub(crate) path: Vec<NodeKind>,
    pub(crate) position: usize,
    pub(crate) deck: Vec<CardInstance>,
    pub(crate) hp: i32,
    pub(crate) max_hp: i32,
    /// Cards currently on offer from a just-won combat node, or empty when
    /// no reward decision is pending. A non-empty value takes priority over
    /// whatever node `position` points at — the run doesn't advance past a
    /// won combat until this is resolved via `Take:<name>` or `Skip`.
    pub(crate) pending_reward: Vec<String>,
}

#[pymethods]
impl RunState {
    #[new]
    #[pyo3(signature = (seed, deck, hp, path, max_hp=None))]
    fn new(seed: u64, deck: Vec<String>, hp: i32, path: Vec<(String, i32)>, max_hp: Option<i32>) -> Self {
        RunState {
            seed,
            path: path
                .into_iter()
                .map(|(monster_name, monster_hp)| NodeKind::Combat { monster_name, monster_hp })
                .collect(),
            position: 0,
            deck: deck.iter().map(|s| CardInstance::parse(s)).collect(),
            hp,
            max_hp: max_hp.unwrap_or(hp),
            pending_reward: Vec::new(),
        }
    }

    #[getter]
    fn hp(&self) -> i32 {
        self.hp
    }

    #[getter]
    fn deck(&self) -> Vec<String> {
        self.deck.iter().map(CardInstance::as_str).collect()
    }
}

#[pyfunction]
pub(crate) fn run_legal_actions(state: &RunState) -> Vec<String> {
    if !state.pending_reward.is_empty() {
        let mut actions: Vec<String> = state.pending_reward.iter().map(|name| format!("Take:{name}")).collect();
        actions.push("Skip".to_string());
        return actions;
    }
    if state.position >= state.path.len() || state.hp <= 0 {
        return Vec::new();
    }
    match &state.path[state.position] {
        NodeKind::Combat { .. } => vec!["ResolveCombat".to_string()],
    }
}

#[pyfunction]
pub(crate) fn run_is_terminal(state: &RunState) -> bool {
    state.pending_reward.is_empty() && (state.hp <= 0 || state.position >= state.path.len())
}

/// Draws `REWARD_CHOICE_COUNT` distinct card names, seeded, from every card
/// `sts_sim` models except `CardType::Status` (monster-inflicted junk cards
/// like Dazed/Wound/Slimed/Infection — never legitimate rewards in the real
/// game either). No rarity weighting (flat/uniform) per HOL-64's scope;
/// HOL-60's `CardRarity` field is for a separate future follow-up.
fn draw_reward_cards(seed: u64) -> Vec<String> {
    let pool: Vec<&str> = ALL_CARD_NAMES
        .iter()
        .copied()
        .filter(|name| card_data(name, 0).map(|data| data.card_type != CardType::Status).unwrap_or(false))
        .collect();
    let mut rng = Pcg32::seed_from_u64(seed);
    pool.choose_multiple(&mut rng, REWARD_CHOICE_COUNT.min(pool.len()))
        .map(|s| s.to_string())
        .collect()
}

/// Resolves a combat node's "ResolveCombat" action: plays the current node
/// to completion via the existing MCTS engine (opaque — individual card
/// plays never surface at the `RunState` level), folds HP back in (clamped
/// at 0 on a loss, never negative), and advances `position`. A win also
/// populates `pending_reward` with a fresh card-reward offer; a loss does
/// not (the run is terminal either way once `position` is exhausted or HP
/// hits 0 — see `run_is_terminal`).
fn resolve_combat(state: &RunState, iterations: u32) -> PyResult<RunState> {
    let NodeKind::Combat { monster_name, monster_hp } = state
        .path
        .get(state.position)
        .ok_or_else(|| PyValueError::new_err("ResolveCombat at a terminal run"))?;

    let monster = Monster::new(
        *monster_hp,
        0,
        None,
        Some(monster_name.clone()),
        0,
        Vec::new(),
        None,
        None,
        0,
        Vec::new(),
    );
    let combat_seed = state.seed.wrapping_add(state.position as u64);
    let combat = CombatState::new(
        state.hp,
        3,
        vec![monster],
        combat_seed,
        Vec::new(),
        Some(state.deck.iter().map(CardInstance::as_str).collect()),
        Some(state.max_hp),
        None,
        0,
        Vec::new(),
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let (hp_lost, _turns) = simulate_fight_outcome(&combat, iterations, combat_seed, true, 8, 1000);

    let mut next = state.clone();
    next.hp = (state.hp - hp_lost).max(0);
    next.position += 1;
    if next.hp > 0 {
        // Distinct from `combat_seed` (which drives the embedded MCTS) so
        // the reward draw isn't correlated with how the fight itself played
        // out.
        let reward_seed = combat_seed.wrapping_add(0x5EED_0FFE_5EED_0FFE);
        next.pending_reward = draw_reward_cards(reward_seed);
    }
    Ok(next)
}

/// Resolves a pending card-reward decision: `Take:<name>` appends an
/// unupgraded copy of `name` to the persistent deck; `Skip` changes
/// nothing. Either way, clears `pending_reward` so the run moves on to
/// whatever node `position` now points at.
fn resolve_reward(state: &RunState, action: &str) -> PyResult<RunState> {
    let mut next = state.clone();
    if action == "Skip" {
        next.pending_reward.clear();
        return Ok(next);
    }
    let Some(chosen) = action.strip_prefix("Take:") else {
        return Err(PyValueError::new_err(format!("unknown run action: {action}")));
    };
    if !state.pending_reward.iter().any(|name| name == chosen) {
        return Err(PyValueError::new_err(format!("{chosen} is not on offer")));
    }
    next.deck.push(CardInstance::new(chosen));
    next.pending_reward.clear();
    Ok(next)
}

fn resolve(state: &RunState, action: &str, iterations: u32) -> PyResult<RunState> {
    if !state.pending_reward.is_empty() {
        resolve_reward(state, action)
    } else if action == "ResolveCombat" {
        resolve_combat(state, iterations)
    } else {
        Err(PyValueError::new_err(format!("unknown run action: {action}")))
    }
}

#[pyfunction]
pub(crate) fn run_apply(state: &RunState, action: &str) -> PyResult<RunState> {
    resolve(state, action, RESOLVE_COMBAT_ITERATIONS)
}

/// Drives `run` to completion with the default run-level policy — uniform
/// random choice among `run_legal_actions` at every decision, seeded so the
/// whole run (path traversal and every embedded combat's MCTS) is
/// reproducible from one seed. Today every combat node has exactly one
/// legal action, so this is a forced walk; the random-choice seam exists so
/// future node kinds (rest sites, card rewards — HOL-63/HOL-64) with real
/// choices need no changes here. Returns `(won, final_hp, nodes_completed)`.
#[pyfunction]
#[pyo3(signature = (state, iterations=200, seed=0))]
pub(crate) fn simulate_run_outcome(state: &RunState, iterations: u32, seed: u64) -> PyResult<(bool, i32, u32)> {
    let mut rng = Pcg32::seed_from_u64(seed);
    let mut current = state.clone();
    loop {
        let actions = run_legal_actions(&current);
        let Some(action) = actions.choose(&mut rng) else {
            break;
        };
        current = resolve(&current, action, iterations)?;
    }
    Ok((current.hp > 0, current.hp, current.position as u32))
}

/// Runs `simulate_run_outcome` once per `runs` entry (in parallel, off the
/// GIL, mirroring `fight_outcomes_per_fight`'s pattern for single fights) —
/// the batch primitive behind a "what's my win rate over N seeded runs"
/// aggregator. Each entry's own index is its run-policy seed, so the batch
/// itself is reproducible independent of `runs`' construction order.
#[pyfunction]
pub(crate) fn simulate_run_outcomes(
    py: Python<'_>,
    runs: Vec<RunState>,
    iterations: u32,
) -> PyResult<Vec<(bool, i32, u32)>> {
    py.allow_threads(|| {
        runs.par_iter()
            .enumerate()
            .map(|(i, run)| simulate_run_outcome(run, iterations, i as u64))
            .collect()
    })
}
