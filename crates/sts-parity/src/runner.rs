//! Drives a case against the live game.

use std::time::{Duration, Instant};

use sts_bridge::{
    Backend, BridgeError, Command, Entity, FixtureError, Fixtures, GameState, Launcher, Unmet,
};

use crate::case::{record_step, CaseError, ParityCase, ParityOutcome};
use crate::map::Carry;
use crate::step::StepAction;

/// A state read slower than this marks the bridge slow.
pub const SLOW_READ: Duration = Duration::from_millis(300);

/// The run cannot go on: the game is wedged or gone. Never retried.
#[derive(Debug)]
pub struct Stop(pub BridgeError);

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The median of five state reads; whether the bridge is slow.
pub fn calibrate<B: Backend>(backend: &mut B) -> Result<(Duration, bool), BridgeError> {
    let mut reads = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        backend.state()?;
        reads.push(started.elapsed());
    }
    reads.sort();
    let median = reads[reads.len() / 2];
    Ok((median, median > SLOW_READ))
}

fn fatal(e: &BridgeError) -> bool {
    matches!(e, BridgeError::Wedged { .. } | BridgeError::Crashed { .. })
}

fn kind(e: &BridgeError) -> &'static str {
    match e {
        BridgeError::ActionRefused { .. } => "ActionRefused",
        BridgeError::PostconditionFailed { .. } => "PostconditionFailed",
        BridgeError::NotSettled { .. } => "NotSettled",
        _ => "BridgeError",
    }
}

pub struct Runner;

impl Runner {
    /// Runs one case. `Err` only when the game is wedged or crashed.
    pub fn run<B: Backend, L: Launcher>(
        fx: &mut Fixtures<B, L>,
        case: &ParityCase,
    ) -> Result<ParityOutcome, Stop> {
        let setters = fx.game_mut().backend().setters();
        let mut case = case.clone();
        if case.blocked_for(setters).is_none() {
            case.blocked = None;
        }
        let case = &case;
        let mut outcome = ParityOutcome {
            case: case.clone(),
            verified: None,
            steps: Vec::new(),
            error: None,
            game_version: None,
        };
        if case.blocked.is_some() {
            return Ok(outcome);
        }
        let verified = match fx.ensure(&case.fixture) {
            Ok(v) => v,
            Err(e) => {
                if let FixtureError::Bridge { source, .. } = &e {
                    if fatal(source) {
                        return Err(Stop(clone_fatal(source)));
                    }
                }
                outcome.error = Some(CaseError {
                    kind: "FixtureError".into(),
                    message: e.to_string(),
                });
                return Ok(outcome);
            }
        };
        outcome.game_version = fx
            .game_mut()
            .backend_mut()
            .health()
            .ok()
            .and_then(|h| h.version);
        let mut state = verified.state.clone();
        outcome.verified = Some(verified.state);
        let enemies = state.combat.as_ref().map_or(0, |c| c.enemies.len());
        let mut carry = Carry::fresh(enemies);
        for step in &case.steps {
            let before = state.clone();
            let after = match act(fx, &before, &step.action) {
                Ok(s) => s,
                Err(e) => {
                    if fatal(&e) {
                        return Err(Stop(e));
                    }
                    outcome.error = Some(CaseError {
                        kind: kind(&e).into(),
                        message: format!("{}: {e}", step.card),
                    });
                    return Ok(outcome);
                }
            };
            match record_step(&before, &carry, step, &after, &case.masks) {
                Ok((step_outcome, next)) => {
                    outcome.steps.push(step_outcome);
                    carry = next;
                }
                Err(f) => {
                    outcome.error = Some(CaseError {
                        kind: "Mapping".into(),
                        message: format!("{}: {f}", step.card),
                    });
                    return Ok(outcome);
                }
            }
            state = after;
        }
        Ok(outcome)
    }
}

fn clone_fatal(e: &BridgeError) -> BridgeError {
    match e {
        BridgeError::Wedged { screen, reason } => BridgeError::Wedged {
            screen: screen.clone(),
            reason: reason.clone(),
        },
        BridgeError::Crashed { pid, last_screen } => BridgeError::Crashed {
            pid: *pid,
            last_screen: last_screen.clone(),
        },
        other => BridgeError::Lifecycle {
            op: "run".into(),
            message: other.to_string(),
        },
    }
}

fn act<B: Backend, L: Launcher>(
    fx: &mut Fixtures<B, L>,
    before: &GameState,
    action: &StepAction,
) -> Result<GameState, BridgeError> {
    let mut session = fx.session();
    match action {
        StepAction::EndTurn => session.act(&Command::EndTurn),
        StepAction::PlayCard { card, target } => {
            let hand = before
                .combat
                .as_ref()
                .map(|c| &c.piles.hand[..])
                .unwrap_or(&[]);
            let card_ref = hand
                .iter()
                .find(|h| h.id == Entity::Known(card.id) && h.upgraded == card.upgraded)
                .ok_or_else(|| BridgeError::ActionRefused {
                    action: format!("play {}", card.id.as_str()),
                    reason: "not in the hand".into(),
                })?;
            let target = match &card_ref.valid_targets {
                Some(valid) if !valid.is_empty() => match target {
                    Some(t) if valid.contains(t) => Some(*t),
                    other => {
                        return Err(BridgeError::ActionRefused {
                            action: format!("play {}", card.id.as_str()),
                            reason: format!("case target {other:?} is not in {valid:?}"),
                        })
                    }
                },
                _ => None,
            };
            let command = Command::PlayCard {
                card_index: card_ref.index,
                target,
            };
            if card_ref.kind == "Power" {
                session.act_with(&command, |b, a| {
                    let powers = |s: &GameState| s.combat.as_ref().map(|c| c.player.powers.clone());
                    if powers(b) != powers(a) {
                        Ok(())
                    } else {
                        Err(Unmet::new("the player's powers to change", "unchanged"))
                    }
                })
            } else {
                session.act(&command)
            }
        }
    }
}
