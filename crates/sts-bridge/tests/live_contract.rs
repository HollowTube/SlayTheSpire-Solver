//! Contract tests against the live game. Opt-in and serial:
//!
//! ```text
//! cargo test -p sts-bridge --test live_contract -- --ignored --test-threads=1
//! ```
//!
//! The game must already be on the screen a test names; these tests do not
//! navigate. An unreachable bridge fails the test, never skips it.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use sts_bridge::mcptest::{TcpTransport, Transport};
use sts_bridge::{Backend, CombatView, GameState, McpTestBackend};

static GAME: Mutex<()> = Mutex::new(());

fn hold_game() -> MutexGuard<'static, ()> {
    GAME.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn live_backend() -> McpTestBackend<TcpTransport> {
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

fn live_state(screen_hint: &str) -> GameState {
    live_backend()
        .state()
        .unwrap_or_else(|e| panic!("state() failed ({screen_hint}): {e}"))
}

fn combat_or_panic(state: GameState) -> CombatView {
    assert!(
        state.screen.as_str() == "COMBAT_PLAYER_TURN",
        "precondition: the game must be on the player's turn in a combat, but the screen is {}",
        state.screen
    );
    state
        .combat
        .expect("a combat screen must carry a combat view")
}

#[test]
#[ignore = "live: needs STS2 + MCPTest bridge"]
fn health() {
    let _game = hold_game();
    let health = live_backend().health().unwrap();
    assert_eq!(health.backend, "MCPTest");
    assert!(health.latency_ms >= 0.0);
    assert!(
        health.screen.is_some(),
        "ping should report the screen: {health:?}"
    );
}

#[test]
#[ignore = "live: needs STS2 + MCPTest bridge"]
fn main_menu() {
    let _game = hold_game();
    let state = live_state("main menu");
    assert_eq!(
        state.screen.as_str(),
        "MAIN_MENU",
        "precondition: the game must be at the main menu"
    );
    assert_eq!(
        state.run, None,
        "no run may be in progress at the main menu"
    );
    assert_eq!(state.combat, None);

    let actions = live_backend().actions().unwrap();
    assert_eq!(actions.screen, state.screen);
}

#[test]
#[ignore = "live: needs STS2 + MCPTest bridge"]
fn combat() {
    let _game = hold_game();
    let state = live_state("combat");
    let run = state
        .run
        .clone()
        .expect("a run is in progress during combat");
    let c = combat_or_panic(state);

    assert!(run.act >= 1 && run.floor >= 1, "{run:?}");
    assert!(!run.seed.is_empty());
    assert_eq!(
        run.hp, c.player.hp,
        "run hp and combat player hp are the same creature"
    );

    assert!(c.round >= 1);
    assert!(c.is_player_turn);
    assert!(
        0 < c.player.hp && c.player.hp <= c.player.max_hp,
        "{:?}",
        c.player
    );
    assert!(
        c.player.block >= 0 && c.player.energy >= 0 && c.player.max_energy > 0,
        "{:?}",
        c.player
    );

    assert!(!c.enemies.is_empty(), "a combat has enemies");
    for (i, e) in c.enemies.iter().enumerate() {
        assert_eq!(e.index, i, "enemy indices are contiguous");
        assert!(
            !e.entity_id.is_empty() && !e.name.name().is_empty(),
            "{e:?}"
        );
        assert!(e.hp <= e.max_hp && e.max_hp > 0 && e.block >= 0, "{e:?}");
        if e.is_alive {
            let intent = e
                .intent
                .as_ref()
                .unwrap_or_else(|| panic!("a living enemy has an intent: {e:?}"));
            assert!(
                !intent.move_id.is_empty() && !intent.parts.is_empty(),
                "{e:?}"
            );
        }
    }
    for p in c
        .player
        .powers
        .iter()
        .chain(c.enemies.iter().flat_map(|e| &e.powers))
    {
        assert!(!p.id.name().is_empty(), "{p:?}");
    }

    let hand = &c.piles.hand;
    for (i, card) in hand.iter().enumerate() {
        assert_eq!(card.index, i, "hand indices are contiguous");
        assert!(
            !card.id.name().is_empty() && !card.kind.is_empty() && !card.target_type.is_empty(),
            "{card:?}"
        );
        if card.playable && card.target_type == "AnyEnemy" {
            let targets = card
                .valid_targets
                .as_ref()
                .unwrap_or_else(|| panic!("a playable targeted card lists targets: {card:?}"));
            assert!(!targets.is_empty(), "{card:?}");
            for &t in targets {
                assert!(
                    t < c.enemies.len() && c.enemies[t].is_alive,
                    "target {t} out of range or dead: {card:?}"
                );
            }
        }
        if !card.playable {
            assert!(
                card.unplayable_reason.is_some(),
                "an unplayable card says why: {card:?}"
            );
        }
    }
    for card in c
        .piles
        .draw
        .iter()
        .chain(&c.piles.discard)
        .chain(&c.piles.exhaust)
    {
        assert!(
            !card.id.name().is_empty() && !card.kind.is_empty(),
            "{card:?}"
        );
    }
    assert!(
        !(hand.is_empty() && c.piles.draw.is_empty() && c.piles.discard.is_empty()),
        "a combat has cards somewhere"
    );
}

/// Expected to fail until HOL-102 fixes the bridge: multi-hit intents report
/// `total_damage` equal to a single hit. When this starts failing with "did
/// not panic", the bridge is fixed; drop `should_panic`.
#[test]
#[ignore = "live: needs STS2 + MCPTest bridge"]
#[should_panic(expected = "intent total_damage mismatch")]
fn combat_intent_totals_are_consistent() {
    let _game = hold_game();
    let c = combat_or_panic(live_state("combat"));
    let attacks: Vec<_> = c
        .enemies
        .iter()
        .filter_map(|e| e.intent.as_ref().map(|i| (e, i)))
        .flat_map(|(e, i)| i.parts.iter().map(move |p| (e, p)))
        .filter(|(_, p)| p.kind == "Attack")
        .collect();
    assert!(
        attacks.iter().any(|(_, p)| p.hits.unwrap_or(1) > 1),
        "precondition: needs an enemy with a multi-hit attack intent"
    );
    for (e, p) in attacks {
        let (damage, hits, total) = (
            p.damage.expect("attack damage"),
            p.hits.unwrap_or(1),
            p.total_damage.expect("attack total_damage"),
        );
        assert!(
            damage * hits == total,
            "intent total_damage mismatch on {}: damage {damage} x hits {hits} != total_damage {total}",
            e.entity_id
        );
    }
}

/// Records the current screen into `tests/fixtures/mcptest/recorded/<screen>/`:
/// `STS2_RECORD=1 cargo test -p sts-bridge --test live_contract record_fixtures -- --ignored`.
#[test]
#[ignore = "live: records fixtures from STS2"]
fn record_fixtures() {
    if std::env::var_os("STS2_RECORD").is_none() {
        panic!("set STS2_RECORD=1 to write tests/fixtures/mcptest/recorded from the live game");
    }
    let _game = hold_game();
    let state = live_state("record");
    let (dir, methods): (&str, &[&str]) = if state.screen.as_str() == "MAIN_MENU" {
        (
            "main_menu",
            &["ping", "get_available_actions", "get_run_state"],
        )
    } else if state.screen.is_combat() {
        (
            "combat",
            &[
                "ping",
                "get_available_actions",
                "get_run_state",
                "get_combat_state",
                "get_card_piles",
            ],
        )
    } else {
        panic!(
            "recording needs the main menu or a combat, but the screen is {}",
            state.screen
        );
    };
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mcptest/recorded")
        .join(dir);
    std::fs::create_dir_all(&out).unwrap();
    let mut transport = TcpTransport::from_env();
    for method in methods {
        let raw = transport.send(method, None).unwrap();
        std::fs::write(out.join(format!("{method}.json")), raw).unwrap();
    }
}
