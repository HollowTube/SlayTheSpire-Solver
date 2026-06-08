use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rand::{Rng, SeedableRng};
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

/// Which side of a damage exchange a status is sitting on — lets a status
/// declare it only contributes when it's the one *dealing* damage (Strength)
/// or only when it's the one *taking* it (Vulnerable). Without this, a status
/// that happens to exist on both combatants (e.g. the monster gaining
/// Strength via Bellow) would double-dip: amplifying its own attacks AND
/// (wrongly) the damage it takes from the player.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Attacker,
    Target,
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
    /// What this status contributes when `event` fires while sitting on
    /// `side`, if anything — the "listener registration" half of the
    /// event-bus skeleton. Declaring the side here (not just the status type)
    /// is what lets the same `Strength` show up on either combatant and only
    /// ever amplify *that combatant's own* outgoing damage.
    fn modifier_for(&self, side: Side, event: EventType) -> Option<Modifier> {
        match (self, side, event) {
            // Per the Slay the Spire wiki, Vulnerable increases damage taken
            // from attacks by 50%, rounded down — it amplifies what its
            // holder *receives*, so it only contributes from the target side.
            (Status::Vulnerable, Side::Target, EventType::OnDamageDealt) => {
                Some(Modifier::MultiplyDamage(1.5))
            }
            // Per the Slay the Spire wiki, each stack of Strength adds its
            // count directly to attack damage *dealt* — it only contributes
            // from the attacker side, regardless of which combatant holds it.
            (Status::Strength(amount), Side::Attacker, EventType::OnDamageDealt) => {
                Some(Modifier::AddDamage(*amount))
            }
            _ => None,
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
        .filter_map(|status| status.modifier_for(Side::Attacker, event))
        .chain(
            target_statuses
                .iter()
                .filter_map(|status| status.modifier_for(Side::Target, event)),
        )
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

/// Which combatant an effect pipeline is acting on behalf of — lets the same
/// `EffectOp` vocabulary and `run_effect_ops` engine drive both the player's
/// cards and the monster's moves, with "self"/"target" resolved generically
/// to the right side rather than hardcoded to the player.
#[derive(Clone, Copy, PartialEq)]
enum Actor {
    Player,
    Monster,
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

/// The move a monster telegraphs before it has acted at all — Slay the
/// Spire's documented monster patterns each name a fixed opening move (e.g.
/// Jaw Worm always opens with Chomp) before any RNG-driven selection kicks in.
fn opening_intent(monster_name: &str) -> Option<String> {
    match monster_name {
        "Jaw Worm" => Some("Chomp".to_string()),
        _ => None,
    }
}

/// A monster move's declarative effect pipeline — interpreted by the same
/// generic `run_effect_ops` engine as cards (with `Actor::Monster`), so that
/// adding an ordinary monster move means adding data here, not new logic.
fn monster_move(monster_name: &str, move_name: &str) -> Option<Vec<EffectOp>> {
    match (monster_name, move_name) {
        // Per the Slay the Spire wiki, Jaw Worm's move pool:
        ("Jaw Worm", "Chomp") => Some(vec![EffectOp::DealDamage(11)]),
        ("Jaw Worm", "Thrash") => Some(vec![EffectOp::DealDamage(7), EffectOp::GainBlock(5)]),
        ("Jaw Worm", "Bellow") => Some(vec![
            EffectOp::ApplyStatusToSelf(Status::Strength(3)),
            EffectOp::GainBlock(6),
        ]),
        _ => None,
    }
}

/// How many times a move may occur back-to-back before the AI is forced to
/// pick something else — per the wiki, Jaw Worm can't repeat Chomp or Bellow
/// at all, but may Thrash up to twice in a row (i.e. not a 3rd time).
fn max_streak(monster_name: &str, move_name: &str) -> u32 {
    match (monster_name, move_name) {
        ("Jaw Worm", "Thrash") => 2,
        ("Jaw Worm", _) => 1,
        _ => u32::MAX,
    }
}

/// Rolls the monster's next telegraphed move from its documented weighted
/// pattern, re-rolling whenever the result would extend a same-move streak
/// past its limit — e.g. Jaw Worm picks Bellow 45% / Thrash 30% / Chomp 25%
/// of the time, but never repeats Bellow/Chomp and never Thrashes a 3rd
/// consecutive time. `last_move`/`streak` describe the run of moves leading
/// up to (and including) the one that just resolved.
fn select_next_intent(
    monster_name: &str,
    last_move: &Option<String>,
    streak: u32,
    rng: &mut Pcg32,
) -> Option<String> {
    match monster_name {
        "Jaw Worm" => loop {
            let roll = rng.gen_range(0..100);
            let candidate = if roll < 45 {
                "Bellow"
            } else if roll < 75 {
                "Thrash"
            } else {
                "Chomp"
            };
            let resulting_streak = if last_move.as_deref() == Some(candidate) {
                streak + 1
            } else {
                1
            };
            if resulting_streak <= max_streak(monster_name, candidate) {
                return Some(candidate.to_string());
            }
        },
        _ => None,
    }
}

/// Deals damage from `actor` to the opposing combatant, running it through
/// that combatant's damage-modifier pipeline first and its block second —
/// the same absorb-then-spill arithmetic regardless of which side is hitting
/// which (mirrors Slay the Spire: block reduces incoming damage from anyone).
fn deal_damage(state: &mut CombatState, actor: Actor, amount: i32) {
    let (attacker_statuses, target_statuses) = match actor {
        Actor::Player => (&state.player_statuses, &state.monster_statuses),
        Actor::Monster => (&state.monster_statuses, &state.player_statuses),
    };
    let modified = apply_damage_modifiers(amount, attacker_statuses, target_statuses, EventType::OnDamageDealt);

    let (target_hp, target_block) = match actor {
        Actor::Player => (&mut state.monster_hp, &mut state.monster_block),
        Actor::Monster => (&mut state.player_hp, &mut state.player_block),
    };
    let absorbed = modified.min(*target_block);
    *target_block -= absorbed;
    *target_hp -= modified - absorbed;
}

fn run_effect_ops(state: &mut CombatState, ops: &[EffectOp], actor: Actor) {
    for op in ops {
        match op {
            EffectOp::DealDamage(amount) => deal_damage(state, actor, *amount),
            EffectOp::GainBlock(amount) => match actor {
                Actor::Player => state.player_block += amount,
                Actor::Monster => state.monster_block += amount,
            },
            EffectOp::ApplyStatusToTarget(status) => match actor {
                Actor::Player => state.monster_statuses.push(status.clone()),
                Actor::Monster => state.player_statuses.push(status.clone()),
            },
            EffectOp::ApplyStatusToSelf(status) => match actor {
                Actor::Player => state.player_statuses.push(status.clone()),
                Actor::Monster => state.monster_statuses.push(status.clone()),
            },
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
    // Mirrors `player_block`: absorbs incoming damage before HP, reset at the
    // start of the monster's own turn (e.g. Jaw Worm's Thrash/Bellow grant it
    // block that lasts through the player's turn). Always 0 for monsters with
    // no move pool — nothing in the trivial flat-attacker model grants it.
    #[pyo3(get)]
    monster_block: i32,
    monster_statuses: Vec<Status>,
    // The monster's species — looked up against move-pool data to drive
    // intent-based AI (e.g. "Jaw Worm"). `None` (the default) keeps the
    // original trivial fixed-`monster_attack` behavior used by the
    // placeholder monster and every pre-HOL-11 test.
    #[pyo3(get)]
    monster_name: Option<String>,
    // The move the monster has telegraphed for its next turn (e.g. "Chomp")
    // — mirrors how Slay the Spire shows enemy intent before the player acts.
    // `None` for monsters with no move pool (the trivial flat-attacker).
    monster_intent: Option<String>,
    // The name of the most recently *executed* move, and how many turns in a
    // row it's now run — the minimal state `select_next_intent` needs to
    // enforce "cannot repeat X" / "cannot use Y N times in a row" constraints
    // without replaying full move history.
    monster_last_move: Option<String>,
    monster_move_streak: u32,
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
    #[pyo3(signature = (player_hp, player_energy, monster_hp, monster_attack, seed, hand=Vec::new(), player_max_hp=None, monster_max_hp=None, player_max_energy=None, monster_name=None))]
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
        monster_name: Option<String>,
    ) -> Self {
        let monster_intent = monster_name.as_deref().and_then(opening_intent);
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
            monster_block: 0,
            monster_statuses: Vec::new(),
            monster_name,
            monster_intent,
            monster_last_move: None,
            monster_move_streak: 0,
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
    fn monster_intent(&self) -> Option<String> {
        self.monster_intent.clone()
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

            match next.monster_name.clone() {
                // Intent-driven monsters (e.g. Jaw Worm): their own turn
                // starts here — block resets, then the telegraphed move
                // resolves through the same generic effect-pipeline engine
                // cards use (with the monster as the actor).
                Some(name) => {
                    next.monster_block = 0;
                    if let Some(intent) = next.monster_intent.clone() {
                        if let Some(effects) = monster_move(&name, &intent) {
                            run_effect_ops(&mut next, &effects, Actor::Monster);
                        }
                        next.monster_move_streak = if next.monster_last_move.as_deref() == Some(intent.as_str()) {
                            next.monster_move_streak + 1
                        } else {
                            1
                        };
                        next.monster_last_move = Some(intent.clone());
                        next.monster_intent = select_next_intent(
                            &name,
                            &next.monster_last_move,
                            next.monster_move_streak,
                            &mut next.rng,
                        );
                    }
                }
                // The trivial flat-attacker (placeholder monster, every
                // pre-HOL-11 test): always swings for `monster_attack`.
                None => {
                    let absorbed = next.monster_attack.min(next.player_block);
                    next.player_hp -= next.monster_attack - absorbed;
                }
            }

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
                run_effect_ops(&mut next, &data.effects, Actor::Player);
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
                    run_effect_ops(&mut next, &data.effects, Actor::Player);
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
