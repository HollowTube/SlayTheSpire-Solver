//! Shared test helpers. Not every test binary uses every helper.
#![allow(dead_code)]

use std::cell::{Cell, RefCell, RefMut};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use serde_json::{json, Value};
use sts_bridge::mcptest::{ReplayTransport, Request, TcpTransport, Transport};
use sts_bridge::{Backend, BridgeError, Launcher, McpTestBackend};

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
    pub player_state: Value,
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
            player_state: result("combat", "get_player_state"),
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
        if self.run["in_progress"] == Value::Bool(true) {
            t.push("get_player_state", wrap(&self.player_state));
        }
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

// ---- Lifecycle fakes: a transport that can fail, and a launcher ----

/// What a [`FaultTransport`] does with a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Delegates to the replay.
    None,
    /// The connection is refused: no game.
    Refused,
    /// The connection is accepted but never answered: a wedged game.
    Timeout,
    /// The connection is reset: a process that is dying.
    Reset,
}

type Matcher = Box<dyn Fn(&Request) -> bool>;
type Action = Box<dyn FnMut(&mut ReplayTransport)>;

/// Wraps a shared [`ReplayTransport`] (every reply comes from it) and fails
/// requests while its fault is set. Clones share the replay, the fault and
/// the triggers, so a bridge and an explorer client can log into one place.
#[derive(Clone)]
pub struct FaultTransport {
    replay: Rc<RefCell<ReplayTransport>>,
    fault: Rc<Cell<Fault>>,
    triggers: Rc<RefCell<Vec<(Matcher, Action)>>>,
}

impl FaultTransport {
    pub fn new(replay: ReplayTransport) -> Self {
        Self {
            replay: Rc::new(RefCell::new(replay)),
            fault: Rc::new(Cell::new(Fault::None)),
            triggers: Rc::default(),
        }
    }

    pub fn set_fault(&self, fault: Fault) {
        self.fault.set(fault);
    }

    pub fn replay(&self) -> RefMut<'_, ReplayTransport> {
        self.replay.borrow_mut()
    }

    pub fn requests(&self) -> Vec<Request> {
        self.replay.borrow().requests().to_vec()
    }

    pub fn methods(&self) -> Vec<String> {
        self.requests().into_iter().map(|r| r.method).collect()
    }

    /// Once a request matching `when` is answered, runs `then` on the replay
    /// (e.g. to switch to the screen the request leads to). Fires once.
    pub fn on(
        &self,
        when: impl Fn(&Request) -> bool + 'static,
        then: impl FnMut(&mut ReplayTransport) + 'static,
    ) {
        self.triggers
            .borrow_mut()
            .push((Box::new(when), Box::new(then)));
    }
}

impl Transport for FaultTransport {
    fn send(&mut self, method: &str, params: Option<&Value>) -> Result<Vec<u8>, BridgeError> {
        match self.fault.get() {
            Fault::Refused => {
                return Err(BridgeError::Refused {
                    addr: "127.0.0.1:21337".into(),
                })
            }
            Fault::Timeout => {
                return Err(BridgeError::Timeout {
                    method: method.into(),
                    after: Duration::from_secs(12),
                })
            }
            Fault::Reset => {
                return Err(BridgeError::Io {
                    method: method.into(),
                    source: std::io::Error::from(std::io::ErrorKind::ConnectionReset),
                })
            }
            Fault::None => {}
        }
        let reply = self.replay.borrow_mut().send(method, params);
        let request = Request {
            method: method.into(),
            params: params.cloned(),
        };
        let fired = {
            let mut triggers = self.triggers.borrow_mut();
            let i = triggers.iter().position(|(when, _)| when(&request));
            i.map(|i| triggers.remove(i))
        };
        if let Some((_, mut then)) = fired {
            then(&mut self.replay.borrow_mut());
        }
        reply
    }
}

/// A request to the explorer tool `tool`.
pub fn is_tool(r: &Request, tool: &str) -> bool {
    r.method == "tools/call" && r.params.as_ref().and_then(|p| p["name"].as_str()) == Some(tool)
}

/// A `navigate_menu` to `target`.
pub fn is_nav(r: &Request, target: &str) -> bool {
    r.method == "navigate_menu"
        && r.params.as_ref().and_then(|p| p["target"].as_str()) == Some(target)
}

/// Whether `r` is a write: a bridge command or an explorer `call_method`.
pub fn is_write(r: &Request) -> bool {
    is_tool(r, "call_method")
        || [
            "console",
            "navigate_menu",
            "start_run",
            "execute_action",
            "play_card",
            "end_turn",
        ]
        .contains(&r.method.as_str())
}

/// One screen as the bridge and the explorer report it, in the shapes
/// recorded from the live game (`ping`, `get_screen`, `find_nodes`).
#[derive(Debug, Clone)]
pub struct Scene {
    pub screen: Option<&'static str>,
    pub run: bool,
    pub source: &'static str,
    pub context: Option<&'static str>,
    pub room: Option<&'static str>,
    /// Whether the explorer finds the `MainMenu` node.
    pub menu_node: bool,
}

