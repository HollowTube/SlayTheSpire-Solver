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

/// A status that can be attached to a combatant. Each status declares which
/// event-bus events it listens to and what modifier it contributes (see
/// `Status::modifier_for`) — the generic damage pipeline applies whatever
/// modifiers are currently registered without knowing what `Vulnerable` (or
/// any future status, e.g. Strength) actually is.
#[derive(Clone, PartialEq)]
enum Status {
    Vulnerable,
}

impl Status {
    fn as_str(&self) -> &'static str {
        match self {
            Status::Vulnerable => "Vulnerable",
        }
    }
}

/// Event-bus event types that statuses (and other future listeners) can hook
/// into. Only `OnDamageDealt` has a listener so far; the rest of the documented
/// set (`OnCardPlayed`, `OnTurnStart`, ...) will be added as content needs them.
#[derive(Clone, Copy, PartialEq)]
enum EventType {
    OnDamageDealt,
}

/// A contribution to a calculation pipeline from a listener. The pipeline
/// (e.g. `apply_damage_modifiers`) folds these generically, so a new status
/// that multiplies or adds to damage needs no change to the pipeline itself.
#[derive(Clone, PartialEq)]
enum Modifier {
    MultiplyDamage(f64),
}

impl Status {
    /// What this status contributes when `event` fires, if anything — the
    /// "listener registration" half of the event-bus skeleton.
    fn modifier_for(&self, event: EventType) -> Option<Modifier> {
        match (self, event) {
            // Per the Slay the Spire wiki, Vulnerable increases damage taken
            // from attacks by 50%, rounded down.
            (Status::Vulnerable, EventType::OnDamageDealt) => Some(Modifier::MultiplyDamage(1.5)),
        }
    }
}

/// Runs `base` through every modifier that `statuses` (the *target's*
/// statuses) register for `OnDamageDealt`, folding them generically.
//
// This data-only path covers a second *multiplicative, target-side* status
// (e.g. Weak: reuse `Modifier::MultiplyDamage` and add a `modifier_for` entry
// — no change here). It does NOT yet cover Strength: that's *additive* (needs
// a new `Modifier::AddDamage` arm in this fold) and *attacker-side* (this fn
// only ever sees the target's statuses — there's no `player_statuses` field
// or attacker-side call site). Both are deferred until such a status exists.
fn apply_damage_modifiers(base: i32, statuses: &[Status], event: EventType) -> i32 {
    let modified = statuses
        .iter()
        .filter_map(|status| status.modifier_for(event))
        .fold(base as f64, |amount, modifier| match modifier {
            Modifier::MultiplyDamage(factor) => amount * factor,
        });
    modified.floor() as i32
}

/// A single step in a card's declarative effect pipeline. A generic engine
/// interprets these against a `CombatState`, so that adding an ordinary card
/// means adding data to `card_data` rather than new engine logic.
#[derive(Clone, PartialEq)]
enum EffectOp {
    DealDamage(i32),
    GainBlock(i32),
    ApplyStatus(Status),
}

/// A card's energy cost and declarative effect pipeline (run once any
/// `RequestChoice` steps, e.g. `SelectTarget`, have been resolved into a
/// `PendingDecision`). Adding an ordinary card means adding an entry here,
/// not new engine logic.
// `targeted` tells the generic `PlayCard:` handler whether to enter
// `SelectTarget` before running `effects` — e.g. `Strike` needs a target to
// deal damage to, while `Defend` resolves immediately against the player.
struct CardData {
    cost: i32,
    targeted: bool,
    effects: Vec<EffectOp>,
}

fn card_data(name: &str) -> Option<CardData> {
    match name {
        "Strike" => Some(CardData {
            cost: 1,
            targeted: true,
            effects: vec![EffectOp::DealDamage(6)],
        }),
        "Defend" => Some(CardData {
            cost: 1,
            targeted: false,
            effects: vec![EffectOp::GainBlock(5)],
        }),
        "Bash" => Some(CardData {
            cost: 2,
            targeted: true,
            effects: vec![
                EffectOp::DealDamage(8),
                EffectOp::ApplyStatus(Status::Vulnerable),
            ],
        }),
        _ => None,
    }
}

