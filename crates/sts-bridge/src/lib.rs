//! Typed interface to a live Slay the Spire 2 game.
//!
//! Consumers use [`Backend`] and [`GameState`] only; [`mcptest`] is one
//! backend behind them.

pub mod backend;
pub mod command;
pub mod console;
pub mod fixture;
pub mod lifecycle;
pub mod mcptest;
pub mod menu;
pub mod schema;
pub mod settle;
pub mod state;

pub use backend::{Backend, BridgeError, ProtocolError, Setters, WedgeReason};
pub use command::{Ack, Command, MenuNav, RunSpec};
pub use console::{
    CombatPile, ConsoleCommand, ConsoleWait, Encounter, PowerName, Relic, Target, UnknownId,
    CONSOLE_TIMEOUT, NIBBITS_WEAK,
};
pub use fixture::{
    plan, CardSpec, EnemyPowers, EnemySpec, Energy, FieldDiff, Fixture, FixtureError, FixturePath,
    Fixtures, Hp, InstantMode, PlayerSpec, Projection, ResetTimings, Step, Timings, VerifiedState,
};
#[cfg(unix)]
pub use lifecycle::SteamLauncher;
pub use lifecycle::{Game, GameStatus, Launcher, Recovered, StartedRun, Timeouts};
pub use mcptest::McpTestBackend;
pub use menu::{classify, ensure_main_menu, wait_run_ready, Phase, Probe, ScreenDetail};
pub use settle::{
    is_interactive, poll_until, wait_settled, wait_until, Observation, Session, SettlePolicy, Unmet,
};
pub use state::*;
