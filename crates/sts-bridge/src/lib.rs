//! Typed interface to a live Slay the Spire 2 game.
//!
//! Consumers use [`Backend`] and [`GameState`] only; [`mcptest`] is one
//! backend behind them.

pub mod backend;
pub mod mcptest;
pub mod schema;
pub mod state;

pub use backend::{Backend, BridgeError, ProtocolError};
pub use mcptest::McpTestBackend;
pub use state::*;
