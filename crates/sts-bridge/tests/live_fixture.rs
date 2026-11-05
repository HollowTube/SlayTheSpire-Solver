//! Verified combat fixtures against the live game. Opt-in and serial:
//!
//! ```text
//! cargo test -p sts-bridge --test live_fixture -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! Needs Steam running and logged in, and the MCPTest and GodotExplorer mods
//! installed. The game need not be running: each test sets up through
//! `Game::start`, and `Fixtures::ensure` resets to the fixture's seeded run
//! when it is not in progress. Each test ends on the main menu. An
//! unreachable bridge fails the test, never skips it.
//!
//! The first thing each test does is time five state reads. A median over
//! 300 ms means a slow bridge (an unfocused game capped at 30 FPS): it prints
//! `SLOW BRIDGE`, runs every correctness check, and reports the timing
//! criteria instead of asserting them.
#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use common::{dirty, hold_game, Dirt, IN_RUN_DIRT};
use sts_bridge::mcptest::TcpTransport;
use sts_bridge::{
    Backend, CardId, CardSpec, EnemySpec, Energy, Fixture, FixtureError, FixturePath, Fixtures,
    Game, Hp, McpTestBackend, PowerName, SteamLauncher, Timeouts, NIBBITS_WEAK,
};

type Live = Fixtures<McpTestBackend<TcpTransport>, SteamLauncher>;

/// A state read slower than this marks the bridge slow.
const SLOW_READ: Duration = Duration::from_millis(300);
/// The provisional per-test setup target on a focused game.
const SETUP_TARGET: Duration = Duration::from_secs(5);
/// A reset must reach an accepted `start_run` within this.
const RESET_TARGET: Duration = Duration::from_secs(10);

fn live() -> Live {
    let mut fx = Fixtures::new(Game::new(
        McpTestBackend::from_env(),
        SteamLauncher::from_env(),
    ));
    fx.game_mut()
        .start()
        .unwrap_or_else(|e| panic!("could not start the game: {e}"));
    fx
}

