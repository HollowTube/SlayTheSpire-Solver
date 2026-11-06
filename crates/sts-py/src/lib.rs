use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use sts_ids::{CardId, MonsterId, StatusId};
use sts_sim::cards::{card_data, on_exhaust_effects};
use sts_sim::engine::{EffectOp, Status};
use sts_sim::mcts::DEFAULT_DETERMINIZATIONS;
use sts_sim::run::{self, RunState};
use sts_sim::state::{CardInstance, CombatState, Monster, PendingDecision};
use sts_sim::{SimAction, SimError};
use std::collections::HashMap;

mod action;
use action::{EndTurnAction, ExhaustCardFromHandAction, PlayCardAction, SelectTargetAction, UpgradeCardFromHandAction};

fn value_error(e: SimError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

#[pyclass(name = "Monster", eq)]
#[derive(Clone, PartialEq)]
struct PyMonster(Monster);

#[pymethods]
impl PyMonster {
    #[new]
    #[pyo3(signature = (hp, attack=0, max_hp=None, name=None, block=0, statuses=Vec::new(), intent=None, last_move=None, move_streak=0, moves_used=Vec::new()))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        hp: i32,
        attack: i32,
        max_hp: Option<i32>,
        name: Option<String>,
        block: i32,
        statuses: Vec<(String, i32)>,
        intent: Option<String>,
        last_move: Option<String>,
        move_streak: u32,
        moves_used: Vec<String>,
    ) -> Self {
        PyMonster(Monster::new(
            hp, attack, max_hp, name, block, statuses, intent, last_move, move_streak, moves_used,
        ))
    }

    #[getter]
    fn attack(&self) -> i32 {
        self.0.attack
    }

    #[getter]
    fn name(&self) -> Option<String> {
        self.0.name.map(|id| id.as_str().to_string())
    }

    #[getter]
    fn hp(&self) -> i32 {
        self.0.fighter.hp
    }

    #[getter]
    fn max_hp(&self) -> i32 {
        self.0.fighter.max_hp
    }

    #[getter]
    fn block(&self) -> i32 {
        self.0.fighter.block
    }

    #[getter]
    fn statuses(&self) -> Vec<String> {
        self.0.fighter.statuses.iter().map(|s| s.as_str().to_string()).collect()
    }

    #[getter]
    fn strength(&self) -> i32 {
        self.0.strength()
    }

    #[getter]
    fn intent(&self) -> Option<String> {
        self.0.intent.clone()
    }

    #[getter]
    fn last_move(&self) -> Option<String> {
        self.0.last_move.clone()
    }

    #[getter]
    fn is_alive(&self) -> bool {
        self.0.fighter.hp > 0
    }
}

#[pyclass(name = "CombatState", eq)]
#[derive(Clone, PartialEq)]
struct PyCombatState(CombatState);

fn pile(cards: &[CardInstance]) -> Vec<String> {
    cards.iter().map(|c| c.as_str()).collect()
}

