//! Loading classification, menu navigation, and game lifecycle over a
//! scripted bridge and a fake launcher; no game needed.

mod common;

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use common::{
    fault_backend, is_nav, is_tool, is_write, FakeLauncher, Fault, FaultTransport, Launch, Scene,
};
use serde_json::json;
use sts_bridge::mcptest::{ReplayTransport, TcpTransport};
use sts_bridge::{
    classify, ensure_main_menu, wait_run_ready, wait_settled, BridgeError, Command, Game,
    GameStatus, McpTestBackend, MenuNav, Phase, Probe, Screen, ScreenDetail, Session, SettlePolicy,
    Timeouts, WedgeReason,
};

fn timeouts() -> Timeouts {
    Timeouts::uniform(Duration::from_millis(200), Duration::from_millis(1))
}

fn transport(scene: Scene) -> FaultTransport {
    let mut replay = ReplayTransport::new();
    scene.show(&mut replay);
    FaultTransport::new(replay)
}

type Fake = Game<McpTestBackend<FaultTransport>, FakeLauncher>;

/// A running game (pid 7) on `scene`.
fn game(scene: Scene) -> (Fake, FaultTransport, FakeLauncher) {
    let t = transport(scene);
    let launcher = FakeLauncher::new(t.clone(), Some(7));
    let game = Game::new(fault_backend(&t), launcher.clone()).with_timeouts(timeouts());
    (game, t, launcher)
}

fn writes(t: &FaultTransport) -> Vec<String> {
    t.requests()
        .iter()
        .filter(|r| is_write(r))
        .map(|r| match r.params.as_ref() {
            Some(p) if r.method == "navigate_menu" => format!("navigate_menu {}", p["target"]),
            Some(p) if r.method == "tools/call" => format!("tools/call {}", p["name"]),
            _ => r.method.clone(),
        })
        .collect()
}

fn probe(screen: &str, run: bool, source: &str) -> Probe {
    Probe {
        ok: true,
        screen: Some(Screen(screen.into())),
        run_in_progress: Some(run),
        in_combat: Some(false),
        detail: Some(ScreenDetail {
            screen: Some(Screen(screen.into())),
            source: Some(source.into()),
            ..ScreenDetail::default()
        }),
        main_menu_node: None,
    }
}

fn verified_main_menu(p: &Probe) {
    assert_eq!(classify(p), Phase::MainMenu, "{p:?}");
    assert_eq!(p.main_menu_node, Some(true), "{p:?}");
}

// ---- classify: the current mod's view of a loading run ----

#[test]
fn the_main_menu_with_a_run_in_progress_is_loading() {
    let p = probe("MAIN_MENU", true, "active_screen_context");
    assert_eq!(classify(&p), Phase::Loading);
}

#[test]
fn a_map_derived_from_the_run_state_is_loading() {
    assert_eq!(classify(&probe("MAP", true, "run_state")), Phase::Loading);
    assert_eq!(
        classify(&probe("MAP", true, "active_screen_context")),
        Phase::InRun
    );
}

#[test]
fn a_loading_screen_is_loading() {
    assert_eq!(
        classify(&probe("LOADING", true, "run_state")),
        Phase::Loading
    );
    assert_eq!(
        classify(&probe("COMBAT_LOADING", true, "x")),
        Phase::Loading
    );
}

#[test]
fn only_the_main_menu_without_a_run_is_the_main_menu() {
    assert_eq!(
        classify(&probe("MAIN_MENU", false, "active_screen_context")),
        Phase::MainMenu
    );
    assert_ne!(
        classify(&probe("MAIN_MENU", true, "active_screen_context")),
        Phase::MainMenu
    );
}

