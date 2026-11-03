//! The game interface every consumer uses, independent of the mod behind it.

use std::time::Duration;

use crate::command::{Ack, Command};
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