#[pymethods]
impl PyCombatState {
    #[new]
    #[pyo3(signature = (player_hp, player_energy, monsters, seed, hand=Vec::new(), deck=None, player_max_hp=None, player_max_energy=None, player_block=0, player_statuses=Vec::new(), turn=0, draw_pile=Vec::new(), discard_pile=Vec::new(), exhaust_pile=Vec::new(), attacks_played_this_turn=0))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        player_hp: i32,
        player_energy: i32,
        monsters: Vec<PyMonster>,
        seed: u64,
        hand: Vec<String>,
        deck: Option<Vec<String>>,
        player_max_hp: Option<i32>,
        player_max_energy: Option<i32>,
        player_block: i32,
        player_statuses: Vec<(String, i32)>,
        turn: u32,
        draw_pile: Vec<String>,
        discard_pile: Vec<String>,
        exhaust_pile: Vec<String>,
        attacks_played_this_turn: i32,
    ) -> Self {
        PyCombatState(CombatState::new(
            player_hp,
            player_energy,
            monsters.into_iter().map(|m| m.0).collect(),
            seed,
            hand,
            deck,
            player_max_hp,
            player_max_energy,
            player_block,
            player_statuses,
            turn,
            draw_pile,
            discard_pile,
            exhaust_pile,
            attacks_played_this_turn,
        ))
    }

    #[getter]
    fn player_energy(&self) -> i32 {
        self.0.energy()
    }

    #[getter]
    fn monsters(&self) -> Vec<PyMonster> {
        self.0.monsters().iter().cloned().map(PyMonster).collect()
    }

    #[getter]
    fn turn(&self) -> u32 {
        self.0.turn()
    }

    #[getter]
    fn player_hp(&self) -> i32 {
        self.0.player().hp
    }

    #[getter]
    fn player_max_hp(&self) -> i32 {
        self.0.player().max_hp
    }

    #[getter]
    fn player_block(&self) -> i32 {
        self.0.player().block
    }

    #[getter]
    fn hand(&self) -> Vec<String> {
        pile(self.0.hand())
    }

    #[getter]
    fn draw_pile(&self) -> Vec<String> {
        pile(self.0.draw_pile())
    }

    #[getter]
    fn discard_pile(&self) -> Vec<String> {
        pile(self.0.discard_pile())
    }

    #[getter]
    fn exhaust_pile(&self) -> Vec<String> {
        pile(self.0.exhaust_pile())
    }

    #[getter]
    fn pending(&self) -> Option<String> {
        self.0.pending().map(|p| p.as_str().to_string())
    }

    #[getter]
    fn player_statuses(&self) -> Vec<String> {
        self.0.player().statuses.iter().map(|s| s.as_str().to_string()).collect()
    }

    #[getter]
    fn player_strength(&self) -> i32 {
        self.0
            .player()
            .statuses
            .iter()
            .filter_map(|s| match s {
                Status::Strength(n) => Some(*n),
                _ => None,
            })
            .sum()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    /// Return a copy of this state with the embedded PRNG re-seeded from
    /// `seed`. Used by MCTS to give each rollout a fresh draw sequence.
    fn with_rng_seed(&self, seed: u64) -> Self {
        PyCombatState(self.0.reseeded(seed))
    }
}

#[pyclass(name = "RunState")]
#[derive(Clone)]
struct PyRunState(RunState);

#[pymethods]
impl PyRunState {
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
        PyRunState(RunState::new(seed, deck, hp, path, max_hp, &elite_indices, &rest_site_indices))
    }

    #[getter]
    fn hp(&self) -> i32 {
        self.0.hp()
    }

    #[getter]
    fn deck(&self) -> Vec<String> {
        self.0.deck()
    }
}

fn to_py_action(py: Python<'_>, action: &SimAction) -> PyObject {
    let name = |card: &CardId, upgrades: &u8| {
        CardInstance { id: *card, upgrade_level: *upgrades }.as_str()
    };
    match action {
        SimAction::EndTurn => EndTurnAction::new().into_pyobject(py).unwrap().into_any().unbind(),
        SimAction::PlayCard { card, upgrades } => {
            PlayCardAction::new(name(card, upgrades)).into_pyobject(py).unwrap().into_any().unbind()
        }
        SimAction::SelectTarget(i) => SelectTargetAction::new(*i).into_pyobject(py).unwrap().into_any().unbind(),
        SimAction::UpgradeFromHand { card, upgrades } => UpgradeCardFromHandAction::new(name(card, upgrades))
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind(),
        SimAction::ExhaustFromHand { card, upgrades } => ExhaustCardFromHandAction::new(name(card, upgrades))
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind(),
    }
}

#[pyfunction]
fn legal_actions(py: Python<'_>, state: &PyCombatState) -> Vec<PyObject> {
    sts_sim::legal_actions(&state.0).iter().map(|a| to_py_action(py, a)).collect()
}

/// An action object as the card name it carries, before the engine sees it.
enum RawAction {
    EndTurn,
    PlayCard(String),
    SelectTarget(usize),
    UpgradeFromHand(String),
    ExhaustFromHand(String),
}