fn run_effect_ops(state: &mut CombatState, ops: &[EffectOp]) {
    for op in ops {
        match op {
            EffectOp::DealDamage(amount) => {
                let modified =
                    apply_damage_modifiers(*amount, &state.monster_statuses, EventType::OnDamageDealt);
                state.monster_hp -= modified;
            }
            EffectOp::GainBlock(amount) => state.player_block += amount,
            EffectOp::ApplyStatus(status) => state.monster_statuses.push(status.clone()),
        }
    }
}

#[pyclass(eq)]
#[derive(Clone, PartialEq)]
pub struct CombatState {
    #[pyo3(get)]
    player_hp: i32,
    // The HP fraction `evaluate`/`reward` shape against — what an STS run
    // actually optimizes for, since (unlike monster HP) player HP carries
    // between fights. Defaults to the starting `player_hp` (combat begins at
    // full); an explicit override exists so states that begin already short
    // of full HP — the common case once multi-fight runs exist — can be
    // constructed. No PyO3 getter: nothing outside the shaping formula reads it.
    player_max_hp: i32,
    #[pyo3(get)]
    player_energy: i32,
    #[pyo3(get)]
    player_block: i32,
    #[pyo3(get)]
    monster_hp: i32,
    // Always equal to the encounter's starting `monster_hp` in real play
    // (monsters spawn at full HP each fight, no carry-over) — the override
    // exists only so tests can construct an already-terminal state directly
    // without making max==current==0 collapse the fraction to NaN.
    monster_max_hp: i32,
    #[pyo3(get)]
    monster_attack: i32,
    monster_statuses: Vec<Status>,
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
    #[pyo3(signature = (player_hp, player_energy, monster_hp, monster_attack, seed, hand=Vec::new(), player_max_hp=None, monster_max_hp=None))]
    fn new(
        player_hp: i32,
        player_energy: i32,
        monster_hp: i32,
        monster_attack: i32,
        seed: u64,
        hand: Vec<String>,
        player_max_hp: Option<i32>,
        monster_max_hp: Option<i32>,
    ) -> Self {
        CombatState {
            player_hp,
            player_max_hp: player_max_hp.unwrap_or(player_hp),
            player_energy,
            player_block: 0,
            monster_hp,
            monster_max_hp: monster_max_hp.unwrap_or(monster_hp),
            monster_attack,
            monster_statuses: Vec::new(),
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

    #[getter]
    fn monster_statuses(&self) -> Vec<String> {
        self.monster_statuses.iter().map(|s| s.as_str().to_string()).collect()
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
            let absorbed = next.monster_attack.min(next.player_block);
            next.player_hp -= next.monster_attack - absorbed;
            // Block does not carry over between turns.
            next.player_block = 0;
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
                if data.targeted {
                    next.pending = Some(PendingDecision::SelectTarget {
                        card: card_name.to_string(),
                    });
                } else {
                    run_effect_ops(&mut next, &data.effects);
                }
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

/// Heuristic value of a state from the player's perspective: each side's
/// remaining HP as a fraction of its max (clamped at zero — overkill damage
/// doesn't make a win "more lost"), player's fraction minus monster's.
///
/// This is deliberately the terminal `reward` formula generalized to every
/// state, not just terminal ones (`evaluate` must be "computable from any
/// reachable state" per HOL-9's AC, since MCTS needs to score states
/// mid-search). The two coincide exactly at the boundary: in a win the
/// monster's clamped fraction is 0, leaving `+player_fraction`; in a loss the
/// player's clamped fraction is 0, leaving `-monster_fraction` — i.e.
/// `(win ? +1 : -1) * hp_fraction` of the side that's still standing, which is
/// what an actual STS run optimizes for (player HP carries between fights) and
/// what an eventual RL value head learns to predict.
#[pyfunction]
fn evaluate(state: &CombatState) -> f64 {
    let player_fraction = state.player_hp.max(0) as f64 / state.player_max_hp as f64;
    let monster_fraction = state.monster_hp.max(0) as f64 / state.monster_max_hp as f64;
    player_fraction - monster_fraction
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
