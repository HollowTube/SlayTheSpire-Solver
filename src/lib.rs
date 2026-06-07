use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rand::SeedableRng;
use rand_pcg::Pcg32;

#[derive(Clone, PartialEq)]
enum PendingDecision {
    SelectTarget { card: String },
}

impl PendingDecision {
    fn as_str(&self) -> &'static str {
        match self {
            PendingDecision::SelectTarget { .. } => "SelectTarget",
        }
    }
}

/// A single step in a card's declarative effect pipeline. A generic engine
/// interprets these against a `CombatState`, so that adding an ordinary card
/// means adding data to `card_data` rather than new engine logic.
#[derive(Clone, PartialEq)]
enum EffectOp {
    DealDamage(i32),
}

/// A card's energy cost and declarative effect pipeline (run once any
/// `RequestChoice` steps, e.g. `SelectTarget`, have been resolved into a
/// `PendingDecision`). Adding an ordinary card means adding an entry here,
/// not new engine logic.
// `effects` only covers post-target-selection ops (e.g. `DealDamage`) — the
// `RequestChoice(SelectTarget)` step is hardcoded into the generic `PlayCard:`
// handler below rather than expressed as pipeline data, since every card so
// far targets the lone monster. A non-targeted card (e.g. HOL-8's GainBlock)
// will need `RequestChoice` modeled as a real op so that the generic engine
// can decide whether to enter `SelectTarget` at all.
struct CardData {
    cost: i32,
    effects: Vec<EffectOp>,
}

fn card_data(name: &str) -> Option<CardData> {
    match name {
        "Strike" => Some(CardData {
            cost: 1,
            effects: vec![EffectOp::DealDamage(6)],
        }),
        _ => None,
    }
}

fn run_effect_ops(state: &mut CombatState, ops: &[EffectOp]) {
    for op in ops {
        match op {
            EffectOp::DealDamage(amount) => state.monster_hp -= amount,
        }
    }
}

#[pyclass(eq)]
#[derive(Clone, PartialEq)]
pub struct CombatState {
    #[pyo3(get)]
    player_hp: i32,
    #[pyo3(get)]
    player_energy: i32,
    #[pyo3(get)]
    monster_hp: i32,
    #[pyo3(get)]
    monster_attack: i32,
    #[pyo3(get)]
    turn: u32,
    #[pyo3(get)]
    hand: Vec<String>,
    pending: Option<PendingDecision>,
    rng: Pcg32,
}

#[pymethods]
impl CombatState {
    #[new]
    #[pyo3(signature = (player_hp, player_energy, monster_hp, monster_attack, seed, hand=Vec::new()))]
    fn new(
        player_hp: i32,
        player_energy: i32,
        monster_hp: i32,
        monster_attack: i32,
        seed: u64,
        hand: Vec<String>,
    ) -> Self {
        CombatState {
            player_hp,
            player_energy,
            monster_hp,
            monster_attack,
            turn: 0,
            hand,
            pending: None,
            rng: Pcg32::seed_from_u64(seed),
        }
    }

    #[getter]
    fn pending(&self) -> Option<String> {
        self.pending.as_ref().map(|p| p.as_str().to_string())
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

#[pyfunction]
fn legal_actions(state: &CombatState) -> Vec<String> {
    match state.pending {
        Some(PendingDecision::SelectTarget { .. }) => vec!["SelectTarget:Monster".to_string()],
        None => {
            let mut actions: Vec<String> = state
                .hand
                .iter()
                .map(|name| format!("PlayCard:{name}"))
                .collect();
            actions.push("EndTurn".to_string());
            actions
        }
    }
}

#[pyfunction]
fn apply(state: &CombatState, action: &str) -> PyResult<CombatState> {
    match action {
        "EndTurn" => {
            let mut next = state.clone();
            next.turn += 1;
            next.player_hp -= next.monster_attack;
            Ok(next)
        }
        "SelectTarget:Monster" => match &state.pending {
            Some(PendingDecision::SelectTarget { card }) => {
                let mut next = state.clone();
                let data = card_data(card).expect("pending card is always known");
                run_effect_ops(&mut next, &data.effects);
                next.pending = None;
                Ok(next)
            }
            None => Err(PyValueError::new_err(format!("unknown action: {action}"))),
        },
        other => match other.strip_prefix("PlayCard:") {
            Some(card_name) => {
                let data = card_data(card_name)
                    .ok_or_else(|| PyValueError::new_err(format!("unknown card: {card_name}")))?;
                let mut next = state.clone();
                let position = next
                    .hand
                    .iter()
                    .position(|c| c == card_name)
                    .ok_or_else(|| PyValueError::new_err(format!("{card_name} is not in hand")))?;
                next.hand.remove(position);
                next.player_energy -= data.cost;
                next.pending = Some(PendingDecision::SelectTarget {
                    card: card_name.to_string(),
                });
                Ok(next)
            }
            None => Err(PyValueError::new_err(format!("unknown action: {other}"))),
        },
    }
}

#[pyfunction]
fn is_terminal(state: &CombatState) -> bool {
    state.player_hp <= 0 || state.monster_hp <= 0
}

/// Heuristic value of a state from the player's perspective: the HP margin.
/// Positive favours the player, negative favours the monster.
#[pyfunction]
fn evaluate(state: &CombatState) -> f64 {
    (state.player_hp - state.monster_hp) as f64
}

/// Terminal reward signal: zero while combat is ongoing, otherwise the HP
/// margin (positive for a win, negative for a loss; magnitude reflects how
/// decisive the outcome was).
#[pyfunction]
fn reward(state: &CombatState) -> f64 {
    if is_terminal(state) {
        evaluate(state)
    } else {
        0.0
    }
}

#[pymodule]
fn _sts_sim(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<CombatState>()?;
    m.add_function(wrap_pyfunction!(legal_actions, m)?)?;
    m.add_function(wrap_pyfunction!(apply, m)?)?;
    m.add_function(wrap_pyfunction!(is_terminal, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate, m)?)?;
    m.add_function(wrap_pyfunction!(reward, m)?)?;
    Ok(())
}