/// The card `raw` names, if it is the exact canonical spelling (`Strike+`).
/// Anything else (a raw STS2 id, an unknown name) can never match a card in
/// hand, because piles are compared by canonical name.
fn canonical(raw: &str) -> Option<CardInstance> {
    CardInstance::try_parse(raw).filter(|c| c.as_str() == raw)
}

fn to_sim_action(state: &CombatState, raw: RawAction) -> Result<SimAction, SimError> {
    let pending = state.pending();
    match raw {
        RawAction::EndTurn => Ok(SimAction::EndTurn),
        RawAction::SelectTarget(i) => Ok(SimAction::SelectTarget(i)),
        RawAction::PlayCard(name) => {
            if matches!(
                pending,
                Some(PendingDecision::UpgradeFromHand | PendingDecision::ExhaustFromHand { .. })
            ) {
                return Err(SimError::HandChoicePending { action: format!("PlayCard:{name}") });
            }
            match canonical(&name) {
                Some(c) => Ok(SimAction::PlayCard { card: c.id, upgrades: c.upgrade_level }),
                None => {
                    // Parses to the same card or panics, exactly as before.
                    CardInstance::parse(&name);
                    Err(SimError::NotInHand { card: name })
                }
            }
        }
        RawAction::UpgradeFromHand(name) => match canonical(&name) {
            Some(c) => Ok(SimAction::UpgradeFromHand { card: c.id, upgrades: c.upgrade_level }),
            None if matches!(pending, Some(PendingDecision::UpgradeFromHand)) => {
                Err(SimError::NoUpgradable { card: name })
            }
            None => Err(SimError::UnknownAction { action: format!("UpgradeCardFromHand:{name}") }),
        },
        RawAction::ExhaustFromHand(name) => match canonical(&name) {
            Some(c) => Ok(SimAction::ExhaustFromHand { card: c.id, upgrades: c.upgrade_level }),
            None if matches!(pending, Some(PendingDecision::ExhaustFromHand { .. })) => {
                Err(SimError::NotInHand { card: name })
            }
            None => Err(SimError::UnknownAction { action: format!("ExhaustCardFromHand:{name}") }),
        },
    }
}

/// Accepts typed Action objects only.
#[pyfunction]
fn apply(state: &PyCombatState, action: &Bound<'_, PyAny>) -> PyResult<PyCombatState> {
    let raw = if action.is_instance_of::<EndTurnAction>() {
        RawAction::EndTurn
    } else if let Ok(a) = action.extract::<PyRef<PlayCardAction>>() {
        RawAction::PlayCard(a.card.clone())
    } else if let Ok(a) = action.extract::<PyRef<SelectTargetAction>>() {
        RawAction::SelectTarget(a.monster_index)
    } else if let Ok(a) = action.extract::<PyRef<UpgradeCardFromHandAction>>() {
        RawAction::UpgradeFromHand(a.card.clone())
    } else if let Ok(a) = action.extract::<PyRef<ExhaustCardFromHandAction>>() {
        RawAction::ExhaustFromHand(a.card.clone())
    } else {
        return Err(PyTypeError::new_err(
            "action must be a PlayCardAction, SelectTargetAction, EndTurnAction, UpgradeCardFromHandAction, or ExhaustCardFromHandAction",
        ));
    };
    let sim_action = to_sim_action(&state.0, raw).map_err(value_error)?;
    sts_sim::apply(&state.0, &sim_action).map(PyCombatState).map_err(value_error)
}

#[pyfunction]
fn is_terminal(state: &PyCombatState) -> bool {
    sts_sim::is_terminal(&state.0)
}

#[pyfunction]
fn evaluate(state: &PyCombatState) -> f64 {
    sts_sim::evaluate(&state.0)
}

#[pyfunction]
fn reward(state: &PyCombatState) -> f64 {
    sts_sim::reward(&state.0)
}

#[pyfunction]
fn redeterminized(state: &PyCombatState, seed: u64) -> PyCombatState {
    PyCombatState(state.0.redeterminized(seed))
}

#[pyfunction]
fn random_rollout(state: &PyCombatState, seed: u64) -> f64 {
    sts_sim::random_rollout(&state.0, seed)
}

