//! Replays the committed game traces against the sim, with no game.
//! `cargo test -p sts-parity --test trace_replay -- --nocapture` prints the report.

use std::path::Path;

use sts_bridge::console::catalog::GAME_VERSION;
use sts_parity::trace::{audit, load_dir, replay_failures, HARNESS_VERSION, MAX_STATE_BYTES};
use sts_parity::{cases, EXPECTED};

fn traces() -> Vec<sts_parity::TraceFile> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/traces");
    load_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
}

#[test]
fn every_declared_case_has_a_current_trace() {
    let a = audit(&traces(), &cases(&[]), HARNESS_VERSION, GAME_VERSION);
    println!("{}", a.report(GAME_VERSION));
    assert!(a.failures.is_empty(), "{:#?}", a.failures);
}

#[test]
fn every_trace_replays_to_the_recorded_game_states() {
    let mut failures = Vec::new();
    for f in traces() {
        if let Ok(t) = &f.trace {
            failures.extend(replay_failures(t, EXPECTED));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_recorded_state_is_within_the_size_cap() {
    let mut big = Vec::new();
    for f in traces() {
        if let Ok(t) = &f.trace {
            for (i, n) in t.state_sizes().into_iter().enumerate() {
                if n > MAX_STATE_BYTES {
                    big.push(format!("{} state {i}: {n} bytes", f.file));
                }
            }
        }
    }
    assert!(big.is_empty(), "over {MAX_STATE_BYTES} bytes: {big:#?}");
}