impl Scene {
    fn at(screen: &'static str, run: bool, source: &'static str) -> Self {
        Self {
            screen: Some(screen),
            run,
            source,
            context: None,
            room: None,
            menu_node: false,
        }
    }

    pub fn main_menu() -> Self {
        Self {
            context: Some("NMainMenu"),
            menu_node: true,
            ..Self::at("MAIN_MENU", false, "active_screen_context")
        }
    }

    /// A new run loading: recorded for ~50 s after `start_run`.
    pub fn run_loading() -> Self {
        Self {
            context: Some("NMainMenu"),
            menu_node: true,
            ..Self::at("MAIN_MENU", true, "active_screen_context")
        }
    }

    /// The run started, its first room not entered yet.
    pub fn no_room_yet() -> Self {
        Self::at("MAP", true, "run_state")
    }

    pub fn event() -> Self {
        Self {
            context: Some("NEventRoom"),
            room: Some("EventRoom"),
            ..Self::at("EVENT", true, "active_screen_context")
        }
    }

    pub fn map() -> Self {
        Self {
            context: Some("NMapScreen"),
            ..Self::at("MAP", true, "active_screen_context")
        }
    }

    pub fn combat() -> Self {
        Self {
            room: Some("CombatRoom"),
            ..Self::at("COMBAT_PLAYER_TURN", true, "combat_manager")
        }
    }

    /// Recorded after `navigate_menu abandon`: an overlay, run still in progress.
    pub fn game_over() -> Self {
        Self {
            context: Some("NGameOverScreen"),
            ..Self::at("OVERLAY", true, "overlay_stack")
        }
    }

    pub fn screen(screen: &'static str, run: bool) -> Self {
        Self::at(screen, run, "active_screen_context")
    }

    /// Ping answered without a screen (its main-thread read failed).
    pub fn screenless() -> Self {
        Self {
            screen: None,
            ..Self::at("", false, "")
        }
    }

    pub fn ping(&self) -> String {
        let mut r = json!({"status": "ok", "mod": "MCPTest", "version": "2.0.0"});
        if let Some(s) = self.screen {
            r["screen"] = json!(s);
            r["run_in_progress"] = json!(self.run);
            r["in_combat"] = json!(s.starts_with("COMBAT"));
        }
        envelope(r)
    }

    pub fn get_screen(&self) -> String {
        let mut r = json!({"screen": self.screen, "screen_source": self.source});
        if let Some(c) = self.context {
            r["screen_context_type"] = json!(c);
        }
        if let Some(room) = self.room {
            r["room_type"] = json!(room);
        }
        envelope(r)
    }

    /// Makes this the screen every later read sees.
    pub fn show(&self, t: &mut ReplayTransport) {
        t.insert("ping", self.ping());
        t.insert("get_screen", self.get_screen());
        self.show_state(t);
    }

    /// Everything [`Self::show`] sets except the probe's own replies, for a
    /// script that queues those with [`Self::push_probes`].
    pub fn show_state(&self, t: &mut ReplayTransport) {
        if self.screen == Some("MAIN_MENU") && !self.run {
            t.insert(
                "tools/call",
                find_nodes(if self.menu_node { &["MainMenu"] } else { &[] }),
            );
            let mut s = WireState::combat();
            s.actions = json!({"screen": "MAIN_MENU", "actions": []});
            s.run = json!({"in_progress": false, "screen": "MAIN_MENU"});
            t.insert("get_available_actions", envelope(s.actions));
            t.insert("get_run_state", envelope(s.run));
        } else {
            let screen = self.screen.unwrap_or("UNKNOWN");
            let s = WireState::out_of_combat(screen);
            t.insert("get_available_actions", envelope(s.actions));
            t.insert("get_run_state", envelope(s.run));
            t.insert("get_player_state", envelope(s.player_state));
            t.insert("get_combat_state", envelope(s.combat));
        }
        if self.run {
            t.insert(
                "navigate_menu",
                envelope(json!({"success": true, "target": "abandon"})),
            );
        }
        if self.context == Some("NGameOverScreen") {
            t.insert("navigate_menu", UNKNOWN_TARGET);
            t.insert("tools/call", find_nodes(&["GameOverScreen"]));
            t.push("tools/call", tool_text("ReturnToMainMenu() = "));
        }
    }

    /// Queues one probe's reads (`ping`, `get_screen`) of this screen, `n` times.
    pub fn push_probes(&self, t: &mut ReplayTransport, n: usize) {
        for _ in 0..n {
            t.push("ping", self.ping());
            if self.screen.is_some() {
                t.push("get_screen", self.get_screen());
            }
        }
    }
}

pub fn envelope(result: Value) -> String {
    json!({"result": result, "id": 1}).to_string()
}

