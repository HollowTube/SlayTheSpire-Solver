//! The live card-parity suite. Opt-in and serial; it has its own `main`.
//!
//! ```text
//! cargo test -p sts-parity --test live_cards -- --live [--card Strike]... [--list] [--allow-slow]
//! ```
//!
//! Needs Steam running and logged in, the MCPTest and GodotExplorer mods
//! installed, and the game focused. Without `--live` it prints "not run" and
//! exits 0, so a plain `cargo test` needs no game. `--card Name` selects both
//! variants of a card, `--card Name+` the upgraded one.
//!
//! With `STS2_RECORD_TRACES=1` each run case is also written to
//! `tests/traces/<case>.json`, tagged with the game build read from
//! `release_info.json` in `$STS2_GAME_DIR` (or next to `$STS2_PCK`). Recording
//! runs the whole case that holds a selected card, so batches keep their names.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use sts_bridge::console::catalog::GAME_VERSION;
use sts_bridge::{Backend, Fixtures, Game, McpTestBackend, SteamLauncher};
use sts_parity::trace::{self, BackendBuild, GameBuild, Trace};
use sts_parity::{
    calibrate, cases, cases_holding, classify, ParityCase, ParityOutcome, Runner, Verdict, EXPECTED,
};

const TRACES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/traces");

struct Recording {
    game: GameBuild,
    backend: BackendBuild,
}

fn recording_requested() -> bool {
    std::env::var("STS2_RECORD_TRACES").is_ok_and(|v| v == "1")
}

/// Whether the case has its trace (a blocked case has none to write).
fn record(rec: &Recording, outcome: &ParityOutcome) -> bool {
    let name = &outcome.case.name;
    match Trace::from_outcome(outcome, &rec.game, &rec.backend) {
        Ok(t) => match trace::write(Path::new(TRACES), &t) {
            Ok(p) => {
                println!("    trace: {}", p.display());
                true
            }
            Err(e) => {
                println!("    TRACE FAILED {name}: {e}");
                false
            }
        },
        Err(why) => {
            println!("    TRACE SKIPPED {name}: {why}");
            outcome.case.blocked.is_some()
        }
    }
}

fn main() -> ExitCode {
    let mut live = false;
    let mut list = false;
    let mut allow_slow = false;
    let mut filter = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--live" => live = true,
            "--list" => list = true,
            "--allow-slow" => allow_slow = true,
            "--card" => match args.next() {
                Some(c) => filter.push(c),
                None => {
                    eprintln!("--card needs a card name");
                    return ExitCode::from(2);
                }
            },
            _ => {}
        }
    }
    let recording = recording_requested();
    let cases = if recording {
        cases_holding(&filter)
    } else {
        cases(&filter)
    };
    if list {
        for c in &cases {
            let tag = c.blocked.as_ref().map_or("", |_| " [blocked]");
            println!("{}{tag}: {}", c.name, c.cards.join(", "));
        }
        return ExitCode::SUCCESS;
    }
    if !live {
        println!("live_cards: not run (pass --live to run against the game)");
        return ExitCode::SUCCESS;
    }
    if cases.is_empty() {
        eprintln!("no case matches {filter:?}");
        return ExitCode::from(2);
    }
    run(&cases, allow_slow, recording)
}

/// The game build, checked before the game starts.
fn game_build() -> Result<GameBuild, String> {
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    trace::game_build(
        var("STS2_GAME_DIR").as_deref(),
        var("STS2_PCK").as_deref(),
        GAME_VERSION,
    )
    .map_err(|e| e.to_string())
}

fn run(cases: &[ParityCase], allow_slow: bool, recording: bool) -> ExitCode {
    let started = Instant::now();
    let game = if recording {
        match game_build() {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("cannot record traces: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    let mut fx = Fixtures::new(Game::new(
        McpTestBackend::from_env(),
        SteamLauncher::from_env(),
    ));
    if let Err(e) = fx.game_mut().start() {
        eprintln!("could not start the game: {e}");
        return ExitCode::from(2);
    }
    match calibrate(fx.game_mut().backend_mut()) {
        Ok((median, slow)) => {
            eprintln!("calibration: state() median {:.0?}", median);
            if slow && !allow_slow {
                eprintln!("SLOW BRIDGE: focus the game (or pass --allow-slow)");
                let _ = fx.game_mut().ensure_main_menu();
                return ExitCode::from(2);
            }
        }
        Err(e) => {
            eprintln!("calibration failed: {e}");
            return ExitCode::from(2);
        }
    }
    let recording = match game {
        Some(game) => match fx.game_mut().backend_mut().health() {
            Ok(h) => Some(Recording {
                game,
                backend: BackendBuild {
                    name: h.backend,
                    version: h.version,
                },
            }),
            Err(e) => {
                eprintln!("cannot record traces: the backend health check failed: {e}");
                let _ = fx.game_mut().ensure_main_menu();
                return ExitCode::from(2);
            }
        },
        None => None,
    };
    let mut counts = std::collections::BTreeMap::<&'static str, usize>::new();
    let mut bad = Vec::new();
    let mut stopped = false;
    for case in cases {
        let at = Instant::now();
        match Runner::run(&mut fx, case) {
            Ok(outcome) => {
                let verdict = classify(&outcome, EXPECTED);
                println!(
                    "{} {} [{}] ({:.1?})",
                    verdict.label(),
                    case.name,
                    case.cards.join(", "),
                    at.elapsed()
                );
                match &verdict {
                    Verdict::Fail(lines) | Verdict::Stale(lines) => {
                        for l in lines {
                            println!("    {l}");
                        }
                        bad.push(case.name.clone());
                    }
                    Verdict::Expected(issues) => println!("    {}", issues.join(", ")),
                    Verdict::Blocked(why) => println!("    {why}"),
                    Verdict::Pass => {}
                }
                if let Some(rec) = &recording {
                    if !record(rec, &outcome) && !bad.contains(&case.name) {
                        bad.push(case.name.clone());
                    }
                }
                *counts.entry(verdict.label()).or_default() += 1;
            }
            Err(stop) => {
                println!("STOPPED {} : {stop}", case.name);
                eprintln!("the game is wedged or crashed: stopping, not relaunching: {stop}");
                stopped = true;
                break;
            }
        }
    }
    println!("summary: {counts:?} in {:.1?}", started.elapsed());
    if !bad.is_empty() {
        println!("failing cases: {bad:?}");
    }
    if !stopped {
        if let Err(e) = fx.game_mut().ensure_main_menu() {
            eprintln!("ensure_main_menu at the end: {e}");
        }
    }
    if stopped {
        ExitCode::from(2)
    } else if bad.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
