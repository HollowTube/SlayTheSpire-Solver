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
/// modifiers are currently registered without knowing what `Vulnerable` or
/// `Strength` actually is.
#[derive(Clone, PartialEq)]
enum Status {
    Vulnerable,
    // Carries its stack count — unlike Vulnerable (a binary on/off debuff),
    // Strength's contribution scales with how many stacks are held.
    Strength(i32),
}

impl Status {
    fn as_str(&self) -> &'static str {
        match self {
            Status::Vulnerable => "Vulnerable",
            Status::Strength(_) => "Strength",
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
    AddDamage(i32),
}

impl Status {
    /// What this status contributes when `event` fires, if anything — the
    /// "listener registration" half of the event-bus skeleton.
    fn modifier_for(&self, event: EventType) -> Option<Modifier> {
        match (self, event) {
            // Per the Slay the Spire wiki, Vulnerable increases damage taken
            // from attacks by 50%, rounded down.
            (Status::Vulnerable, EventType::OnDamageDealt) => Some(Modifier::MultiplyDamage(1.5)),
            // Per the Slay the Spire wiki, each stack of Strength adds its
            // count directly to attack damage dealt.
            (Status::Strength(amount), EventType::OnDamageDealt) => {
                Some(Modifier::AddDamage(*amount))
            }
        }
    }
}

/// Runs `base` through every modifier that `attacker_statuses` and
/// `target_statuses` register for `event`, folding them generically in two
/// passes — flat (`AddDamage`) contributions first, then multiplicative
/// (`MultiplyDamage`) ones — matching the wiki's documented damage order
/// (e.g. Strength applies before Vulnerable). Each pass ignores modifier
/// kinds it doesn't handle, so a new status that contributes either kind from
/// either side needs no change here, only a `modifier_for` entry.
fn apply_damage_modifiers(
    base: i32,
    attacker_statuses: &[Status],
    target_statuses: &[Status],
    event: EventType,
) -> i32 {
    let modifiers: Vec<Modifier> = attacker_statuses
        .iter()
        .chain(target_statuses.iter())
        .filter_map(|status| status.modifier_for(event))
        .collect();

    let additive = modifiers.iter().fold(base, |amount, modifier| match modifier {
        Modifier::AddDamage(delta) => amount + delta,
        Modifier::MultiplyDamage(_) => amount,
    });

    let multiplied = modifiers
        .iter()
        .fold(additive as f64, |amount, modifier| match modifier {
            Modifier::MultiplyDamage(factor) => amount * factor,
            Modifier::AddDamage(_) => amount,
        });

    multiplied.floor() as i32
}

/// A single step in a card's declarative effect pipeline. A generic engine
/// interprets these against a `CombatState`, so that adding an ordinary card
/// means adding data to `card_data` rather than new engine logic.
#[derive(Clone, PartialEq)]
enum EffectOp {
    DealDamage(i32),
    GainBlock(i32),
    // Applies to whichever combatant the card resolved a target against
    // (e.g. Bash's Vulnerable lands on the monster it struck).
    ApplyStatusToTarget(Status),
    // Applies to the player directly, regardless of targeting — for
    // self-buffs like Inflame's Strength, which never go through SelectTarget.
    ApplyStatusToSelf(Status),
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
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
        }),
        "Iron Wave" => Some(CardData {
            cost: 1,
            targeted: true,
            effects: vec![EffectOp::DealDamage(5), EffectOp::GainBlock(5)],
        }),
        "Inflame" => Some(CardData {
            cost: 1,
            targeted: false,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))],
        }),
        _ => None,
    }
}

fn run_effect_ops(state: &mut CombatState, ops: &[EffectOp]) {
    for op in ops {
        match op {
            EffectOp::DealDamage(amount) => {
                let modified = apply_damage_modifiers(
                    *amount,
                    &state.player_statuses,
                    &state.monster_statuses,
                    EventType::OnDamageDealt,
                );
                state.monster_hp -= modified;
            }
            EffectOp::GainBlock(amount) => state.player_block += amount,
            EffectOp::ApplyStatusToTarget(status) => state.monster_statuses.push(status.clone()),
            EffectOp::ApplyStatusToSelf(status) => state.player_statuses.push(status.clone()),
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
    // What `EndTurn` refreshes `player_energy` back up to each turn — Slay
    // the Spire characters draw a fixed energy amount every turn rather than
    // carrying it over. Defaults to the starting `player_energy` (a fresh
    // combat begins at full); the override exists for the same reason as
    // `player_max_hp`/`monster_max_hp` — relics/potions that raise max energy
    // mid-run are a real mechanic, and tests may want to start mid-turn with
    // partially-spent energy without that looking like the per-turn maximum.
    player_max_energy: i32,
    #[pyo3(get)]
    player_block: i32,
    player_statuses: Vec<Status>,
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
    #[pyo3(signature = (player_hp, player_energy, monster_hp, monster_attack, seed, hand=Vec::new(), player_max_hp=None, monster_max_hp=None, player_max_energy=None))]
    fn new(
        player_hp: i32,
        player_energy: i32,
        monster_hp: i32,
        monster_attack: i32,
        seed: u64,
        hand: Vec<String>,
        player_max_hp: Option<i32>,
        monster_max_hp: Option<i32>,
        player_max_energy: Option<i32>,
    ) -> Self {
        CombatState {
            player_hp,
            player_max_hp: player_max_hp.unwrap_or(player_hp),
            player_energy,
            player_max_energy: player_max_energy.unwrap_or(player_energy),
            player_block: 0,
            player_statuses: Vec::new(),
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

    #[getter]
    fn player_statuses(&self) -> Vec<String> {
        self.player_statuses.iter().map(|s| s.as_str().to_string()).collect()
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
                // Mirrors Slay the Spire greying out cards you can't afford —
                // unaffordable cards are never legal plays, so the engine
                // never has to model (or reject) going into negative energy.
                .filter(|name| {
                    card_data(name)
                        .map(|data| data.cost <= state.player_energy)
                        .unwrap_or(false)
                })
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
            // Energy refreshes to its per-turn maximum each turn — it does
            // not carry over either.
            next.player_energy = next.player_max_energy;
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
                if data.cost > next.player_energy {
                    return Err(PyValueError::new_err(format!(
                        "cannot afford {card_name}: costs {} but only {} energy available",
                        data.cost, next.player_energy
                    )));
                }
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
