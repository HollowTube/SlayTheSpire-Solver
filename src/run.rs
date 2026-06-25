//! `RunState`: a multi-fight Slay the Spire run (HOL-59). Mirrors
//! `CombatState`'s `legal_actions`/`apply` shape so the same kind of code
//! that drives a single fight can drive a sequence of them. Combat nodes are
//! resolved opaquely — `run_apply`'s "ResolveCombat" action constructs a
//! fresh `CombatState` from the run's persistent deck/HP, plays it to
//! completion via the existing MCTS engine, and folds the outcome back in.
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

/// One stop on a run's path. Only combat nodes exist for now (HOL-59) — rest
/// sites and card rewards are separate follow-up issues (HOL-63/HOL-64) that
/// add variants here without changing `RunState`'s core loop.
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
    if state.position >= state.path.len() || state.hp <= 0 {
        return Vec::new();
    }
    match &state.path[state.position] {
        NodeKind::Combat { .. } => vec!["ResolveCombat".to_string()],
    }
}

#[pyfunction]
pub(crate) fn run_is_terminal(state: &RunState) -> bool {
    state.hp <= 0 || state.position >= state.path.len()
}

/// Shared implementation behind `run_apply` and `simulate_run_outcome`, with
/// the embedded combat's MCTS iteration count as an explicit parameter
/// rather than always reaching for the same constant — plays the current
/// combat node to completion via the existing MCTS engine (opaque
/// resolution — individual card plays never surface at the `RunState`
/// level) and folds the outcome back into a new `RunState`: HP carries over
/// (clamped at 0 on a loss, never negative), the persistent deck is
/// unchanged (no rewards exist yet — HOL-64), and `position` advances by
/// one node.
fn resolve(state: &RunState, action: &str, iterations: u32) -> PyResult<RunState> {
    if action != "ResolveCombat" {
        return Err(PyValueError::new_err(format!("unknown run action: {action}")));
    }
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
    Ok(next)
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
