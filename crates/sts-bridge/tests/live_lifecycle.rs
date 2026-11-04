//! Lifecycle tests against the live game. Opt-in and serial:
//!
//! ```text
//! cargo test -p sts-bridge --test live_lifecycle -- --ignored --test-threads=1
//! ```
//!
//! Needs Steam running and logged in (the game's launch options apply), and
//! the MCPTest and GodotExplorer mods installed. The game need not be
//! running: each test starts it. Some tests kill it (SIGKILL) or freeze it
//! (SIGSTOP) and check that it is recovered. An unreachable bridge fails the
//! test, never skips it.
//!
//! Positioning sends raw console commands, each only after
//! `wait_run_ready()`: a room-changing command sent while a run loads
//! crashes the game.
#![cfg(unix)]

mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use common::hold_game;
use serde_json::{json, Value};
use sts_bridge::mcptest::{TcpTransport, Transport, EXPLORER_PORT};
use sts_bridge::{
    classify, poll_until, Backend, BridgeError, Game, GameStatus, Launcher, McpTestBackend, Phase,
    Probe, SteamLauncher,
};

/// Every request sent, with its raw reply, in order.
type Log = Rc<RefCell<Vec<(String, Option<Value>, String)>>>;

struct Logged {
    inner: TcpTransport,
    log: Log,
}

impl Transport for Logged {
    fn send(&mut self, method: &str, params: Option<&Value>) -> Result<Vec<u8>, BridgeError> {
        let reply = self.inner.send(method, params);
        let text = match &reply {
            Ok(raw) => String::from_utf8_lossy(raw).into_owned(),
            Err(e) => format!("error: {e}"),
        };
        self.log
            .borrow_mut()
            .push((method.to_string(), params.cloned(), text));
        reply
    }
}

type Live = Game<McpTestBackend<Logged>, SteamLauncher>;

fn host() -> String {
    std::env::var("STS2_BRIDGE_HOST")
        .ok()
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "127.0.0.1".into())
}

/// The game, started (launched through Steam if it is not running).
fn live_game() -> (Live, Log) {
    let log: Log = Rc::default();
    let bridge = Logged {
        inner: TcpTransport::from_env(),
        log: log.clone(),
    };
    let explorer = Logged {
        inner: TcpTransport::new(host(), EXPLORER_PORT),
        log: log.clone(),
    };
    let backend = McpTestBackend::new(bridge).with_explorer(explorer);
    let mut game = Game::new(backend, SteamLauncher::from_env());
    game.start()
        .unwrap_or_else(|e| panic!("could not start the game: {e}"));
    (game, log)
}

fn is_write(method: &str, params: &Option<Value>) -> bool {
    let tool = params.as_ref().and_then(|p| p["name"].as_str());
    [
        "console",
        "navigate_menu",
        "start_run",
        "execute_action",
        "play_card",
        "end_turn",
    ]
    .contains(&method)
        || (method == "tools/call" && tool == Some("call_method"))
}

fn writes_since(log: &Log, from: usize) -> Vec<String> {
    log.borrow()[from..]
        .iter()
        .filter(|(m, p, _)| is_write(m, p))
        .map(|(m, p, _)| format!("{m} {}", p.clone().unwrap_or(Value::Null)))
        .collect()
}

fn verified(p: &Probe) {
    assert_eq!(classify(p), Phase::MainMenu, "{p:?}");
    assert_eq!(p.main_menu_node, Some(true), "{p:?}");
}

fn ensure_main_menu(game: &mut Live, from: &str) -> Probe {
    let p = game
        .ensure_main_menu()
        .unwrap_or_else(|e| panic!("ensure_main_menu from {from}: {e}"));
    verified(&p);
    assert!(
        game.recoveries().is_empty(),
        "ensure_main_menu from {from} needed a restart: {:?}",
        game.recoveries()
    );
    p
}

fn request(game: &mut Live, method: &str, params: Value) -> Value {
    game.backend_mut()
        .request(method, Some(&params))
        .unwrap_or_else(|e| panic!("{method} {params}: {e}"))
}

