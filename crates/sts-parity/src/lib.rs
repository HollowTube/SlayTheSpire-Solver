//! Live sim-vs-game parity: each Ironclad card is played in the game and in
//! the sim from the same fixture, and the two states are compared field by
//! field. [`sim_step`] and [`diff`] are pure and run in CI; [`Runner`] drives
//! the live game.

pub mod case;
pub mod cases;
pub mod expected;
pub mod map;
pub mod runner;
pub mod step;

pub use case::{
    classify, record_step, Blocked, CaseError, Needs, ParityCase, ParityOutcome, StepOutcome,
    Verdict,
};
pub use cases::cases;
pub use expected::{Expected, EXPECTED};
pub use map::{Carry, ParityState, SimFailure};
pub use runner::{calibrate, Runner, Stop};
pub use step::{diff, guarded, sim_step, Mask, ParityDiff, ParityStep, SimStep, StepAction};