#[pyfunction]
fn optimal_value(state: &PyCombatState) -> f64 {
    sts_sim::optimal_value(&state.0)
}

#[pyfunction]
fn all_card_names() -> Vec<&'static str> {
    sts_sim::all_card_names()
}

#[pyfunction]
fn all_monster_names() -> Vec<&'static str> {
    sts_sim::all_monster_names()
}

#[pyfunction]
#[pyo3(signature = (state, iterations=200, seed=0, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS))]
fn mcts_action_values(
    state: &PyCombatState,
    iterations: u32,
    seed: u64,
    determinize: bool,
    determinizations: u32,
) -> HashMap<String, f64> {
    sts_sim::mcts::action_values(&state.0, iterations, seed, determinize, determinizations)
}

#[pyfunction]
#[pyo3(signature = (state, iterations=200, seed=0, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS))]
fn mcts_search(
    state: &PyCombatState,
    iterations: u32,
    seed: u64,
    determinize: bool,
    determinizations: u32,
) -> String {
    sts_sim::mcts::search(&state.0, iterations, seed, determinize, determinizations)
}

#[pyfunction]
#[pyo3(signature = (state, iterations=200, seed=0, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS, max_actions=1000))]
fn simulate_hp_lost(
    state: &PyCombatState,
    iterations: u32,
    seed: u64,
    determinize: bool,
    determinizations: u32,
    max_actions: u32,
) -> i32 {
    sts_sim::mcts::simulate_hp_lost(&state.0, iterations, seed, determinize, determinizations, max_actions)
}

fn inner_states(states: Vec<PyCombatState>) -> Vec<CombatState> {
    states.into_iter().map(|s| s.0).collect()
}

/// Per-fight HP lost, run in parallel with the GIL released.
#[pyfunction]
#[pyo3(signature = (states, iterations=100, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS, max_actions=1000))]
fn hp_lost_per_fight(
    py: Python<'_>,
    states: Vec<PyCombatState>,
    iterations: u32,
    determinize: bool,
    determinizations: u32,
    max_actions: u32,
) -> Vec<i32> {
    let states = inner_states(states);
    py.allow_threads(|| {
        sts_sim::mcts::hp_lost_per_fight(&states, iterations, determinize, determinizations, max_actions)
    })
}

/// `(hp lost, turns taken)` per fight, run in parallel with the GIL released.
#[pyfunction]
#[pyo3(signature = (states, iterations=100, determinize=true, determinizations=DEFAULT_DETERMINIZATIONS, max_actions=1000))]
fn fight_outcomes_per_fight(
    py: Python<'_>,
    states: Vec<PyCombatState>,
    iterations: u32,
    determinize: bool,
    determinizations: u32,
    max_actions: u32,
) -> Vec<(i32, u32)> {
    let states = inner_states(states);
    py.allow_threads(|| {
        sts_sim::mcts::fight_outcomes_per_fight(&states, iterations, determinize, determinizations, max_actions)
    })
}

#[pyfunction]
fn run_legal_actions(state: &PyRunState) -> Vec<String> {
    run::run_legal_actions(&state.0)
}

#[pyfunction]
fn run_is_terminal(state: &PyRunState) -> bool {
    run::run_is_terminal(&state.0)
}

#[pyfunction]
#[pyo3(signature = (state, action, iterations=200))]
fn run_apply(state: &PyRunState, action: &str, iterations: u32) -> PyResult<PyRunState> {
    run::run_apply(&state.0, action, iterations).map(PyRunState).map_err(value_error)
}

#[pyfunction]
#[pyo3(signature = (state, iterations=200, seed=0))]
fn simulate_run_outcome(state: &PyRunState, iterations: u32, seed: u64) -> PyResult<(bool, i32, u32)> {
    run::simulate_run_outcome(&state.0, iterations, seed).map_err(value_error)
}

