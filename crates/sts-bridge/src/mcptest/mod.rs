//! Backend for the MCPTest bridge mod (HollowTube/sts2-modding-mcp).

mod transport;
mod wire;

use std::time::Instant;

use serde::Deserialize;
use serde_json::{json, Value};

pub use transport::{
    ReplayTransport, Request, TcpTransport, Transport, DEFAULT_PORT, DEFAULT_TIMEOUT,
    MAX_RESPONSE_BYTES,
};

use crate::backend::{Backend, BridgeError};
use crate::command::{Ack, Command};
use crate::state::{Actions, GameState, Health, Screen};
use wire::{parse, WireActions, WireCardPiles, WireCombat, WireCombatFlag, WireRun, WireRunFlag};

pub struct McpTestBackend<T: Transport> {
    transport: T,
}

impl McpTestBackend<TcpTransport> {
    /// Connects per request to `STS2_BRIDGE_HOST` (default `127.0.0.1`):21337.
    pub fn from_env() -> Self {
        Self::new(TcpTransport::from_env())
    }
}

impl<T: Transport> McpTestBackend<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub(crate) fn call(
        &mut self,
        method: &str,
        params: Option<&Value>,
    ) -> Result<Value, BridgeError> {
        let raw = self.transport.send(method, params)?;
        transport::decode(method, &raw)
    }

    fn read_actions(&mut self) -> Result<WireActions, BridgeError> {
        const METHOD: &str = "get_available_actions";
        let value = self.call(METHOD, None)?;
        parse(METHOD, &value)
    }
}

impl<T: Transport> Backend for McpTestBackend<T> {
    fn send(&mut self, command: &Command) -> Result<Ack, BridgeError> {
        let (method, params) = match command {
            Command::PlayCard { card_index, target } => {
                let mut params = json!({ "card_index": card_index });
                if let Some(t) = target {
                    params["target_index"] = json!(t);
                }
                ("play_card", Some(params))
            }
            Command::EndTurn => ("end_turn", None),
            Command::Choose(a) => {
                let mut params = a.params.clone();
                params.insert("action".into(), json!(a.action));
                if let Some(i) = a.card_index {
                    params.insert("card_index".into(), json!(i));
                }
                if let Some(i) = a.target_index {
                    params.insert("target_index".into(), json!(i));
                }
                ("execute_action", Some(Value::Object(params)))
            }
        };
        let refused = |reason: String| BridgeError::ActionRefused {
            action: command.to_string(),
            reason,
        };
        let raw = self.transport.send(method, params.as_ref())?;
        // The handler's own `error` means it rejected the command. A
        // request-level one (dispatch timeout, serialization failure) leaves
        // the outcome unknown, so it stays `Remote`.
        let reply = match transport::decode(method, &raw) {
            Err(BridgeError::Remote { message, .. }) if !transport::is_envelope_error(&raw) => {
                return Err(refused(message))
            }
            other => other?,
        };
        // The bridge reports a rejected play as `success: false`, not an error.
        if reply.get("success") == Some(&Value::Bool(false)) {
            let detail = ["message", "reason"]
                .iter()
                .find_map(|k| reply.get(*k).and_then(Value::as_str))
                .map_or(String::new(), |m| format!(": {m}"));
            return Err(refused(format!(
                "bridge replied success: false{detail} ({reply})"
            )));
        }
        Ok(Ack::new(reply))
    }

    fn state(&mut self) -> Result<GameState, BridgeError> {
        let screen = self.read_actions()?.screen();

        const RUN: &str = "get_run_state";
        let run_value = self.call(RUN, None)?;
        let run = if parse::<WireRunFlag>(RUN, &run_value)?.in_progress {
            Some(parse::<WireRun>(RUN, &run_value)?.into_run_info(RUN)?)
        } else {
            None
        };

        // Not gated on a COMBAT* screen: in-combat overlays (HAND_SELECT,
        // CARD_SELECTION) still carry a combat.
        let combat = if run.is_some() && !screen.is_loading() {
            const COMBAT: &str = "get_combat_state";
            let combat_value = self.call(COMBAT, None)?;
            if parse::<WireCombatFlag>(COMBAT, &combat_value)?.in_combat {
                let combat = parse::<WireCombat>(COMBAT, &combat_value)?;
                const PILES: &str = "get_card_piles";
                let piles_value = self.call(PILES, None)?;
                let piles = parse::<WireCardPiles>(PILES, &piles_value)?;
                Some(combat.into_combat_view(COMBAT, piles)?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(GameState {
            screen,
            run,
            combat,
        })
    }

    fn actions(&mut self) -> Result<Actions, BridgeError> {
        let wire = self.read_actions()?;
        Ok(Actions {
            screen: wire.screen(),
            screen_context: wire.screen_context_type,
            actions: wire.actions,
        })
    }

    fn health(&mut self) -> Result<Health, BridgeError> {
        #[derive(Deserialize)]
        struct WirePing {
            status: String,
            #[serde(rename = "mod")]
            backend: String,
            version: Option<String>,
            screen: Option<String>,
            run_in_progress: Option<bool>,
            in_combat: Option<bool>,
        }
        const METHOD: &str = "ping";
        let started = Instant::now();
        let value = self.call(METHOD, None)?;
        let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
        let ping: WirePing = parse(METHOD, &value)?;
        Ok(Health {
            ok: ping.status == "ok",
            latency_ms,
            backend: ping.backend,
            version: ping.version,
            screen: ping.screen.map(Screen),
            run_in_progress: ping.run_in_progress,
            in_combat: ping.in_combat,
        })
    }
}
