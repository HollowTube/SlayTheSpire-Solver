//! Typed console commands against the live game. Opt-in and serial:
//!
//! ```text
//! cargo test -p sts-bridge --test live_console -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! Needs Steam running and logged in, and the MCPTest and GodotExplorer mods
//! installed. The game need not be running: each test starts it, then sets up
//! with `Game::start`, `ensure_main_menu`, a seeded `start_run`,
//! `wait_run_ready` and `Session::console(Fight(NIBBITS_WEAK))`. Every console
//! command goes through `Session::console`; no test sends a raw console line,
//! because a room-changing command sent while a run loads crashes the game.
//! An unreachable bridge fails the test, never skips it.
#![cfg(unix)]

mod common;

use std::time::{Duration, Instant};

use common::{hold_game, live_backend};
use serde_json::json;
use sts_bridge::mcptest::TcpTransport;
use sts_bridge::{
    Backend, BridgeError, CardId, CombatPile, ConsoleCommand, Game, GameState, Launcher,
    McpTestBackend, PowerName, Relic, Session, SteamLauncher, Target, NIBBITS_WEAK,
};

type Live = Game<McpTestBackend<TcpTransport>, SteamLauncher>;
type Console = Session<McpTestBackend<TcpTransport>>;

const SEED: &str = "HOL107LIVE";

fn live_game() -> Live {
    let mut game = Game::new(McpTestBackend::from_env(), SteamLauncher::from_env());
    game.start()
        .unwrap_or_else(|e| panic!("could not start the game: {e}"));
    game
}

fn main_menu(game: &mut Live, from: &str) {
    game.ensure_main_menu()
        .unwrap_or_else(|e| panic!("ensure_main_menu {from}: {e}"));
    assert!(
        game.recoveries().is_empty(),
        "ensure_main_menu {from} needed a restart: {:?}",
        game.recoveries()
    );
}

/// Starts a seeded Ironclad run; it is loading when this returns.
fn start_run(game: &mut Live) {
    main_menu(game, "before start_run");
    let params = json!({"character": "Ironclad", "seed": SEED, "ascension": 0});
    game.backend_mut()
        .request("start_run", Some(&params))
        .unwrap_or_else(|e| panic!("start_run: {e}"));
}

fn session() -> Console {
    Session::new(live_backend())
}

fn secs(since: Instant) -> String {
    format!("{:.1}s", since.elapsed().as_secs_f64())
}

/// A seeded run, ready, and in `NIBBITS_WEAK`.
fn in_combat(game: &mut Live) -> Console {
    start_run(game);
    let started = Instant::now();
    game.wait_run_ready()
        .unwrap_or_else(|e| panic!("the run never became ready: {e}"));
    eprintln!("run ready after {}", secs(started));
    let mut s = session();
    let fought = Instant::now();
    s.console(&ConsoleCommand::Fight(NIBBITS_WEAK))
        .unwrap_or_else(|e| panic!("fight NIBBITS_WEAK: {e}"));
    eprintln!("fight settled in {}", secs(fought));
    s
}

fn combat(state: &GameState) -> &sts_bridge::CombatView {
    state.combat.as_ref().expect("in combat")
}

#[derive(Default)]
struct Report {
    failures: Vec<String>,
    slowest: Duration,
}

impl Report {
    /// Runs `command`, prints its result and duration, and records a failure.
    fn step(&mut self, s: &mut Console, command: ConsoleCommand) -> Option<GameState> {
        let started = Instant::now();
        let result = s.console(&command);
        let took = started.elapsed();
        self.slowest = self.slowest.max(took);
        match result {
            Ok(state) => {
                eprintln!("PASS {command} ({})", secs(started));
                Some(state)
            }
            Err(e) => {
                eprintln!("FAIL {command} ({}): {e}", secs(started));
                self.failures.push(format!("{command}: {e}"));
                None
            }
        }
    }
}

#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; starts a run"]
fn fight_is_refused_while_the_run_loads_and_works_once_it_is_ready() {
    let _game = hold_game();
    let mut game = live_game();
    start_run(&mut game);
    let pid = game.launcher_mut().pid();
    let mut s = session();
    match s.console(&ConsoleCommand::Fight(NIBBITS_WEAK)) {
        Err(BridgeError::ActionRefused { reason, .. }) => {
            assert!(reason.contains("loading"), "{reason}")
        }
        other => panic!("expected ActionRefused (room still loading), got {other:?}"),
    }
    game.wait_run_ready()
        .unwrap_or_else(|e| panic!("the run never became ready: {e}"));
    let state = s
        .console(&ConsoleCommand::Fight(NIBBITS_WEAK))
        .unwrap_or_else(|e| panic!("fight once ready: {e}"));
    assert_eq!(state.screen.as_str(), "COMBAT_PLAYER_TURN");
    assert_eq!(combat(&state).round, 1);
    assert_eq!(game.launcher_mut().pid(), pid, "the game restarted");
    main_menu(&mut game, "at the end");
}