#[pyfunction]
fn simulate_run_outcomes(py: Python<'_>, runs: Vec<PyRunState>, iterations: u32) -> PyResult<Vec<(bool, i32, u32)>> {
    let runs: Vec<RunState> = runs.into_iter().map(|r| r.0).collect();
    py.allow_threads(|| run::simulate_run_outcomes(&runs, iterations)).map_err(value_error)
}

#[pyfunction]
fn draw_overgrowth_monster_sequence(seed: u64, slots: usize) -> Vec<String> {
    sts_sim::act::draw_overgrowth_monster_sequence(seed, slots)
}

#[pyfunction]
fn draw_overgrowth_elite(seed: u64) -> String {
    sts_sim::act::draw_overgrowth_elite(seed)
}

/// Registry card rows, in declaration order, for `sts_sim.sim._registry`.
#[pyfunction(name = "cards")]
fn card_rows(py: Python<'_>) -> PyResult<Vec<Bound<'_, PyDict>>> {
    CardId::all()
        .iter()
        .map(|&id| {
            let row = PyDict::new(py);
            row.set_item("variant", id.variant())?;
            row.set_item("display", id.as_str())?;
            row.set_item("sts2_id", id.sts2_id())?;
            row.set_item("bridge_names", id.bridge_names())?;
            row.set_item("in_card_pool", id.in_card_pool())?;
            Ok(row)
        })
        .collect()
}

fn status_dict<'py>(py: Python<'py>, op: &str, status: &Status) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("op", op)?;
    d.set_item("status", status.as_str())?;
    d.set_item("amount", status.amount())?;
    Ok(d)
}

