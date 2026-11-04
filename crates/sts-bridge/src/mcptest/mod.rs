//! Backend for the MCPTest bridge mod (HollowTube/sts2-modding-mcp).

mod explorer;
mod transport;
mod wire;

use std::time::Instant;

use serde::Deserialize;
use serde_json::{json, Value};

pub use explorer::EXPLORER_PORT;
pub use transport::{
    ReplayTransport, Request, TcpTransport, Transport, DEFAULT_PORT, DEFAULT_TIMEOUT,
    MAX_RESPONSE_BYTES,
};

use crate::backend::{Backend, BridgeError};
use crate::command::{Ack, Command, MenuNav};
use crate::menu::{Probe, ScreenDetail};
use crate::state::{Actions, GameState, Health, Screen};
use explorer::Explorer;
use wire::{parse, WireActions, WireCardPiles, WireCombat, WireCombatFlag, WireRun, WireRunFlag};

pub struct McpTestBackend<T: Transport> {
    transport: T,
    explorer: Option<Explorer>,
    /// Whether the bridge's `navigate_menu` knows `return_to_main_menu`;
    /// `None` until tried.
    return_to_main_menu: Option<bool>,
}

impl McpTestBackend<TcpTransport> {
    /// Connects per request to `STS2_BRIDGE_HOST` (default `127.0.0.1`):21337,
    /// with the GodotExplorer mod on the same host, port 27020.
    pub fn from_env() -> Self {
        let bridge = TcpTransport::from_env();
        let host = bridge.addr();
        let host = host.rsplit_once(':').map_or(host.as_str(), |(h, _)| h);
        let explorer = TcpTransport::new(host, EXPLORER_PORT);
        let addr = explorer.addr();
        let mut backend = Self::new(bridge);
        backend.explorer = Some(Explorer::new(explorer, addr));
        backend
    }
}

impl<T: Transport> McpTestBackend<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            explorer: None,
            return_to_main_menu: None,
        }
    }

    /// Adds the GodotExplorer client, which the main-menu check and leaving
    /// the game-over screen need. Without one, those fail with
    /// [`BridgeError::ExplorerUnavailable`].
    pub fn with_explorer(mut self, transport: impl Transport + 'static) -> Self {
        self.explorer = Some(Explorer::new(
            transport,
            format!("127.0.0.1:{EXPLORER_PORT}"),
        ));
        self
    }

    /// One raw bridge call, decoded. Unverified: nothing waits for the game
    /// to settle before or after it, and nothing checks its effect. For test
    /// positioning only; it must not be sent while a run is loading (see
    /// [`crate::wait_run_ready`]).
    pub fn request(&mut self, method: &str, params: Option<&Value>) -> Result<Value, BridgeError> {
        self.call(method, params)
    }

    fn explorer(&mut self) -> Result<&mut Explorer, BridgeError> {
        self.explorer
            .as_mut()
            .ok_or_else(|| BridgeError::ExplorerUnavailable {
                addr: format!("127.0.0.1:{EXPLORER_PORT}"),
                reason: "no explorer client configured".into(),
            })
    }

    fn navigate(&mut self, command: &Command, target: &str) -> Result<Ack, BridgeError> {
        self.send_raw(command, "navigate_menu", Some(json!({ "target": target })))
    }

    fn leave_game_over(&mut self, command: &Command) -> Result<Ack, BridgeError> {
        if self.return_to_main_menu != Some(false) {
            match self.navigate(command, "return_to_main_menu") {
                Err(BridgeError::ActionRefused { reason, .. })
                    if reason.starts_with("Unknown target") =>
                {
                    self.return_to_main_menu = Some(false);
                }
                other => {
                    self.return_to_main_menu = Some(true);
                    return other;
                }
            }
        }
        let refused = |reason: String| BridgeError::ActionRefused {
            action: command.to_string(),
            reason,
        };
        let explorer = self.explorer()?;
        let found = explorer.call("find_nodes", json!({ "pattern": "*GameOver*" }))?;
        let nodes = explorer::nodes(&found)
            .map_err(|e| refused(format!("explorer find_nodes reply: {e}")))?;
        let node = nodes
            .iter()
            .find(|n| n.name == "GameOverScreen")
            .ok_or_else(|| {
                let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
                refused(format!("no GameOverScreen node found (found: {names:?})"))
            })?;
        let reply = explorer.call(
            "call_method",
            json!({ "path": node.path, "method": "ReturnToMainMenu" }),
        )?;
        Ok(Ack::new(json!({ "explorer": reply, "path": node.path })))
    }

    /// Sends one write and turns the handler's rejection into `ActionRefused`.
    fn send_raw(
        &mut self,
        command: &Command,
        method: &str,
        params: Option<Value>,
    ) -> Result<Ack, BridgeError> {
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
            Command::Navigate(MenuNav::AbandonRun) => return self.navigate(command, "abandon"),
            Command::Navigate(MenuNav::Back) => return self.navigate(command, "back"),
            Command::Navigate(MenuNav::LeaveGameOver) => return self.leave_game_over(command),
        };
        self.send_raw(command, method, params)
    }

    fn probe(&mut self) -> Result<Probe, BridgeError> {
        let health = self.health()?;
        let mut probe = Probe {
            ok: health.ok,
            screen: health.screen,
            run_in_progress: health.run_in_progress,
            in_combat: health.in_combat,
            detail: None,
            main_menu_node: None,
        };
        if probe.screen.is_none() {
            return Ok(probe);
        }
        #[derive(Deserialize)]
        struct WireScreen {
            screen: Option<String>,
            screen_source: Option<String>,
            room_type: Option<String>,
            screen_context_type: Option<String>,
        }
        const SCREEN: &str = "get_screen";
        let value = self.call(SCREEN, None)?;
        let wire: WireScreen = parse(SCREEN, &value)?;
        probe.detail = Some(ScreenDetail {
            screen: wire.screen.map(Screen),
            source: wire.screen_source,
            room_type: wire.room_type,
            context: wire.screen_context_type,
        });
        if probe.screen.as_ref().map(Screen::as_str) == Some("MAIN_MENU")
            && probe.run_in_progress != Some(true)
        {
            let explorer = self.explorer()?;
            let found = explorer.call("find_nodes", json!({ "pattern": "MainMenu" }))?;
            let nodes = explorer::nodes(&found).map_err(|e| BridgeError::Protocol {
                method: "tools/call find_nodes".into(),
                kind: crate::backend::ProtocolError::InvalidJson(e),
            })?;
            probe.main_menu_node = Some(nodes.iter().any(|n| n.name == "MainMenu"));
        }
        Ok(probe)
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
