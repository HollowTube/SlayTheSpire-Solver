//! Client for the GodotExplorer mod: MCP `tools/call` over the same
//! newline-delimited JSON-RPC as the bridge, on its own port.

use serde::Deserialize;
use serde_json::{json, Value};

use super::transport::{self, Transport};
use crate::backend::BridgeError;

pub const EXPLORER_PORT: u16 = 27020;

pub(crate) struct Explorer {
    transport: Box<dyn Transport>,
    pub(crate) addr: String,
}

impl Explorer {
    pub(crate) fn new(transport: impl Transport + 'static, addr: String) -> Self {
        Self {
            transport: Box::new(transport),
            addr,
        }
    }

    /// Calls one explorer tool and returns its text result. A tool error
    /// (`isError`) is [`BridgeError::ActionRefused`]; a JSON-RPC error is
    /// [`BridgeError::Remote`]. Failing to connect is
    /// [`BridgeError::ExplorerUnavailable`]; a connection that never answers
    /// stays [`BridgeError::Timeout`], since the explorer answers from the
    /// game's main thread and its hang is the game's.
    pub(crate) fn call(&mut self, tool: &str, arguments: Value) -> Result<String, BridgeError> {
        const METHOD: &str = "tools/call";
        let params = json!({ "name": tool, "arguments": arguments });
        let raw = self
            .transport
            .send(METHOD, Some(&params))
            .map_err(|e| match e {
                BridgeError::Refused { .. } | BridgeError::Io { .. } => {
                    BridgeError::ExplorerUnavailable {
                        addr: self.addr.clone(),
                        reason: e.to_string(),
                    }
                }
                other => other,
            })?;
        let result = transport::decode(METHOD, &raw)?;
        let text = result
            .get("content")
            .and_then(Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if result.get("isError") == Some(&Value::Bool(true)) {
            return Err(BridgeError::ActionRefused {
                action: format!("explorer {tool}"),
                reason: text,
            });
        }
        Ok(text)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Node {
    pub name: String,
    pub path: String,
}

/// The nodes in a `find_nodes` result.
pub(crate) fn nodes(text: &str) -> Result<Vec<Node>, String> {
    #[derive(Deserialize)]
    struct Found {
        results: Vec<Node>,
    }
    serde_json::from_str::<Found>(text)
        .map(|f| f.results)
        .map_err(|e| format!("{e}: {text}"))
}