/// The current mod's reply to `navigate_menu return_to_main_menu`.
pub const UNKNOWN_TARGET: &str = r#"{"result":{"error":"Unknown target: return_to_main_menu. Valid: continue, compendium, card_library, settings, profile, timeline, multiplayer, new_run, abandon, back, proceed"},"id":1}"#;

/// An explorer tool reply with `text` as its content.
pub fn tool_text(text: &str) -> String {
    json!({"jsonrpc": "2.0", "id": 1, "result": {"content": [{"type": "text", "text": text}]}})
        .to_string()
}

/// An explorer `find_nodes` reply listing nodes named `names`.
pub fn find_nodes(names: &[&str]) -> String {
    let results: Vec<Value> = names
        .iter()
        .map(|n| {
            let path = match *n {
                "GameOverScreen" => {
                    "/root/Game/RootSceneContainer/Run/GlobalUi/OverlayScreensContainer/GameOverScreen"
                }
                _ => "/root/Game/RootSceneContainer/MainMenu",
            };
            json!({"name": n, "type": "Control", "path": path})
        })
        .collect();
    tool_text(&json!({"count": results.len(), "results": results}).to_string())
}

/// A fake game process: launching brings it up on `on_launch`'s scene (or
/// makes it die), killing takes it down.
#[derive(Clone)]
pub struct FakeLauncher {
    pub transport: FaultTransport,
    state: Rc<RefCell<FakeProcess>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    /// Comes up at the main menu.
    Up,
    /// Starts, then dies before the bridge answers.
    Dies,
}

#[derive(Default)]
struct FakeProcess {
    pid: Option<u32>,
    next_pid: u32,
    launches: usize,
    kills: usize,
    outcomes: VecDeque<Launch>,
    /// `pid()` calls left before a dying process disappears.
    dies_in: Option<usize>,
    /// `pid()` calls left before a booting process's bridge answers.
    up_in: Option<usize>,
}

impl FakeLauncher {
    /// `pid` is the running process, if any.
    pub fn new(transport: FaultTransport, pid: Option<u32>) -> Self {
        Self {
            transport,
            state: Rc::new(RefCell::new(FakeProcess {
                pid,
                next_pid: 1000,
                ..FakeProcess::default()
            })),
        }
    }

    /// The outcomes of the next launches, in order; `Up` after these.
    pub fn launches_will(&self, outcomes: &[Launch]) {
        self.state.borrow_mut().outcomes = outcomes.iter().copied().collect();
    }

    pub fn launches(&self) -> usize {
        self.state.borrow().launches
    }

    pub fn kills(&self) -> usize {
        self.state.borrow().kills
    }

    /// The process is up but its bridge refuses for the next `n` `pid()`
    /// calls, then answers: a game still booting.
    pub fn booting(&self, n: usize) {
        self.state.borrow_mut().up_in = Some(n);
        self.transport.set_fault(Fault::Refused);
    }

    /// The bridge refuses at once, but the process lingers for `n` more
    /// `pid()` calls: a crash whose core dump is still being written.
    pub fn crash_lingering(&self, n: usize) {
        self.state.borrow_mut().dies_in = Some(n);
        self.transport.set_fault(Fault::Refused);
    }

    /// The process vanishes and the bridge refuses: a crash.
    pub fn crash(&self) {
        self.state.borrow_mut().pid = None;
        self.transport.set_fault(Fault::Refused);
    }
}

impl Launcher for FakeLauncher {
    fn launch(&mut self) -> Result<(), BridgeError> {
        let mut s = self.state.borrow_mut();
        s.launches += 1;
        s.next_pid += 1;
        s.pid = Some(s.next_pid);
        match s.outcomes.pop_front().unwrap_or(Launch::Up) {
            Launch::Up => {
                Scene::main_menu().show(&mut self.transport.replay());
                self.transport.set_fault(Fault::None);
            }
            Launch::Dies => {
                s.dies_in = Some(2);
                self.transport.set_fault(Fault::Refused);
            }
        }
        Ok(())
    }

    fn pid(&mut self) -> Option<u32> {
        let mut s = self.state.borrow_mut();
        if let Some(n) = s.dies_in {
            if n == 0 {
                s.pid = None;
                s.dies_in = None;
            } else {
                s.dies_in = Some(n - 1);
            }
        }
        if let Some(n) = s.up_in {
            if n == 0 {
                s.up_in = None;
                self.transport.set_fault(Fault::None);
            } else {
                s.up_in = Some(n - 1);
            }
        }
        s.pid
    }

    fn kill(&mut self, pid: u32, _grace: Duration) -> Result<(), BridgeError> {
        let mut s = self.state.borrow_mut();
        assert_eq!(s.pid, Some(pid), "killed a process that is not running");
        s.kills += 1;
        s.pid = None;
        self.transport.set_fault(Fault::Refused);
        Ok(())
    }
}

/// A backend over `t` with the explorer on the same transport.
pub fn fault_backend(t: &FaultTransport) -> McpTestBackend<FaultTransport> {
    McpTestBackend::new(t.clone()).with_explorer(t.clone())
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
