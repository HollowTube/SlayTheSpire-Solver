//! Shared test helpers. Not every test binary uses every helper.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use serde_json::Value;
use sts_bridge::mcptest::{ReplayTransport, TcpTransport};
use sts_bridge::{Backend, McpTestBackend};

pub fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mcptest")
        .join(name)
}

/// One game state as the four bridge replies `Backend::state` reads, starting
/// from the `combat` fixture. Edit the `result` values, then [`Self::push`].
#[derive(Clone)]
pub struct WireState {
    pub actions: Value,
    pub run: Value,
    pub combat: Value,
    pub piles: Value,
}

fn result(dir: &str, method: &str) -> Value {
    let raw = std::fs::read(fixture_dir(dir).join(format!("{method}.json"))).unwrap();
    let envelope: Value = serde_json::from_slice(&raw).unwrap();
    envelope["result"].clone()
}

impl WireState {
    pub fn combat() -> Self {
        Self {
            actions: result("combat", "get_available_actions"),
            run: result("combat", "get_run_state"),
            combat: result("combat", "get_combat_state"),
            piles: result("combat", "get_card_piles"),
        }
    }

    /// In a run, out of combat, on `screen` with no actions on offer.
    pub fn out_of_combat(screen: &str) -> Self {
        let mut s = Self::combat();
        s.actions = serde_json::json!({"screen": screen, "actions": []});
        s.combat = serde_json::json!({"in_combat": false, "screen": screen});
        s
    }

    pub fn screen(mut self, screen: &str) -> Self {
        self.actions["screen"] = Value::from(screen);
        self
    }

    pub fn edit(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }

    pub fn player(&mut self) -> &mut Value {
        &mut self.combat["players"][0]
    }

    pub fn hand(&mut self) -> &mut Vec<Value> {
        self.combat["players"][0]["hand"].as_array_mut().unwrap()
    }

    pub fn enemy(&mut self, i: usize) -> &mut Value {
        &mut self.combat["enemies"][i]
    }

    /// `pile` is `draw_pile`, `discard_pile` or `exhaust_pile`.
    pub fn pile(&mut self, pile: &str) -> &mut Vec<Value> {
        self.piles[pile]["cards"].as_array_mut().unwrap()
    }

    /// Queues the replies `Backend::state` reads for this state, in its order.
    pub fn push(&self, t: &mut ReplayTransport) {
        let wrap = |v: &Value| serde_json::json!({ "result": v, "id": 1 }).to_string();
        t.push("get_available_actions", wrap(&self.actions));
        t.push("get_run_state", wrap(&self.run));
        let screen = self.actions["screen"].as_str().unwrap_or_default();
        if self.run["in_progress"] == Value::Bool(true) && !screen.contains("LOADING") {
            t.push("get_combat_state", wrap(&self.combat));
            if self.combat["in_combat"] == Value::Bool(true) {
                t.push("get_card_piles", wrap(&self.piles));
            }
        }
    }

    /// Queues this state `n` times.
    pub fn push_n(&self, t: &mut ReplayTransport, n: usize) {
        for _ in 0..n {
            self.push(t);
        }
    }
}

/// The hand card at `index` of the `combat` fixture, moved to a pile card.
pub fn as_pile_card(hand_card: &Value) -> Value {
    serde_json::json!({
        "index": 0,
        "name": hand_card["name"],
        "type": hand_card["type"],
        "energy_cost": hand_card["energy_cost"],
        "upgraded": hand_card["upgraded"],
    })
}

static GAME: Mutex<()> = Mutex::new(());

/// Serializes live tests: they share one game.
pub fn hold_game() -> MutexGuard<'static, ()> {
    GAME.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A backend for the live bridge; panics (never skips) if it is unreachable.
pub fn live_backend() -> McpTestBackend<TcpTransport> {
    let mut backend = McpTestBackend::from_env();
    let addr = backend.transport().addr();
    match backend.health() {
        Ok(h) if h.ok => backend,
        Ok(h) => panic!("bridge at {addr} answered ping but is not ok: {h:?}"),
        Err(e) => panic!(
            "bridge unavailable at {addr}: {e}\n\
             Start STS2 with the MCPTest mod (set STS2_BRIDGE_HOST if the game is on another host)."
        ),
    }
}
