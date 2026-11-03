//! Typed interface to a live Slay the Spire 2 game.
//!
//! Consumers use [`Backend`] and [`GameState`] only; [`mcptest`] is one
//! backend behind them.

pub mod backend;
pub mod command;
pub mod mcptest;
pub mod schema;
pub mod settle;
pub mod state;

pub use backend::{Backend, BridgeError, ProtocolError};
pub use command::{Ack, Command};
pub use mcptest::McpTestBackend;
pub use settle::{
    is_interactive, poll_until, wait_settled, wait_until, Observation, Session, SettlePolicy, Unmet,
};
pub use state::*;
