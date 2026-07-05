//! `RunState`: a multi-fight Slay the Spire run (HOL-59). Mirrors
//! `CombatState`'s `legal_actions`/`apply` shape so the same kind of code
//! that drives a single fight can drive a sequence of them. Combat nodes are
//! resolved opaquely — `run_apply`'s "ResolveCombat" action constructs a
//! fresh `CombatState` from the run's persistent deck/HP, plays it to
//! completion via the existing MCTS engine, and folds the outcome back in.
//! A won combat immediately offers a card-reward decision (HOL-64) before
//! the run advances to its next node.
use crate::cards::{card_data, CardType, ALL_CARD_NAMES};
use crate::encounters;
use crate::mcts::simulate_fight_outcome;
use crate::state::{CardInstance, CombatState};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_pcg::Pcg32;
use rayon::prelude::*;

/// How many distinct cards a reward decision offers, before `Skip`.
const REWARD_CHOICE_COUNT: usize = 3;

/// Fraction of max HP a Rest Site's Heal option restores — matches the real
/// game's rest-site heal percentage (30%, rounded down).
const REST_SITE_HEAL_FRACTION: f64 = 0.3;

/// One stop on a run's path. `Combat` and `Elite` are resolved identically
/// (opaque embedded `CombatState`, same MCTS-driven resolution) — the
/// distinction exists only to record provenance (which pool a monster came
/// from), since HOL-65's skeleton assembly needs to know where it placed
/// elites. `RestSite` is a real player decision (Heal vs. Upgrade) resolved
/// directly against `RunState`, with no embedded `CombatState` involved.
///
/// HOL-72: `Combat`/`Elite` now store an encounter name (looked up via
/// `encounters::encounter_def` at resolution time) rather than a raw
/// (name, hp) pair.
#[derive(Clone, PartialEq)]
pub(crate) enum NodeKind {
    Combat { encounter_name: String },
    Elite { encounter_name: String },
    RestSite,
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
    #[pyo3(signature = (seed, deck, hp, path, max_hp=None, elite_indices=Vec::new(), rest_site_indices=Vec::new()))]
    fn new(
        seed: u64,
        deck: Vec<String>,
        hp: i32,
        path: Vec<String>,
        max_hp: Option<i32>,
        elite_indices: Vec<usize>,
        rest_site_indices: Vec<usize>,
    ) -> Self {
        RunState {
            seed,
            path: path
                .into_iter()
                .enumerate()
                .map(|(i, encounter_name)| {
                    if rest_site_indices.contains(&i) {
                        NodeKind::RestSite
                    } else if elite_indices.contains(&i) {
                        NodeKind::Elite { encounter_name }
                    } else {
                        NodeKind::Combat { encounter_name }
                    }
                })
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
        NodeKind::Combat { .. } | NodeKind::Elite { .. } => vec!["ResolveCombat".to_string()],
        NodeKind::RestSite => {
            let mut actions = vec!["Heal".to_string()];
            for (i, card) in state.deck.iter().enumerate() {
                if card.upgrade_level == 0 {
                    actions.push(format!("Upgrade:{i}"));
                }
            }
            actions
        }
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
    let encounter_name = match state.path.get(state.position) {
        Some(NodeKind::Combat { encounter_name }) | Some(NodeKind::Elite { encounter_name }) => encounter_name,
        _ => return Err(PyValueError::new_err("ResolveCombat at a terminal or non-combat node")),
    };

    let shape = encounters::encounter_def(encounter_name)
        .ok_or_else(|| PyValueError::new_err(format!("unknown encounter: {encounter_name}")))?;
    let combat_seed = state.seed.wrapping_add(state.position as u64);
    let mut rng = Pcg32::seed_from_u64(combat_seed);
    let monsters = encounters::resolve_shape(&shape, &mut rng);

    let combat = CombatState::new(
        state.hp,
        3,
        monsters,
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

/// Resolves a Rest Site's `Heal`/`Upgrade:<index>` decision: `Heal` restores
/// `REST_SITE_HEAL_FRACTION` of max HP (capped at max HP, never overheals);
/// `Upgrade:<index>` increments that card's `upgrade_level` in the
/// persistent deck (refused if it's already upgraded — `run_legal_actions`
/// never offers it, but `apply` validates directly rather than trusting the
/// caller). Either way, advances `position` — no combat, no reward.
fn resolve_rest_site(state: &RunState, action: &str) -> PyResult<RunState> {
    let mut next = state.clone();
    if action == "Heal" {
        let healed = (state.max_hp as f64 * REST_SITE_HEAL_FRACTION).floor() as i32;
        next.hp = (state.hp + healed).min(state.max_hp);
    } else if let Some(index_str) = action.strip_prefix("Upgrade:") {
        let index: usize = index_str
            .parse()
            .map_err(|_| PyValueError::new_err(format!("invalid upgrade index: {action}")))?;
        let card = next
            .deck
            .get_mut(index)
            .ok_or_else(|| PyValueError::new_err(format!("no card at index {index}")))?;
        if card.upgrade_level >= 1 {
            return Err(PyValueError::new_err(format!("card at index {index} is already upgraded")));
        }
        card.upgrade_level += 1;
    } else {
        return Err(PyValueError::new_err(format!("unknown run action: {action}")));
    }
    next.position += 1;
    Ok(next)
}

fn resolve(state: &RunState, action: &str, iterations: u32) -> PyResult<RunState> {
    if !state.pending_reward.is_empty() {
        return resolve_reward(state, action);
    }
    match state.path.get(state.position) {
        Some(NodeKind::RestSite) => resolve_rest_site(state, action),
        Some(NodeKind::Combat { .. }) | Some(NodeKind::Elite { .. }) => {
            if action == "ResolveCombat" {
                resolve_combat(state, iterations)
            } else {
                Err(PyValueError::new_err(format!("unknown run action: {action}")))
            }
        }
        None => Err(PyValueError::new_err(format!("{action} at a terminal run"))),
    }
}

#[pyfunction]
#[pyo3(signature = (state, action, iterations=200))]
pub(crate) fn run_apply(state: &RunState, action: &str, iterations: u32) -> PyResult<RunState> {
    resolve(state, action, iterations)
}

/// Drives `run` to completion with the default run-level policy — uniform
/// random choice among `run_legal_actions` at every decision, seeded so the
/// whole run (path traversal and every embedded combat's MCTS) is
/// reproducible from one seed. A combat node has exactly one legal action
/// (forced), but reward (HOL-64) and Rest Site (HOL-63) decisions offer
/// several — this is where that choice actually gets made, with no changes
/// needed here as further node kinds gain their own real choices. Returns
/// `(won, final_hp, nodes_completed)`.
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
