//! The game interface every consumer uses, independent of the mod behind it.

use std::time::Duration;

use crate::state::{Actions, GameState, Health};

/// Read operations on a live game. `&mut self` keeps one command in flight.
pub trait Backend {
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