/// Every command a fixture needs, in one combat, each checked before and
/// after; and a command the game ignores is reported as having no effect.
#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; starts a run"]
fn each_fixture_command_passes_its_check() {
    let _game = hold_game();
    let mut game = live_game();
    let mut s = in_combat(&mut game);
    let mut r = Report::default();
    let vulnerable = PowerName::new("VULNERABLE_POWER").unwrap();
    let strength = PowerName::new("STRENGTH_POWER").unwrap();
    let anchor = Relic::new("ANCHOR").unwrap();
    let nibbit = Target::Enemy(0);

    for pile in [
        CombatPile::Hand,
        CombatPile::Draw,
        CombatPile::Discard,
        CombatPile::Exhaust,
    ] {
        let card = ConsoleCommand::AddCard {
            card: CardId::Bash,
            pile,
        };
        r.step(&mut s, card);
    }
    r.step(
        &mut s,
        ConsoleCommand::RemoveCardFromHand { card: CardId::Bash },
    );
    r.step(&mut s, ConsoleCommand::Energy { amount: 2 });
    r.step(
        &mut s,
        ConsoleCommand::Block {
            target: Target::Player,
            amount: 5,
        },
    );
    r.step(
        &mut s,
        ConsoleCommand::Block {
            target: nibbit,
            amount: 5,
        },
    );
    // The enemy's 5 block soaks the first 5 of these 8.
    r.step(
        &mut s,
        ConsoleCommand::Damage {
            target: nibbit,
            amount: 8,
        },
    );
    r.step(
        &mut s,
        ConsoleCommand::Damage {
            target: Target::Player,
            amount: 12,
        },
    );
    r.step(&mut s, ConsoleCommand::HealPlayer { amount: 99 });
    for _ in 0..2 {
        r.step(
            &mut s,
            ConsoleCommand::Power {
                power: vulnerable,
                amount: 2,
                target: nibbit,
            },
        );
    }
    r.step(
        &mut s,
        ConsoleCommand::Power {
            power: strength,
            amount: 1,
            target: Target::Player,
        },
    );
    r.step(&mut s, ConsoleCommand::RelicAdd(anchor));
    r.step(&mut s, ConsoleCommand::RelicRemove(anchor));
    r.step(&mut s, ConsoleCommand::Draw { count: 2 });
    r.step(&mut s, ConsoleCommand::Upgrade { hand_index: 0 });
    r.step(&mut s, ConsoleCommand::DamageAllEnemies { amount: 3 });

    // The game rejects a discard-pile removal but the bridge still says
    // success: it must be reported, not trusted.
    let probe = ConsoleCommand::Probe("remove_card BASH discard".into());
    let started = Instant::now();
    match s.console(&probe) {
        Err(BridgeError::PostconditionFailed { .. }) => {
            eprintln!("PASS {probe} reported as no effect ({})", secs(started))
        }
        other => {
            eprintln!("FAIL {probe}: expected PostconditionFailed, got {other:?}");
            r.failures.push(format!("{probe}: {other:?}"));
        }
    }

    r.step(&mut s, ConsoleCommand::Win);
    eprintln!("slowest console call: {:.1}s", r.slowest.as_secs_f64());
    main_menu(&mut game, "at the end");
    assert!(r.failures.is_empty(), "failed: {:#?}", r.failures);
}

/// The room-change dwell, calibrated: the whole setup chain, ten times, must
/// never crash the game.
#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; starts 10 runs"]
fn the_setup_chain_never_crashes_the_game_10_of_10() {
    let _game = hold_game();
    let mut game = live_game();
    let pid = game.launcher_mut().pid().expect("the game is running");
    let mut slowest = Duration::ZERO;
    for run in 1..=10 {
        start_run(&mut game);
        game.wait_run_ready()
            .unwrap_or_else(|e| panic!("run {run}: the run never became ready: {e}"));
        let mut s = session();
        let started = Instant::now();
        let state = s
            .console(&ConsoleCommand::Fight(NIBBITS_WEAK))
            .unwrap_or_else(|e| panic!("run {run}: fight: {e}"));
        slowest = slowest.max(started.elapsed());
        assert_eq!(state.screen.as_str(), "COMBAT_PLAYER_TURN", "run {run}");
        assert_eq!(
            game.launcher_mut().pid(),
            Some(pid),
            "run {run}: the game crashed or restarted"
        );
        eprintln!("run {run}/10: fight settled in {}", secs(started));
        main_menu(&mut game, &format!("after run {run}"));
    }
    eprintln!("slowest fight: {:.1}s", slowest.as_secs_f64());
}

/// Instant mode is set, not toggled. The bridge reports `fast_mode` only
/// from HOL-103 on; until then this says so and checks nothing.
#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; HOL-103's ping.fast_mode"]
fn instant_mode_is_set_not_toggled() {
    let _game = hold_game();
    let mut game = live_game();
    main_menu(&mut game, "before instant");
    let mut backend = live_backend();
    let Some(current) = backend.health().unwrap().fast_mode else {
        eprintln!("NOT RUN: the bridge's ping has no fast_mode (waits for HOL-103)");
        return;
    };
    let mut s = Session::new(backend);
    for want in [!current, !current, current] {
        assert_eq!(s.set_instant(want).unwrap(), want);
        assert_eq!(s.backend_mut().health().unwrap().fast_mode, Some(want));
    }
}