fn effect_dict<'py>(py: Python<'py>, effect: &EffectOp) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    let op = match effect {
        EffectOp::ApplyStatusToTarget(s) => return status_dict(py, "ApplyStatusToTarget", s),
        EffectOp::ApplyStatusToSelf(s) => return status_dict(py, "ApplyStatusToSelf", s),
        EffectOp::EscalateSelfStatus(s) => return status_dict(py, "EscalateSelfStatus", s),
        EffectOp::DealDamage(n) => { d.set_item("amount", n)?; "DealDamage" }
        EffectOp::GainBlock(n) => { d.set_item("amount", n)?; "GainBlock" }
        EffectOp::DrawCards(n) => { d.set_item("amount", n)?; "DrawCards" }
        EffectOp::LoseHp(n) => { d.set_item("amount", n)?; "LoseHp" }
        EffectOp::GainEnergy(n) => { d.set_item("amount", n)?; "GainEnergy" }
        EffectOp::Heal(n) => { d.set_item("amount", n)?; "Heal" }
        EffectOp::DealDamageToAllEnemies(n) => { d.set_item("amount", n)?; "DealDamageToAllEnemies" }
        EffectOp::DealDamageToRandomEnemy(n) => { d.set_item("amount", n)?; "DealDamageToRandomEnemy" }
        EffectOp::DealDamageToLastAttacker(n) => { d.set_item("amount", n)?; "DealDamageToLastAttacker" }
        EffectOp::ApplyCardToTarget(c) => { d.set_item("card", c.as_str())?; "ApplyCardToTarget" }
        EffectOp::AddCardToDiscard(c) => { d.set_item("card", c.as_str())?; "AddCardToDiscard" }
        EffectOp::AddUpgradedCardToDiscard(c) => { d.set_item("card", c.as_str())?; "AddUpgradedCardToDiscard" }
        EffectOp::AddCardToHand(c, n) => {
            d.set_item("card", c.as_str())?;
            d.set_item("amount", n)?;
            "AddCardToHand"
        }
        EffectOp::AddRandomCardToHand(pool) => {
            d.set_item("pool", pool.iter().map(|c| c.as_str()).collect::<Vec<_>>())?;
            "AddRandomCardToHand"
        }
        EffectOp::ExhaustRandomFromHand(f) => { d.set_item("filter", f.as_str())?; "ExhaustRandomFromHand" }
        EffectOp::ReturnRandomDiscardToHand(f) => { d.set_item("filter", f.as_str())?; "ReturnRandomDiscardToHand" }
        EffectOp::ExhaustAllFromHand { filter, gain_block_per_card } => {
            d.set_item("filter", filter.as_str())?;
            d.set_item("gain_block_per_card", gain_block_per_card)?;
            "ExhaustAllFromHand"
        }
        EffectOp::DealDamageScaled { base, per_unit, source } => {
            d.set_item("base", base)?;
            d.set_item("per_unit", per_unit)?;
            d.set_item("source", source.as_str())?;
            "DealDamageScaled"
        }
        EffectOp::GainBlockScaled { base, per_unit, source } => {
            d.set_item("base", base)?;
            d.set_item("per_unit", per_unit)?;
            d.set_item("source", source.as_str())?;
            "GainBlockScaled"
        }
        EffectOp::GainEnergyScaled { base, per_unit, source } => {
            d.set_item("base", base)?;
            d.set_item("per_unit", per_unit)?;
            d.set_item("source", source.as_str())?;
            "GainEnergyScaled"
        }
        EffectOp::DealDamageRepeated { amount, hits_base, hits_per_unit, hits_source } => {
            d.set_item("amount", amount)?;
            d.set_item("hits_base", hits_base)?;
            d.set_item("hits_per_unit", hits_per_unit)?;
            d.set_item("source", hits_source.as_str())?;
            "DealDamageRepeated"
        }
        EffectOp::PlayTopOfDeck { count, exhaust } => {
            d.set_item("amount", count)?;
            d.set_item("exhaust", exhaust)?;
            "PlayTopOfDeck"
        }
        EffectOp::PlayTopOfDeckScaled { count_base, count_per_unit, count_source, exhaust } => {
            d.set_item("base", count_base)?;
            d.set_item("per_unit", count_per_unit)?;
            d.set_item("source", count_source.as_str())?;
            d.set_item("exhaust", exhaust)?;
            "PlayTopOfDeckScaled"
        }
        EffectOp::SpawnMonster(name, hp) => {
            d.set_item("monster", name)?;
            d.set_item("amount", hp)?;
            "SpawnMonster"
        }
        EffectOp::GainMaxHpIfLastAttackKilled(n) => { d.set_item("amount", n)?; "GainMaxHpIfLastAttackKilled" }
        EffectOp::DealDamageWithCombatBonus { base, bonus_key, increment } => {
            d.set_item("base", base)?;
            d.set_item("bonus_key", bonus_key)?;
            d.set_item("increment", increment)?;
            "DealDamageWithCombatBonus"
        }
        EffectOp::PromptExhaustCardFromHand { strength } => {
            d.set_item("strength", strength)?;
            "PromptExhaustCardFromHand"
        }
        EffectOp::PromptUpgradeCardFromHand => "PromptUpgradeCardFromHand",
        EffectOp::UpgradeAllCardsInHand => "UpgradeAllCardsInHand",
        EffectOp::PutRandomDiscardOnTopOfDraw => "PutRandomDiscardOnTopOfDraw",
        EffectOp::DoubleVulnerableOnTarget => "DoubleVulnerableOnTarget",
        EffectOp::GainStrengthEqualToTargetVulnerable => "GainStrengthEqualToTargetVulnerable",
    };
    d.set_item("op", op)?;
    Ok(d)
}

/// A card's sim data as a dict: `card_spec("Bash")`, `card_spec("Bash+")`.
#[pyfunction]
fn card_spec<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyDict>> {
    let card = CardInstance::try_parse(name)
        .ok_or_else(|| PyValueError::new_err(format!("unknown card: {name}")))?;
    let data = card_data(card.id, card.upgrade_level);
    let mut keywords: Vec<&str> = data.keywords.iter().map(|k| k.as_str()).collect();
    keywords.sort_unstable();
    let effects = data
        .effects
        .iter()
        .map(|e| effect_dict(py, e))
        .collect::<PyResult<Vec<_>>>()?;
    let on_exhaust = on_exhaust_effects(card.id, card.upgrade_level)
        .iter()
        .map(|e| effect_dict(py, e))
        .collect::<PyResult<Vec<_>>>()?;
    let d = PyDict::new(py);
    d.set_item("name", card.as_str())?;
    d.set_item("sts2_id", card.id.sts2_id())?;
    d.set_item("cost", data.cost)?;
    d.set_item("targeted", data.targeted)?;
    d.set_item("card_type", data.card_type.as_str())?;
    d.set_item("rarity", data.rarity.as_str())?;
    d.set_item("keywords", keywords)?;
    d.set_item("description", data.description)?;
    d.set_item("effects", effects)?;
    d.set_item("on_exhaust", on_exhaust)?;
    Ok(d)
}