#[test]
fn the_phases_of_the_other_screens() {
    let mut over = probe("OVERLAY", true, "overlay_stack");
    over.detail.as_mut().unwrap().context = Some("NGameOverScreen".into());
    assert_eq!(classify(&over), Phase::GameOver);
    assert_eq!(classify(&probe("GAME_OVER", true, "x")), Phase::GameOver);
    assert_eq!(classify(&probe("EVENT", true, "x")), Phase::InRun);
    assert_eq!(
        classify(&probe("COMBAT_PLAYER_TURN", true, "x")),
        Phase::InRun
    );
    assert_eq!(
        classify(&probe("COMBAT_ENEMY_TURN", true, "x")),
        Phase::Transition
    );
    assert_eq!(
        classify(&probe("MENU_NCombatRoom", true, "x")),
        Phase::Transition
    );
    assert_eq!(
        classify(&probe("CHARACTER_SELECT", false, "x")),
        Phase::MenuSubscreen
    );
    assert_eq!(
        classify(&probe("MENU_NLogoAnimation", false, "x")),
        Phase::Transition
    );
    assert_eq!(classify(&probe("SOMETHING_NEW", true, "x")), Phase::Unknown);
    let screenless = Probe {
        screen: None,
        detail: None,
        ..probe("MAP", true, "x")
    };
    assert_eq!(classify(&screenless), Phase::Unreadable);
}

#[test]
fn ping_and_get_screen_disagreeing_is_a_transition() {
    let mut p = probe("COMBAT_PLAYER_TURN", true, "x");
    p.detail.as_mut().unwrap().screen = Some(Screen("OVERLAY".into()));
    assert_eq!(classify(&p), Phase::Transition);
}

// ---- the crash trigger, offline: a new run loading ----

/// `start_run` as recorded: `MAIN_MENU` with the run in progress on 20+
/// identical reads, then `MAP` from the run state, then the opening `EVENT`.
fn run_start() -> FaultTransport {
    let mut replay = ReplayTransport::new();
    Scene::run_loading().push_probes(&mut replay, 24);
    Scene::no_room_yet().push_probes(&mut replay, 4);
    Scene::event().push_probes(&mut replay, 1);
    replay.insert(
        "navigate_menu",
        common::envelope(json!({"success": true, "target": "abandon"})),
    );
    let t = FaultTransport::new(replay);
    t.on(|r| is_nav(r, "abandon"), |r| Scene::game_over().show(r));
    t.on(
        |r| is_tool(r, "call_method"),
        |r| Scene::main_menu().show(r),
    );
    t
}

/// How many probes had been read when the first write went out.
fn pings_before_first_write(t: &FaultTransport) -> usize {
    t.requests()
        .iter()
        .take_while(|r| !is_write(r))
        .filter(|r| r.method == "ping")
        .count()
}

#[test]
fn wait_run_ready_returns_only_on_the_stable_event() {
    let t = run_start();
    let mut backend = fault_backend(&t);
    let p = wait_run_ready(&mut backend, &timeouts()).unwrap();
    assert_eq!(p.screen.as_ref().unwrap().as_str(), "EVENT");
    assert_eq!(classify(&p), Phase::InRun);
    let pings = t.methods().iter().filter(|m| *m == "ping").count();
    assert!(pings >= 24 + 4 + 3, "returned after {pings} probes");
    assert!(writes(&t).is_empty(), "{:?}", writes(&t));
}

#[test]
fn ensure_main_menu_sends_nothing_until_the_event_held() {
    let t = run_start();
    let mut backend = fault_backend(&t);
    let p = ensure_main_menu(&mut backend, &timeouts()).unwrap();
    verified_main_menu(&p);
    assert!(
        pings_before_first_write(&t) >= 24 + 4 + 3,
        "first write after {} probes: {:?}",
        pings_before_first_write(&t),
        t.methods()
    );
    let first = writes(&t).into_iter().next();
    assert_eq!(first.as_deref(), Some("navigate_menu \"abandon\""));
}

#[test]
fn wait_settled_never_settles_inside_the_loading_window() {
    let mut replay = ReplayTransport::new();
    Scene::run_loading().show(&mut replay);
    let mut backend = fault_backend(&FaultTransport::new(replay));
    let policy = SettlePolicy::default()
        .with_timeout(Duration::from_millis(100))
        .with_poll(Duration::from_millis(1), Duration::from_millis(2));
    match wait_settled(&mut backend, &policy) {
        Err(BridgeError::NotSettled { screen, .. }) => {
            assert_eq!(screen.unwrap().as_str(), "MAIN_MENU")
        }
        other => panic!("expected NotSettled, got {other:?}"),
    }
}

