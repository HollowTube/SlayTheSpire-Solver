//! One step through the sim, and the diff against the game.

use std::collections::BTreeSet;
use std::panic::{catch_unwind, AssertUnwindSafe};

use serde::{Deserialize, Serialize};
use sts_bridge::{CardSpec, GameState};
use sts_sim::state::PendingDecision;
use sts_sim::{CombatState, SimAction};

use crate::map::{to_parts, Carry, ParityState, SimFailure};

/// What a step does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepAction {
    PlayCard {
        card: CardSpec,
        target: Option<usize>,
    },
    EndTurn,
}

/// One step of a case. `card` is the card under test (`Name` or `Name+`)
/// that the step's diffs are attributed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityStep {
    pub action: StepAction,
    pub card: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimStep {
    pub after: ParityState,
    pub carry: Carry,
}

fn resolve_pending(state: &mut CombatState, target: Option<usize>) -> Result<(), SimFailure> {
    for _ in 0..8 {
        match state.pending() {
            None => return Ok(()),
            Some(PendingDecision::SelectTarget { .. }) => {
                let t = target.ok_or_else(|| {
                    SimFailure::PendingChoice("the sim asks for a target, the step has none".into())
                })?;
                state
                    .step(&SimAction::SelectTarget(t))
                    .map_err(|e| SimFailure::Sim(e.to_string()))?;
            }
            Some(other) => {
                let name = other.as_str();
                let options = state.legal_actions();
                if options.len() != 1 {
                    return Err(SimFailure::PendingChoice(format!(
                        "{name} with {} options",
                        options.len()
                    )));
                }
                state
                    .step(&options[0])
                    .map_err(|e| SimFailure::Sim(e.to_string()))?;
            }
        }
    }
    Err(SimFailure::PendingChoice("choices never ended".into()))
}

fn run(before: &GameState, carry: &Carry, step: &ParityStep) -> Result<SimStep, SimFailure> {
    let combat = before
        .combat
        .as_ref()
        .ok_or_else(|| SimFailure::Mapping(format!("no combat on {}", before.screen)))?;
    let parts = to_parts(combat, carry)?;
    let mut state = CombatState::from_parts(parts).map_err(|e| SimFailure::Sim(e.to_string()))?;
    match &step.action {
        StepAction::EndTurn => state
            .step(&SimAction::EndTurn)
            .map_err(|e| SimFailure::Sim(e.to_string()))?,
        StepAction::PlayCard { card, target } => {
            state
                .step(&SimAction::PlayCard {
                    card: card.id,
                    upgrades: u8::from(card.upgraded),
                })
                .map_err(|e| SimFailure::Sim(e.to_string()))?;
            resolve_pending(&mut state, *target)?;
        }
    }
    let after = state.parts();
    Ok(SimStep {
        after: ParityState::from_sim(&after),
        carry: after.counters.into(),
    })
}

/// Runs `f`, turning a panic into a [`SimFailure::Panic`].
pub fn guarded<T>(f: impl FnOnce() -> Result<T, SimFailure>) -> Result<T, SimFailure> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown panic".into());
            Err(SimFailure::Panic(msg))
        }
    }
}

/// Runs `step` in the sim from the game's state `before` and the carried
/// counters. Any failure, including a panic, is an `Err`.
pub fn sim_step(
    before: &GameState,
    carry: &Carry,
    step: &ParityStep,
) -> Result<SimStep, SimFailure> {
    guarded(|| run(before, carry, step))
}

/// A field whose identity (not count) is not compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mask {
    /// `hand`, `draw`, `discard` or `exhaust`.
    pub field: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityDiff {
    pub field: String,
    pub sim: String,
    pub game: String,
}

impl std::fmt::Display for ParityDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: sim {}, game {}", self.field, self.sim, self.game)
    }
}

fn list(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

/// Every field where `sim` and `game` differ, by path.
pub fn diff(sim: &ParityState, game: &ParityState, masks: &[Mask]) -> Vec<ParityDiff> {
    let mut out = Vec::new();
    fn check_into(out: &mut Vec<ParityDiff>, field: String, s: String, g: String) {
        if s != g {
            out.push(ParityDiff {
                field,
                sim: s,
                game: g,
            });
        }
    }
    let (p, q) = (&sim.player, &game.player);
    check_into(
        &mut out,
        "player.hp".into(),
        p.hp.to_string(),
        q.hp.to_string(),
    );
    check_into(
        &mut out,
        "player.max_hp".into(),
        p.max_hp.to_string(),
        q.max_hp.to_string(),
    );
    check_into(
        &mut out,
        "player.block".into(),
        p.block.to_string(),
        q.block.to_string(),
    );
    check_into(
        &mut out,
        "player.energy".into(),
        p.energy.to_string(),
        q.energy.to_string(),
    );
    power_diffs(&mut out, "player", &p.powers, &q.powers);
    check_into(
        &mut out,
        "enemies".into(),
        sim.enemies.len().to_string(),
        game.enemies.len().to_string(),
    );
    for (i, (a, b)) in sim.enemies.iter().zip(&game.enemies).enumerate() {
        check_into(
            &mut out,
            format!("enemies[{i}].monster"),
            a.monster.clone(),
            b.monster.clone(),
        );
        check_into(
            &mut out,
            format!("enemies[{i}].hp"),
            a.hp.to_string(),
            b.hp.to_string(),
        );
        check_into(
            &mut out,
            format!("enemies[{i}].max_hp"),
            a.max_hp.to_string(),
            b.max_hp.to_string(),
        );
        check_into(
            &mut out,
            format!("enemies[{i}].block"),
            a.block.to_string(),
            b.block.to_string(),
        );
        check_into(
            &mut out,
            format!("enemies[{i}].alive"),
            a.alive.to_string(),
            b.alive.to_string(),
        );
        power_diffs(&mut out, &format!("enemies[{i}]"), &a.powers, &b.powers);
    }
    check_into(
        &mut out,
        "round".into(),
        sim.round.to_string(),
        game.round.to_string(),
    );
    for (name, a, b) in [
        ("hand", &sim.hand, &game.hand),
        ("draw", &sim.draw, &game.draw),
        ("discard", &sim.discard, &game.discard),
        ("exhaust", &sim.exhaust, &game.exhaust),
    ] {
        if masks.iter().any(|m| m.field == name) {
            check_into(
                &mut out,
                format!("{name}.len"),
                a.len().to_string(),
                b.len().to_string(),
            );
        } else {
            check_into(&mut out, name.into(), list(a), list(b));
        }
    }
    out
}

fn power_diffs(
    out: &mut Vec<ParityDiff>,
    who: &str,
    a: &std::collections::BTreeMap<String, i32>,
    b: &std::collections::BTreeMap<String, i32>,
) {
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for k in keys {
        let (x, y) = (
            a.get(k).copied().unwrap_or(0),
            b.get(k).copied().unwrap_or(0),
        );
        if x != y {
            out.push(ParityDiff {
                field: format!("{who}.powers[{k}]"),
                sim: x.to_string(),
                game: y.to_string(),
            });
        }
    }
}