/// A console line, sent only on a stable in-run screen.
fn console(game: &mut Live, line: &str) {
    game.wait_run_ready()
        .unwrap_or_else(|e| panic!("before `{line}`: {e}"));
    let reply = request(game, "console", json!({ "command": line }));
    assert_eq!(reply["success"], Value::Bool(true), "`{line}`: {reply}");
}

/// Waits, sending nothing, for `phase` on `screen` (any screen if `None`).
fn wait_for(game: &mut Live, phase: Phase, screen: Option<&str>) -> Probe {
    let policy = game
        .timeouts()
        .probe
        .clone()
        .with_timeout(game.timeouts().run_load);
    let unmet = format!("{phase:?} on {screen:?}");
    poll_until(
        &policy,
        &unmet,
        || game.backend_mut().probe(),
        |p| {
            classify(p) == phase
                && screen.is_none_or(|s| p.screen.as_ref().is_some_and(|x| x.as_str() == s))
        },
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

/// A fresh seeded Ironclad run, on its first room (the opening event).
fn new_run(game: &mut Live) -> Probe {
    ensure_main_menu(game, "before start_run");
    request(
        game,
        "start_run",
        json!({"character": "Ironclad", "seed": "HOL106LIVE", "ascension": 0}),
    );
    game.wait_run_ready()
        .unwrap_or_else(|e| panic!("the run never became ready: {e}"))
}

#[derive(Debug, Clone, Copy)]
enum At {
    MainMenu,
    Event,
    Map,
    Combat,
    GameOver,
}

fn position(game: &mut Live, at: At) {
    match at {
        At::MainMenu => {
            ensure_main_menu(game, "positioning");
        }
        At::Event => {
            let p = new_run(game);
            assert_eq!(
                p.screen.unwrap().as_str(),
                "EVENT",
                "the seed opens on an event"
            );
        }
        At::Map => {
            new_run(game);
            console(game, "room Map");
            wait_for(game, Phase::InRun, Some("MAP"));
        }
        At::Combat => {
            new_run(game);
            console(game, "fight NIBBITS_WEAK");
            wait_for(game, Phase::InRun, Some("COMBAT_PLAYER_TURN"));
        }
        At::GameOver => {
            new_run(game);
            request(game, "navigate_menu", json!({"target": "abandon"}));
            wait_for(game, Phase::GameOver, None);
        }
    }
}

/// Not a check: leaves the game in an Ironclad combat on the player's turn,
/// the position the `live_contract` `act_*` tests need.
#[test]
#[ignore = "live: positions the game for live_contract"]
fn position_ironclad_combat() {
    let _game = hold_game();
    let (mut game, _) = live_game();
    position(&mut game, At::Combat);
}

fn ten_of_ten(at: At) {
    let _game = hold_game();
    let (mut game, log) = live_game();
    for run in 1..=10 {
        position(&mut game, at);
        let from = log.borrow().len();
        ensure_main_menu(&mut game, &format!("{at:?}, run {run}"));
        if matches!(at, At::MainMenu) {
            assert_eq!(
                writes_since(&log, from),
                Vec::<String>::new(),
                "run {run}: sent a write from the main menu"
            );
        }
    }
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; stops and relaunches the game"]
fn start_from_not_running() {
    let _game = hold_game();
    let (mut game, _) = live_game();
    game.stop().unwrap_or_else(|e| panic!("stop: {e}"));
    assert_eq!(game.status().unwrap(), GameStatus::NotRunning);
    let p = game.start().unwrap_or_else(|e| panic!("start: {e}"));
    verified(&p);
    assert!(game.launcher_mut().pid().is_some());
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer"]
fn start_at_the_main_menu_does_not_relaunch_or_send() {
    let _game = hold_game();
    let (mut game, log) = live_game();
    ensure_main_menu(&mut game, "setup");
    let pid = game.launcher_mut().pid();
    let from = log.borrow().len();
    game.start().unwrap_or_else(|e| panic!("start: {e}"));
    assert_eq!(game.launcher_mut().pid(), pid);
    assert_eq!(writes_since(&log, from), Vec::<String>::new());
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer"]
fn ensure_main_menu_from_the_main_menu_10_of_10() {
    ten_of_ten(At::MainMenu);
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; starts runs"]
fn ensure_main_menu_from_an_event_10_of_10() {
    ten_of_ten(At::Event);
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; starts runs"]
fn ensure_main_menu_from_the_map_10_of_10() {
    ten_of_ten(At::Map);
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; starts runs"]
fn ensure_main_menu_from_combat_10_of_10() {
    ten_of_ten(At::Combat);
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; starts runs"]
fn ensure_main_menu_from_the_game_over_screen_10_of_10() {
    ten_of_ten(At::GameOver);
}

/// Right after `start_run` the run is loading: `ensure_main_menu` must wait
/// it out, sending nothing, and abandon from a settled in-run screen.
#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; starts runs"]
fn ensure_main_menu_waits_out_a_loading_run_5_of_5() {
    let _game = hold_game();
    let (mut game, log) = live_game();
    for run in 1..=5 {
        ensure_main_menu(&mut game, "setup");
        request(
            &mut game,
            "start_run",
            json!({"character": "Ironclad", "seed": "HOL106LIVE", "ascension": 0}),
        );
        let from = log.borrow().len();
        ensure_main_menu(&mut game, &format!("a loading run, run {run}"));
        let entries = log.borrow()[from..].to_vec();
        let first_write = entries
            .iter()
            .position(|(m, p, _)| is_write(m, p))
            .expect("ensure_main_menu sent no abandon");
        let loading_pings = entries[..first_write]
            .iter()
            .filter(|(m, _, reply)| {
                m == "ping"
                    && reply.contains("\"screen\":\"MAIN_MENU\"")
                    && reply.contains("\"run_in_progress\":true")
            })
            .count();
        assert!(
            loading_pings >= 1,
            "run {run}: no loading probe seen before the first write"
        );
        let (m, p, _) = &entries[first_write];
        assert_eq!(
            (m.as_str(), p.as_ref().map(|p| p["target"].clone())),
            ("navigate_menu", Some(json!("abandon"))),
            "run {run}: the first write must be the abandon"
        );
        // The screen just before the abandon was a settled in-run one.
        let last_ping = entries[..first_write]
            .iter()
            .rev()
            .find(|(m, _, _)| m == "ping")
            .map(|(_, _, r)| r.clone())
            .unwrap();
        assert!(
            !last_ping.contains("\"screen\":\"MAIN_MENU\""),
            "run {run}: abandoned while loading: {last_ping}"
        );
    }
}

fn signal(pid: u32, sig: &str) {
    let out = std::process::Command::new("kill")
        .args([sig, &pid.to_string()])
        .output()
        .unwrap();
    assert!(out.status.success(), "kill {sig} {pid}: {out:?}");
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; freezes and relaunches the game"]
fn a_frozen_game_is_wedged_and_recovered() {
    let _game = hold_game();
    let (mut game, _) = live_game();
    ensure_main_menu(&mut game, "setup");
    let pid = game.launcher_mut().pid().expect("the game is running");
    signal(pid, "-STOP");
    match game.status() {
        Ok(GameStatus::Wedged { .. }) => {}
        other => {
            signal(pid, "-CONT");
            panic!("expected Wedged, got {other:?}")
        }
    }
    let p = game
        .ensure_main_menu()
        .unwrap_or_else(|e| panic!("recovery from SIGSTOP: {e}"));
    verified(&p);
    assert_eq!(game.recoveries().len(), 1);
    assert!(game.recoveries()[0].cause.contains("wedged"));
    assert_ne!(game.launcher_mut().pid(), Some(pid));
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; kills and relaunches the game"]
fn a_killed_game_is_crashed_and_recovered() {
    let _game = hold_game();
    let (mut game, _) = live_game();
    ensure_main_menu(&mut game, "setup");
    let pid = game.launcher_mut().pid().expect("the game is running");
    game.status().unwrap();
    signal(pid, "-KILL");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while game.launcher_mut().pid().is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
    }
    match game.status().unwrap() {
        GameStatus::Crashed { pid: seen, .. } => assert_eq!(seen, Some(pid)),
        other => panic!("expected Crashed, got {other:?}"),
    }
    let p = game
        .ensure_main_menu()
        .unwrap_or_else(|e| panic!("recovery from SIGKILL: {e}"));
    verified(&p);
    assert_eq!(game.recoveries().len(), 1);
    assert!(game.recoveries()[0].cause.contains("crashed"));
}