#[test]
fn a_navigation_while_loading_is_refused_and_never_sent() {
    let t = transport(Scene::run_loading());
    let mut s = Session::new(fault_backend(&t));
    match s.act(&Command::Navigate(MenuNav::AbandonRun)) {
        Err(BridgeError::ActionRefused { reason, .. }) => {
            assert!(reason.contains("loading"), "{reason}")
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
    assert!(writes(&t).is_empty(), "{:?}", writes(&t));
}

// ---- ensure_main_menu ----

fn from(scene: Scene) -> FaultTransport {
    let t = transport(scene);
    t.on(|r| is_nav(r, "abandon"), |r| Scene::game_over().show(r));
    t.on(
        |r| is_tool(r, "call_method"),
        |r| Scene::main_menu().show(r),
    );
    t
}

#[test]
fn ensure_main_menu_on_the_main_menu_sends_nothing() {
    let t = transport(Scene::main_menu());
    let p = ensure_main_menu(&mut fault_backend(&t), &timeouts()).unwrap();
    verified_main_menu(&p);
    assert!(writes(&t).is_empty(), "{:?}", writes(&t));
}

#[test]
fn ensure_main_menu_from_inside_a_run_abandons_and_leaves_the_game_over_screen() {
    for scene in [Scene::combat(), Scene::map(), Scene::event()] {
        let t = from(scene.clone());
        let p = ensure_main_menu(&mut fault_backend(&t), &timeouts())
            .unwrap_or_else(|e| panic!("from {:?}: {e}", scene.screen));
        verified_main_menu(&p);
        assert_eq!(
            writes(&t),
            [
                "navigate_menu \"abandon\"",
                "navigate_menu \"return_to_main_menu\"",
                "tools/call \"call_method\"",
            ],
            "from {:?}",
            scene.screen
        );
    }
}

#[test]
fn leaving_the_game_over_screen_calls_its_return_to_main_menu_through_the_explorer() {
    let t = from(Scene::game_over());
    ensure_main_menu(&mut fault_backend(&t), &timeouts()).unwrap();
    let calls: Vec<_> = t
        .requests()
        .into_iter()
        .filter(|r| r.method == "tools/call")
        .filter(|r| {
            !is_tool(r, "find_nodes")
                || r.params.as_ref().unwrap()["arguments"]["pattern"] != "MainMenu"
        })
        .map(|r| r.params.unwrap())
        .collect();
    assert_eq!(calls[0]["arguments"], json!({"pattern": "*GameOver*"}));
    assert_eq!(
        calls[1]["arguments"],
        json!({
            "path": "/root/Game/RootSceneContainer/Run/GlobalUi/OverlayScreensContainer/GameOverScreen",
            "method": "ReturnToMainMenu"
        })
    );
}

#[test]
fn once_return_to_main_menu_is_unknown_it_is_not_tried_again() {
    let t = from(Scene::game_over());
    let mut backend = fault_backend(&t);
    ensure_main_menu(&mut backend, &timeouts()).unwrap();
    Scene::game_over().show(&mut t.replay());
    t.on(
        |r| is_tool(r, "call_method"),
        |r| Scene::main_menu().show(r),
    );
    ensure_main_menu(&mut backend, &timeouts()).unwrap();
    let tries = t
        .requests()
        .iter()
        .filter(|r| is_nav(r, "return_to_main_menu"))
        .count();
    assert_eq!(tries, 1);
    let calls = t
        .requests()
        .iter()
        .filter(|r| is_tool(r, "call_method"))
        .count();
    assert_eq!(calls, 2);
}

#[test]
fn a_bridge_that_knows_return_to_main_menu_needs_no_explorer_call() {
    let t = transport(Scene::game_over());
    t.replay().insert(
        "navigate_menu",
        common::envelope(json!({"success": true, "target": "return_to_main_menu"})),
    );
    t.on(
        |r| is_nav(r, "return_to_main_menu"),
        |r| Scene::main_menu().show(r),
    );
    ensure_main_menu(&mut fault_backend(&t), &timeouts()).unwrap();
    assert!(
        !t.requests().iter().any(|r| is_tool(r, "call_method") || {
            is_tool(r, "find_nodes")
                && r.params.as_ref().unwrap()["arguments"]["pattern"] == "*GameOver*"
        }),
        "{:?}",
        t.requests()
    );
}

#[test]
fn ensure_main_menu_goes_back_from_a_menu_screen() {
    let t = transport(Scene::screen("CHARACTER_SELECT", false));
    t.replay().insert(
        "navigate_menu",
        common::envelope(json!({"success": true, "target": "back"})),
    );
    t.on(|r| is_nav(r, "back"), |r| Scene::main_menu().show(r));
    let p = ensure_main_menu(&mut fault_backend(&t), &timeouts()).unwrap();
    verified_main_menu(&p);
    assert_eq!(writes(&t), ["navigate_menu \"back\""]);
}

#[test]
fn an_abandon_acked_on_a_map_that_stays_is_a_postcondition_failure_naming_it() {
    let t = transport(Scene::map());
    match ensure_main_menu(&mut fault_backend(&t), &timeouts()) {
        Err(BridgeError::PostconditionFailed {
            action, observed, ..
        }) => {
            assert_eq!(action, "navigate(abandon_run)");
            assert!(observed.contains("MAP"), "{observed}");
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
}

#[test]
fn an_unknown_screen_is_not_settled_naming_it_and_sends_nothing() {
    let t = transport(Scene::screen("SOMETHING_NEW", true));
    match ensure_main_menu(&mut fault_backend(&t), &timeouts()) {
        Err(BridgeError::NotSettled { screen, .. }) => {
            assert_eq!(screen.unwrap().as_str(), "SOMETHING_NEW")
        }
        other => panic!("expected NotSettled, got {other:?}"),
    }
    assert!(writes(&t).is_empty());
}

#[test]
fn a_combat_room_that_never_opens_is_a_wedge() {
    let t = transport(Scene::screen("MENU_NCombatRoom", true));
    match ensure_main_menu(&mut fault_backend(&t), &timeouts()) {
        Err(BridgeError::Wedged { reason, .. }) => {
            assert_eq!(reason, WedgeReason::StuckScreen("MENU_NCombatRoom".into()))
        }
        other => panic!("expected Wedged, got {other:?}"),
    }
    assert!(writes(&t).is_empty());
}

#[test]
fn a_ping_that_never_has_a_screen_is_a_wedge() {
    let t = transport(Scene::screenless());
    match ensure_main_menu(&mut fault_backend(&t), &timeouts()) {
        Err(BridgeError::Wedged { reason, .. }) => {
            assert_eq!(reason, WedgeReason::PingWithoutScreen)
        }
        other => panic!("expected Wedged, got {other:?}"),
    }
}

#[test]
fn an_unreachable_explorer_is_a_typed_error_naming_its_port() {
    let t = transport(Scene::main_menu());
    let mut backend = McpTestBackend::new(t.clone());
    match ensure_main_menu(&mut backend, &timeouts()) {
        Err(e @ BridgeError::ExplorerUnavailable { .. }) => {
            assert!(e.to_string().contains("27020"), "{e}")
        }
        other => panic!("expected ExplorerUnavailable, got {other:?}"),
    }
}

// ---- Game: status, start, stop ----

#[test]
fn start_from_not_running_launches_and_waits_for_a_verified_main_menu() {
    let t = transport(Scene::main_menu());
    t.set_fault(Fault::Refused);
    let launcher = FakeLauncher::new(t.clone(), None);
    let mut game = Game::new(fault_backend(&t), launcher.clone()).with_timeouts(timeouts());
    assert_eq!(game.status().unwrap(), GameStatus::NotRunning);
    let p = game.start().unwrap();
    verified_main_menu(&p);
    assert_eq!(launcher.launches(), 1);
    assert!(matches!(game.status().unwrap(), GameStatus::Ready(_)));
}

#[test]
fn start_on_a_running_main_menu_does_not_relaunch_or_send() {
    let (mut game, t, launcher) = game(Scene::main_menu());
    let p = game.start().unwrap();
    verified_main_menu(&p);
    assert_eq!(launcher.launches(), 0);
    assert!(writes(&t).is_empty());
}

#[test]
fn stop_kills_the_process_and_verifies_it_is_gone() {
    let (mut game, _t, launcher) = game(Scene::main_menu());
    game.stop().unwrap();
    assert_eq!(launcher.kills(), 1);
    assert_eq!(game.status().unwrap(), GameStatus::NotRunning);
}

#[test]
fn restart_stops_and_starts() {
    let (mut game, _t, launcher) = game(Scene::combat());
    verified_main_menu(&game.restart().unwrap());
    assert_eq!((launcher.kills(), launcher.launches()), (1, 1));
}

#[test]
fn status_reports_loading() {
    let (mut game, _t, _) = game(Scene::run_loading());
    assert!(matches!(game.status().unwrap(), GameStatus::Loading(_)));
}

// ---- Game: a dead process ----

#[test]
fn a_process_that_vanished_is_crashed() {
    let (mut game, _t, launcher) = game(Scene::combat());
    assert!(matches!(game.status().unwrap(), GameStatus::Ready(_)));
    launcher.crash();
    match game.status().unwrap() {
        GameStatus::Crashed { pid, last_screen } => {
            assert_eq!(pid, Some(7));
            assert_eq!(last_screen.unwrap().as_str(), "COMBAT_PLAYER_TURN");
        }
        other => panic!("expected Crashed, got {other:?}"),
    }
}

/// Seen live right after a SIGKILL: the socket resets instead of refusing.
#[test]
fn a_vanished_process_whose_socket_resets_is_crashed_and_recovered() {
    let (mut game, t, launcher) = game(Scene::combat());
    game.status().unwrap();
    launcher.crash();
    t.set_fault(Fault::Reset);
    assert!(
        matches!(game.status().unwrap(), GameStatus::Crashed { .. }),
        "{:?}",
        game.status()
    );
    verified_main_menu(&game.ensure_main_menu().unwrap());
    assert_eq!(launcher.launches(), 1);
}

#[test]
fn ensure_main_menu_recovers_a_crash_once_with_one_launch() {
    let (mut game, _t, launcher) = game(Scene::combat());
    launcher.crash();
    let p = game.ensure_main_menu().unwrap();
    verified_main_menu(&p);
    assert_eq!(launcher.launches(), 1);
    assert_eq!(game.recoveries().len(), 1);
    assert!(game.recoveries()[0].restarted);
    assert!(
        game.recoveries()[0].cause.contains("crashed"),
        "{}",
        game.recoveries()[0].cause
    );
}

#[test]
fn a_second_crash_during_recovery_fails_crashed_without_looping() {
    let (mut game, _t, launcher) = game(Scene::combat());
    launcher.launches_will(&[Launch::Dies]);
    launcher.crash();
    match game.ensure_main_menu() {
        Err(BridgeError::Crashed { .. }) => {}
        other => panic!("expected Crashed, got {other:?}"),
    }
    assert_eq!(launcher.launches(), 1);
}

// ---- Game: a wedge ----

#[test]
fn a_bridge_that_accepts_and_never_replies_is_wedged() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        thread::sleep(Duration::from_secs(2));
        drop(stream);
    });
    let backend = McpTestBackend::new(
        TcpTransport::new("127.0.0.1", port).with_timeout(Duration::from_millis(200)),
    );
    let launcher = FakeLauncher::new(transport(Scene::main_menu()), Some(7));
    let mut game = Game::new(backend, launcher).with_timeouts(timeouts());
    match game.status().unwrap() {
        GameStatus::Wedged { reason, .. } => assert_eq!(reason, WedgeReason::BridgeTimeout),
        other => panic!("expected Wedged, got {other:?}"),
    }
}

#[test]
fn a_wedge_is_recovered_by_killing_and_relaunching() {
    let (mut game, t, launcher) = game(Scene::combat());
    t.set_fault(Fault::Timeout);
    let p = game.ensure_main_menu().unwrap();
    verified_main_menu(&p);
    assert_eq!((launcher.kills(), launcher.launches()), (1, 1));
    assert!(game.recoveries()[0].cause.contains("wedged"));
}

#[test]
fn recover_after_a_wedge_returns_a_verified_main_menu() {
    let (mut game, t, launcher) = game(Scene::combat());
    t.set_fault(Fault::Timeout);
    let cause = BridgeError::Wedged {
        screen: None,
        reason: WedgeReason::BridgeTimeout,
    };
    let r = game.recover(&cause).unwrap();
    assert!(r.restarted);
    verified_main_menu(&r.probe);
    assert_eq!(launcher.launches(), 1);
}

// ---- Game: recovery stays narrow ----

#[test]
fn errors_that_are_not_wedges_or_crashes_never_relaunch() {
    // NotSettled: an unknown screen.
    let (mut game, _t, launcher) = game(Scene::screen("SOMETHING_NEW", true));
    assert!(matches!(
        game.ensure_main_menu(),
        Err(BridgeError::NotSettled { .. })
    ));
    assert_eq!(launcher.launches(), 0);

    // PostconditionFailed: an abandon that does nothing.
    let (mut game, _t, launcher) = self::game(Scene::map());
    assert!(matches!(
        game.ensure_main_menu(),
        Err(BridgeError::PostconditionFailed { .. })
    ));
    assert_eq!(launcher.launches(), 0);

    // ActionRefused: the bridge rejects the abandon.
    let (mut game, t, launcher) = self::game(Scene::map());
    t.replay().insert(
        "navigate_menu",
        common::envelope(json!({"error": "No run in progress to abandon"})),
    );
    assert!(matches!(
        game.ensure_main_menu(),
        Err(BridgeError::ActionRefused { .. })
    ));
    assert_eq!(launcher.launches(), 0);
}

#[test]
fn wait_run_ready_on_a_game_that_died_recovers_and_reports_the_lost_run() {
    let (mut game, _t, launcher) = game(Scene::run_loading());
    launcher.crash();
    match game.wait_run_ready() {
        Err(BridgeError::NotSettled { unmet, .. }) => {
            assert!(unmet.contains("restarted"), "{unmet}")
        }
        other => panic!("expected NotSettled, got {other:?}"),
    }
    assert_eq!(launcher.launches(), 1);
}

// ---- longer than one step, still not a wedge ----

#[test]
fn a_run_tearing_down_for_longer_than_a_step_still_reaches_the_main_menu() {
    let t = transport(Scene::game_over());
    t.on(
        |r| is_tool(r, "call_method"),
        |r| {
            // The run tears down (MAIN_MENU, run in progress) for ~300 reads.
            r.insert("ping", Scene::run_loading().ping());
            r.insert("get_screen", Scene::run_loading().get_screen());
            Scene::run_loading().push_probes(r, 300);
            Scene::main_menu().push_probes(r, 1);
            r.insert("tools/call", common::find_nodes(&["MainMenu"]));
            Scene::main_menu().show_state(r);
        },
    );
    let timeouts = timeouts()
        .with_step(Duration::from_millis(50))
        .with_run_load(Duration::from_secs(5));
    let p = ensure_main_menu(&mut fault_backend(&t), &timeouts).unwrap();
    verified_main_menu(&p);
}

#[test]
fn an_enemy_turn_longer_than_the_stuck_screen_deadline_is_not_a_wedge() {
    let mut replay = ReplayTransport::new();
    Scene::combat().show(&mut replay);
    Scene::screen("COMBAT_ENEMY_TURN", true).push_probes(&mut replay, 300);
    Scene::combat().push_probes(&mut replay, 1);
    let t = FaultTransport::new(replay);
    t.on(|r| is_nav(r, "abandon"), |r| Scene::game_over().show(r));
    t.on(
        |r| is_tool(r, "call_method"),
        |r| Scene::main_menu().show(r),
    );
    let timeouts = timeouts()
        .with_stuck_screen(Duration::from_millis(20))
        .with_step(Duration::from_secs(5));
    verified_main_menu(&ensure_main_menu(&mut fault_backend(&t), &timeouts).unwrap());
}

#[test]
fn a_game_still_booting_is_waited_for_not_restarted() {
    let (mut game, _t, launcher) = game(Scene::main_menu());
    launcher.booting(20);
    verified_main_menu(&game.ensure_main_menu().unwrap());
    assert_eq!((launcher.launches(), launcher.kills()), (0, 0));
    assert!(game.recoveries().is_empty());
}

#[test]
fn a_launch_without_the_explorer_mod_fails_naming_it() {
    let t = transport(Scene::main_menu());
    t.set_fault(Fault::Refused);
    let launcher = FakeLauncher::new(t.clone(), None);
    let mut game = Game::new(McpTestBackend::new(t.clone()), launcher).with_timeouts(timeouts());
    match game.start() {
        Err(e @ BridgeError::ExplorerUnavailable { .. }) => {
            assert!(e.to_string().contains("27020"), "{e}")
        }
        other => panic!("expected ExplorerUnavailable, got {other:?}"),
    }
}

#[test]
fn a_game_over_node_with_another_name_is_refused_not_called() {
    let t = transport(Scene::game_over());
    t.replay()
        .insert("tools/call", common::find_nodes(&["GameOverBackground"]));
    match ensure_main_menu(&mut fault_backend(&t), &timeouts()) {
        Err(BridgeError::ActionRefused { reason, .. }) => {
            assert!(reason.contains("GameOverBackground"), "{reason}")
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
    assert!(!t.requests().iter().any(|r| is_tool(r, "call_method")));
}

#[test]
fn a_running_game_whose_ping_never_has_a_screen_is_recovered() {
    let (mut game, _t, launcher) = game(Scene::screenless());
    verified_main_menu(&game.ensure_main_menu().unwrap());
    assert_eq!((launcher.kills(), launcher.launches()), (1, 1));
    assert!(game.recoveries()[0].cause.contains("without a screen"));
}

#[test]
fn a_combat_room_opening_into_a_long_enemy_turn_is_not_a_wedge() {
    let mut replay = ReplayTransport::new();
    Scene::combat().show(&mut replay);
    Scene::screen("MENU_NCombatRoom", true).push_probes(&mut replay, 5);
    Scene::screen("COMBAT_ENEMY_TURN", true).push_probes(&mut replay, 300);
    Scene::combat().push_probes(&mut replay, 1);
    let t = FaultTransport::new(replay);
    t.on(|r| is_nav(r, "abandon"), |r| Scene::game_over().show(r));
    t.on(
        |r| is_tool(r, "call_method"),
        |r| Scene::main_menu().show(r),
    );
    let timeouts = timeouts()
        .with_stuck_screen(Duration::from_millis(20))
        .with_step(Duration::from_secs(5));
    verified_main_menu(&ensure_main_menu(&mut fault_backend(&t), &timeouts).unwrap());
}

#[test]
fn settings_opened_over_a_run_are_left_with_back() {
    let t = from(Scene::screen("SETTINGS", true));
    t.replay().insert(
        "navigate_menu",
        common::envelope(json!({"success": true, "target": "back"})),
    );
    t.on(|r| is_nav(r, "back"), |r| Scene::event().show(r));
    verified_main_menu(&ensure_main_menu(&mut fault_backend(&t), &timeouts()).unwrap());
    assert_eq!(writes(&t)[0], "navigate_menu \"back\"");
}

#[test]
fn a_session_without_the_explorer_can_still_act_on_the_main_menu() {
    let t = transport(Scene::main_menu());
    let mut s = Session::with_policy(
        McpTestBackend::new(t.clone()),
        SettlePolicy::default().with_timeout(Duration::from_millis(100)),
    );
    // Back on a main menu changes nothing here, so it ends in a
    // postcondition failure; what matters is that it was sent.
    t.replay().insert(
        "navigate_menu",
        common::envelope(json!({"success": true, "target": "back"})),
    );
    let _ = s.act(&Command::Navigate(MenuNav::Back));
    assert_eq!(writes(&t), ["navigate_menu \"back\""]);
}

#[test]
fn a_crash_mid_operation_whose_process_lingers_is_recovered() {
    let (mut game, t, launcher) = game(Scene::combat());
    let dying = launcher.clone();
    t.on(|r| is_nav(r, "abandon"), move |_| dying.crash_lingering(3));
    verified_main_menu(&game.ensure_main_menu().unwrap());
    assert_eq!(launcher.launches(), 1);
    assert!(
        game.recoveries()[0].cause.contains("crashed"),
        "{}",
        game.recoveries()[0].cause
    );
}

#[test]
fn a_load_that_never_ends_is_a_wedge_and_recovered() {
    let (mut game, _t, launcher) = game(Scene::run_loading());
    verified_main_menu(&game.ensure_main_menu().unwrap());
    assert_eq!((launcher.kills(), launcher.launches()), (1, 1));
    assert!(
        game.recoveries()[0].cause.contains("loading"),
        "{}",
        game.recoveries()[0].cause
    );
}
