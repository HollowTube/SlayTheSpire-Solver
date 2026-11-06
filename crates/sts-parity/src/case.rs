//! Cases, their outcomes, and how an outcome is classified.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sts_bridge::{Fixture, GameState};

use crate::expected::Expected;
use crate::map::{Carry, ParityState, SimFailure};
use crate::step::{diff, sim_step, Mask, ParityDiff, ParityStep};

/// What a blocked case is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Needs {
    /// A backend setter: the case runs once its fixture is reachable.
    Setters,
    /// A bridge action that picks a hand card, which does not exist yet.
    HandSelection,
}

/// A case that is declared but not run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocked {
    pub by: String,
    pub reason: String,
    pub needs: Needs,
}

impl ParityCase {
    /// Whether the case is still blocked for a backend with `setters`.
    pub fn blocked_for(&self, setters: sts_bridge::Setters) -> Option<&Blocked> {
        let b = self.blocked.as_ref()?;
        match b.needs {
            Needs::HandSelection => Some(b),
            Needs::Setters => (!self.fixture.unreachable(setters).is_empty()).then_some(b),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityCase {
    pub name: String,
    /// The cards under test (`Name` or `Name+`), for the filter and report.
    pub cards: Vec<String>,
    pub fixture: Fixture,
    pub steps: Vec<ParityStep>,
    pub masks: Vec<Mask>,
    pub blocked: Option<Blocked>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepOutcome {
    pub step: ParityStep,
    pub before: GameState,
    pub carry: Carry,
    pub game_after: GameState,
    pub sim_after: Result<ParityState, SimFailure>,
    pub diff: Vec<ParityDiff>,
}

/// Why a case stopped before its steps ran out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseError {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParityOutcome {
    pub case: ParityCase,
    pub verified: Option<GameState>,
    pub steps: Vec<StepOutcome>,
    pub error: Option<CaseError>,
    pub game_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// Every diff is listed; the issue ids.
    Expected(Vec<String>),
    Fail(Vec<String>),
    /// Listed entries that no longer diverge.
    Stale(Vec<String>),
    Blocked(String),
}

impl Verdict {
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Expected(_) => "EXPECTED",
            Verdict::Fail(_) => "FAIL",
            Verdict::Stale(_) => "STALE",
            Verdict::Blocked(_) => "BLOCKED",
        }
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, Verdict::Fail(_) | Verdict::Stale(_))
    }
}

/// Pure: the outcome and the expected list decide the verdict.
pub fn classify(outcome: &ParityOutcome, expected: &[Expected]) -> Verdict {
    let case = &outcome.case;
    if let Some(b) = &case.blocked {
        return Verdict::Blocked(format!("{}: {}", b.by, b.reason));
    }
    let mut fails = Vec::new();
    if let Some(e) = &outcome.error {
        fails.push(format!("{}: {}", e.kind, e.message));
    }
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut issues = BTreeSet::new();
    for s in &outcome.steps {
        match &s.sim_after {
            Err(f) => fails.push(format!("{}: {f}", s.step.card)),
            Ok(_) => {
                for d in &s.diff {
                    match expected
                        .iter()
                        .find(|e| e.card == s.step.card && e.field == d.field)
                    {
                        Some(e) => {
                            seen.insert((e.card.to_string(), e.field.to_string()));
                            issues.insert(e.issue.to_string());
                        }
                        None => fails.push(format!("{}: {d}", s.step.card)),
                    }
                }
            }
        }
    }
    if !fails.is_empty() {
        return Verdict::Fail(fails);
    }
    // A stale entry is only judged when every step of the case ran.
    let stale: Vec<String> = expected
        .iter()
        .filter(|e| case.cards.iter().any(|c| c == e.card))
        .filter(|e| !seen.contains(&(e.card.to_string(), e.field.to_string())))
        .map(|e| {
            format!(
                "{}: {} matches now; remove it from the expected list ({})",
                e.card, e.field, e.issue
            )
        })
        .collect();
    if !stale.is_empty() {
        return Verdict::Stale(stale);
    }
    if issues.is_empty() {
        Verdict::Pass
    } else {
        Verdict::Expected(issues.into_iter().collect())
    }
}

/// One step's record, from the state before, the carry going in and the
/// game's state after. Returns the carry to feed the next step. Replaying
/// `sim_step(&before, &carry, &step)` gives `sim_after` again.
pub fn record_step(
    before: &GameState,
    carry: &Carry,
    step: &ParityStep,
    game_after: &GameState,
    masks: &[Mask],
) -> Result<(StepOutcome, Carry), SimFailure> {
    let game = ParityState::from_game(game_after)?;
    let (sim_after, d, next) = match sim_step(before, carry, step) {
        Ok(s) => {
            let d = diff(&s.after, &game, masks);
            (Ok(s.after), d, s.carry)
        }
        Err(f) => (Err(f), Vec::new(), carry.clone()),
    };
    Ok((
        StepOutcome {
            step: step.clone(),
            before: before.clone(),
            carry: carry.clone(),
            game_after: game_after.clone(),
            sim_after,
            diff: d,
        },
        next,
    ))
}