/// Registry monster rows, in declaration order, for `sts_sim.sim._registry`.
#[pyfunction(name = "monsters")]
fn monster_rows(py: Python<'_>) -> PyResult<Vec<Bound<'_, PyDict>>> {
    MonsterId::all()
        .iter()
        .map(|&id| {
            let row = PyDict::new(py);
            row.set_item("variant", id.variant())?;
            row.set_item("display", id.as_str())?;
            row.set_item("sts2_id", id.sts2_id())?;
            row.set_item("bridge_names", id.bridge_names().to_vec())?;
            row.set_item("hp", id.hp())?;
            row.set_item("encounter", id.encounter())?;
            Ok(row)
        })
        .collect()
}

/// Registry status rows, in declaration order, for `sts_sim.sim._registry`.
#[pyfunction(name = "statuses")]
fn status_rows(py: Python<'_>) -> PyResult<Vec<Bound<'_, PyDict>>> {
    StatusId::all()
        .iter()
        .map(|&id| {
            let row = PyDict::new(py);
            row.set_item("variant", id.variant())?;
            row.set_item("display", id.as_str())?;
            row.set_item("bridge_names", id.bridge_names().to_vec())?;
            Ok(row)
        })
        .collect()
}


#[pymodule]
fn _sts_sim(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<EndTurnAction>()?;
    m.add_class::<PlayCardAction>()?;
    m.add_class::<SelectTargetAction>()?;
    m.add_class::<UpgradeCardFromHandAction>()?;
    m.add_class::<ExhaustCardFromHandAction>()?;
    m.add_class::<PyCombatState>()?;
    m.add_class::<PyMonster>()?;
    m.add_class::<PyRunState>()?;
    m.add_function(wrap_pyfunction!(run_legal_actions, m)?)?;
    m.add_function(wrap_pyfunction!(run_apply, m)?)?;
    m.add_function(wrap_pyfunction!(run_is_terminal, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_run_outcome, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_run_outcomes, m)?)?;
    m.add_function(wrap_pyfunction!(draw_overgrowth_monster_sequence, m)?)?;
    m.add_function(wrap_pyfunction!(draw_overgrowth_elite, m)?)?;
    m.add_function(wrap_pyfunction!(legal_actions, m)?)?;
    m.add_function(wrap_pyfunction!(apply, m)?)?;
    m.add_function(wrap_pyfunction!(is_terminal, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate, m)?)?;
    m.add_function(wrap_pyfunction!(reward, m)?)?;
    m.add_function(wrap_pyfunction!(random_rollout, m)?)?;
    m.add_function(wrap_pyfunction!(redeterminized, m)?)?;
    m.add_function(wrap_pyfunction!(all_card_names, m)?)?;
    m.add_function(wrap_pyfunction!(all_monster_names, m)?)?;
    m.add_function(wrap_pyfunction!(card_rows, m)?)?;
    m.add_function(wrap_pyfunction!(card_spec, m)?)?;
    m.add_function(wrap_pyfunction!(monster_rows, m)?)?;
    m.add_function(wrap_pyfunction!(status_rows, m)?)?;
    m.add_function(wrap_pyfunction!(optimal_value, m)?)?;
    m.add_function(wrap_pyfunction!(mcts_action_values, m)?)?;
    m.add_function(wrap_pyfunction!(mcts_search, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_hp_lost, m)?)?;
    m.add_function(wrap_pyfunction!(hp_lost_per_fight, m)?)?;
    m.add_function(wrap_pyfunction!(fight_outcomes_per_fight, m)?)?;
    Ok(())
}
