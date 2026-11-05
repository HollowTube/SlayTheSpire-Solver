//! The game interface every consumer uses, independent of the mod behind it.

use std::fmt;
use std::time::Duration;

use crate::command::{Ack, Command};
use crate::menu::Probe;
use crate::state::{Actions, GameState, Health, Screen};

/// Operations on a live game. `&mut self` keeps one command in flight.
pub trait Backend {
    /// Sends one command and returns once the backend accepted it, not once
    /// its effect is visible ([`crate::Session::act`] waits for that). A
    /// command the backend rejects is [`BridgeError::ActionRefused`].
    fn send(&mut self, command: &Command) -> Result<Ack, BridgeError>;

    /// The full typed state. Fails if any part of it cannot be read; never
    /// returns a partial or defaulted state.
    fn state(&mut self) -> Result<GameState, BridgeError>;

    /// The actions the game accepts on its current screen.
    fn actions(&mut self) -> Result<Actions, BridgeError>;

    /// A ping round-trip.
    fn health(&mut self) -> Result<Health, BridgeError>;

    fn ping(&mut self) -> Result<Health, BridgeError> {
        self.health()
    }

    /// A cheap read of where the game is, from which [`crate::classify`]
    /// decides its [`crate::Phase`]. Sends nothing that changes the game.
    fn probe(&mut self) -> Result<Probe, BridgeError>;

    /// What this backend can set exactly, beyond the dev console.
    fn setters(&self) -> Setters {
        Setters::default()
    }
}

impl<B: Backend + ?Sized> Backend for &mut B {
    fn send(&mut self, command: &Command) -> Result<Ack, BridgeError> {
        (**self).send(command)
    }

    fn state(&mut self) -> Result<GameState, BridgeError> {
        (**self).state()
    }

    fn actions(&mut self) -> Result<Actions, BridgeError> {
        (**self).actions()
    }

    fn health(&mut self) -> Result<Health, BridgeError> {
        (**self).health()
    }

    fn ping(&mut self) -> Result<Health, BridgeError> {
        (**self).ping()
    }

    fn probe(&mut self) -> Result<Probe, BridgeError> {
        (**self).probe()
    }

    fn setters(&self) -> Setters {
        (**self).setters()
    }
}

/// Fixture fields a backend can set exactly, which the dev console cannot.
/// The default is none: console commands only. A flag set here makes
/// [`crate::fixture::plan`] treat the field as reachable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Setters {
    /// Any hp, including above an enemy's spawned hp or the player's max.
    pub exact_hp: bool,
    /// Energy below the turn's start.
    pub energy: bool,
    /// Lowering or removing a power.
    pub remove_power: bool,
    /// Setting the draw, discard and exhaust piles outright.
    pub exact_piles: bool,
    /// Upgraded cards outside the hand.
    pub upgraded_in_piles: bool,
    pub round: bool,
    pub intent: bool,
    /// Restoring a saved state, in place of a full reset.
    pub snapshot: bool,
}

/// Why the game is considered wedged.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WedgeReason {
    /// The bridge accepted the connection but never answered.
    BridgeTimeout,
    /// The bridge answered, but its main-thread dispatch timed out.
    MainThreadTimeout,
    /// Ping kept answering without a screen.
    PingWithoutScreen,
    /// This screen held past the stuck-screen deadline.
    StuckScreen(String),
    /// A transition (e.g. the enemy turn) never completed.
    TransitionTimeout,
}

impl fmt::Display for WedgeReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WedgeReason::BridgeTimeout => f.write_str("the bridge stopped answering"),
            WedgeReason::MainThreadTimeout => {
                f.write_str("the game's main thread stopped answering")
            }
            WedgeReason::PingWithoutScreen => f.write_str("ping kept answering without a screen"),
            WedgeReason::StuckScreen(s) => write!(f, "stuck on {s}"),
            WedgeReason::TransitionTimeout => f.write_str("a transition never completed"),
        }
    }
}

fn screen_name(screen: &Option<Screen>) -> &str {
    screen.as_ref().map_or("none read", Screen::as_str)
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BridgeError {
    #[error(
        "bridge refused the connection at {addr}: is the game running with the MCPTest mod loaded?"
    )]
    Refused { addr: String },

    #[error(
        "bridge did not answer `{method}` within {after:?}: the game may be loading or wedged"
    )]
    Timeout { method: String, after: Duration },

    #[error("bridge I/O failed during `{method}`: {source}")]
    Io {
        method: String,
        #[source]
        source: std::io::Error,
    },

    #[error("bridge returned an error for `{method}`: {message}")]
    Remote { method: String, message: String },

    #[error("bridge sent a malformed reply to `{method}`: {kind}")]
    Protocol { method: String, kind: ProtocolError },

    #[error("bridge reply to `{method}` does not match the contract at `{path}`: {message}")]
    Schema {
        method: String,
        path: String,
        message: String,
    },

    /// The command was rejected, by the backend or by a local check against
    /// the settled state; it had no effect.
    #[error("`{action}` was refused: {reason}")]
    ActionRefused { action: String, reason: String },

    /// The game settled, but `expected` did not happen.
    #[error("`{action}` settled without its effect: expected {expected}, observed {observed}")]
    PostconditionFailed {
        action: String,
        expected: String,
        observed: String,
    },

    /// `unmet` did not hold on a stable read within `waited`.
    #[error(
        "the game did not settle within {waited:?}: {unmet} never held (last screen: {})",
        screen.as_ref().map_or("none read", Screen::as_str)
    )]
    NotSettled {
        waited: Duration,
        screen: Option<Screen>,
        unmet: String,
    },

    /// The game is running but not making progress; a restart recovers it.
    #[error("the game is wedged: {reason} (last screen: {})", screen_name(screen))]
    Wedged {
        screen: Option<Screen>,
        reason: WedgeReason,
    },

    /// The game process we were driving is gone and the bridge refuses.
    #[error(
        "the game crashed: process {} is gone and the bridge refuses (last screen: {})",
        pid.map_or_else(|| "?".to_string(), |p| p.to_string()),
        screen_name(last_screen)
    )]
    Crashed {
        pid: Option<u32>,
        last_screen: Option<Screen>,
    },

    /// Launching or stopping the game failed.
    #[error("game {op} failed: {message}")]
    Lifecycle { op: String, message: String },

    /// The GodotExplorer mod, which some menu steps need, is unreachable.
    #[error(
        "GodotExplorer is unreachable at {addr} ({reason}): is the game running with the GodotExplorer mod (port 27020)?"
    )]
    ExplorerUnavailable { addr: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    #[error("empty response")]
    Empty,
    #[error("response is not UTF-8")]
    NonUtf8,
    #[error("response is not valid JSON: {0}")]
    InvalidJson(String),
    #[error("response exceeded {limit} bytes")]
    TooLarge { limit: usize },
}