fn secs(d: Duration) -> String {
    format!("{:.2}s", d.as_secs_f64())
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn percentile(v: &[Duration], p: f64) -> Duration {
    let mut v = v.to_vec();
    v.sort();
    let i = ((p * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1;
    v[i]
}

fn summary(name: &str, v: &[Duration]) -> String {
    if v.is_empty() {
        return format!("{name}: none");
    }
    format!(
        "{name}: n={} p50={} p95={} max={}",
        v.len(),
        secs(percentile(v, 0.5)),
        secs(percentile(v, 0.95)),
        secs(*v.iter().max().unwrap())
    )
}

/// The median of five state reads and of five pings; whether the bridge is slow.
fn calibrate(fx: &mut Live) -> bool {
    let backend = fx.game_mut().backend_mut();
    let mut reads = Vec::new();
    let mut pings = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        backend
            .state()
            .unwrap_or_else(|e| panic!("calibration read: {e}"));
        reads.push(started.elapsed());
        let h = backend
            .health()
            .unwrap_or_else(|e| panic!("calibration ping: {e}"));
        pings.push(Duration::from_secs_f64(h.latency_ms / 1000.0));
    }
    let read = median(reads);
    let ping = median(pings);
    let slow = read > SLOW_READ;
    eprintln!(
        "calibration: state() median {} (5 reads), ping median {}{}",
        secs(read),
        secs(ping),
        if slow {
            " -- SLOW BRIDGE: timing criteria are reported, not asserted"
        } else {
            ""
        }
    );
    slow
}

fn fixture() -> Fixture {
    Fixture::new(NIBBITS_WEAK)
        .enemy(EnemySpec::new(20))
        .player_hp(Hp::Exact(60))
        .player_power(PowerName::new("STRENGTH_POWER").unwrap(), 1)
        .hand([
            CardSpec::from(CardId::Bash),
            CardSpec::upgraded(CardId::StrikeIronclad),
        ])
        .draw([CardId::DefendIronclad, CardId::StrikeIronclad])
}

fn leave(fx: &mut Live) {
    fx.game_mut()
        .ensure_main_menu()
        .unwrap_or_else(|e| panic!("ensure_main_menu at the end: {e}"));
}

/// From 50 deliberately dirtied states, `ensure` returns the same verified
/// state every time. Two iterations force the reset path (main menu, game
/// over).
#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; about 10-15 min focused"]
fn fifty_dirtied_states_all_verify_identically() {
    let _game = hold_game();
    let started = Instant::now();
    let mut fx = live();
    let slow = calibrate(&mut fx);
    let fixture = fixture();

    let first = fx
        .ensure(&fixture)
        .unwrap_or_else(|e| panic!("first ensure: {e}"));
    eprintln!(
        "session setup: {} ({:?}, {} commands, instant {:?})",
        secs(first.timings.total),
        first.path,
        first.timings.commands,
        first.instant
    );
    let projection = fixture.project(&first.state);

    let mut failures = Vec::new();
    let mut in_run = Vec::new();
    let mut commands = Vec::new();
    let mut accepted = Vec::new();
    let mut first_fight = Vec::new();
    let mut intents = BTreeSet::new();
    let mut max_hps = BTreeSet::new();
    let mut verified = 0;
    for i in 0..50 {
        let dirt = match i {
            24 => Dirt::MainMenu,
            49 => Dirt::GameOver,
            _ => IN_RUN_DIRT[i % IN_RUN_DIRT.len()],
        };
        if let Err(e) = dirty(&mut fx, dirt) {
            eprintln!("FAIL {i:02} dirty {dirt:?}: {e}");
            failures.push(format!("{i}: dirty {dirt:?}: {e}"));
        }
        match fx.ensure(&fixture) {
            Ok(v) => {
                verified += 1;
                let same = fixture.project(&v.state) == projection;
                eprintln!(
                    "{} {i:02} after {dirt:?}: {:?} in {} ({} commands){}",
                    if same { "PASS" } else { "FAIL" },
                    v.path,
                    secs(v.timings.total),
                    v.timings.commands,
                    v.timings
                        .reset
                        .map(|r| format!(
                            ", reset: accepted {} ready {} first fight {}",
                            secs(r.to_accepted),
                            secs(r.to_ready),
                            r.to_first_fight.map_or("-".into(), secs)
                        ))
                        .unwrap_or_default()
                );
                if !same {
                    failures.push(format!(
                        "{i}: not identical: {:?} vs {:?}",
                        fixture.project(&v.state),
                        projection
                    ));
                }
                if let Some(c) = &v.state.combat {
                    for e in &c.enemies {
                        max_hps.insert(e.max_hp);
                        intents.insert(e.intent.as_ref().map(|i| i.move_id.clone()));
                    }
                }
                match (v.path, v.timings.reset) {
                    (FixturePath::Reset, Some(r)) => {
                        accepted.push(r.to_accepted);
                        if let Some(f) = r.to_first_fight {
                            first_fight.push(f);
                        }
                    }
                    _ => {
                        in_run.push(v.timings.total);
                        commands.push(v.timings.commands);
                    }
                }
            }
            Err(e) => {
                eprintln!("FAIL {i:02} after {dirt:?}: {e}");
                failures.push(format!("{i} after {dirt:?}: {e}"));
            }
        }
    }

    eprintln!("verified {verified}/50; resets {}", fx.resets());
    eprintln!("{}", summary("per-test setup (in-run)", &in_run));
    eprintln!("{}", summary("reset to accepted start_run", &accepted));
    eprintln!("{}", summary("reset to first ready fight", &first_fight));
    if !commands.is_empty() {
        let total: usize = commands.iter().sum();
        eprintln!(
            "console commands per in-run test: mean {:.1}, max {}",
            total as f64 / commands.len() as f64,
            commands.iter().max().unwrap()
        );
    }
    eprintln!("undeclared, varied: enemy max_hp {max_hps:?}; intent {intents:?}");
    eprintln!("wall clock: {}", secs(started.elapsed()));
    leave(&mut fx);

    assert!(failures.is_empty(), "failed: {failures:#?}");
    if slow {
        eprintln!("SLOW BRIDGE: the timing criteria above are reported, not asserted");
        return;
    }
    let p95 = percentile(&in_run, 0.95);
    assert!(
        p95 <= SETUP_TARGET,
        "per-test setup p95 {} over {}",
        secs(p95),
        secs(SETUP_TARGET)
    );
    for a in &accepted {
        assert!(
            *a < RESET_TARGET,
            "reset to accepted start_run took {}",
            secs(*a)
        );
    }
    let run_load = Timeouts::default().run_load;
    for f in &first_fight {
        assert!(*f <= run_load, "reset to first fight took {}", secs(*f));
    }
}

/// A fixture that cannot be met names its fields: a static limit at once,
/// a dynamic one after exactly one reset.
#[test]
#[ignore = "live: needs Steam + STS2 with MCPTest and GodotExplorer; one or two resets"]
fn an_unmeetable_fixture_names_its_fields() {
    let _game = hold_game();
    let started = Instant::now();
    let mut fx = live();
    calibrate(&mut fx);
    fx.ensure(&fixture())
        .unwrap_or_else(|e| panic!("setup ensure: {e}"));

    let upgraded_draw = fixture().draw([CardSpec::upgraded(CardId::Bash)]);
    let resets = fx.resets();
    let at = Instant::now();
    match fx.ensure(&upgraded_draw) {
        Err(FixtureError::Unreachable { fields, .. }) => {
            assert_eq!(fields, ["piles.draw"]);
            eprintln!("PASS static: {fields:?} in {}", secs(at.elapsed()));
        }
        other => panic!("static: expected Unreachable, got {other:?}"),
    }
    assert_eq!(fx.resets(), resets, "a static limit reset the game");

    let impossible = Fixture::new(NIBBITS_WEAK)
        .enemy(EnemySpec::new(999))
        .energy(Energy::Exact(0));
    let at = Instant::now();
    match fx.ensure(&impossible) {
        Err(FixtureError::Mismatch {
            fields,
            after_reset: true,
        }) => {
            let names: Vec<&str> = fields.iter().map(|f| f.field.as_str()).collect();
            eprintln!("PASS dynamic: {names:?} in {}", secs(at.elapsed()));
            assert_eq!(names, ["enemies[0].hp", "player.energy"]);
            assert!(fields.iter().all(|f| f.needs_setter), "{fields:?}");
        }
        other => panic!("dynamic: expected Mismatch after a reset, got {other:?}"),
    }
    assert_eq!(
        fx.resets(),
        resets + 1,
        "a dynamic limit resets exactly once"
    );
    eprintln!("wall clock: {}", secs(started.elapsed()));
    leave(&mut fx);
}
