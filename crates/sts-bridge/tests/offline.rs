//! `Backend` behaviour over checked-in bridge responses; no game needed.

use std::path::PathBuf;

use serde_json::Value;
use sts_bridge::mcptest::ReplayTransport;
use sts_bridge::{schema, Backend, BridgeError, Entity, GameState, McpTestBackend};
use sts_ids::{CardId, MonsterId, StatusId};

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mcptest")
        .join(name)
}

fn replay(name: &str) -> ReplayTransport {
    ReplayTransport::from_dir(fixture_dir(name)).expect("fixture dir readable")
}

fn backend(name: &str) -> McpTestBackend<ReplayTransport> {
    McpTestBackend::new(replay(name))
}

fn combat_state() -> GameState {
    backend("combat").state().expect("combat fixture parses")
}

#[test]
fn main_menu_state_has_no_run_or_combat() {
    let state = backend("main_menu").state().unwrap();
    assert_eq!(state.screen.as_str(), "MAIN_MENU");
    assert_eq!(state.run, None);
    assert_eq!(state.combat, None);
}

#[test]
fn combat_state_populates_run_enemies_hand_and_piles() {
    let state = combat_state();
    assert_eq!(state.screen.as_str(), "COMBAT_PLAYER_TURN");

    let run = state.run.expect("run info in combat");
    assert_eq!(
        (run.act, run.floor, run.hp, run.max_hp, run.gold),
        (1, 2, 70, 80, 99)
    );
    assert_eq!(run.character.as_deref(), Some("Ironclad"));
    assert_eq!(run.seed, "SOLVER01");

    let combat = state.combat.expect("combat view in combat");
    assert_eq!((combat.round, combat.is_player_turn), (2, true));
    assert_eq!(
        (
            combat.player.hp,
            combat.player.energy,
            combat.player.max_energy,
            combat.player.block
        ),
        (70, 3, 3, 3)
    );
    assert_eq!(
        combat.player.powers[0].id,
        Entity::Known(StatusId::Strength)
    );

    assert_eq!(combat.enemies.len(), 2);
    let first = &combat.enemies[0];
    assert_eq!(
        (first.entity_id.as_str(), first.hp, first.max_hp),
        ("Nibbit_0", 38, 44)
    );
    assert_eq!(first.name, Entity::Known(MonsterId::Nibbit));
    assert_eq!(first.powers[0].id, Entity::Known(StatusId::Vulnerable));
    assert_eq!(first.powers[0].amount, 2);
    let intent = combat.enemies[1].intent.as_ref().expect("intent");
    assert_eq!(intent.move_id, "SLICE_AND_HISS");
    assert_eq!(
        (
            intent.parts[0].damage,
            intent.parts[0].hits,
            intent.parts[0].total_damage
        ),
        (Some(6), Some(2), Some(12))
    );
    assert_eq!(intent.parts[1].kind, "Buff");
    assert_eq!(intent.parts[1].damage, None);

    let hand = &combat.piles.hand;
    assert_eq!(hand.len(), 5);
    assert_eq!(
        (&hand[2].id, hand[2].cost, hand[2].upgraded),
        (&Entity::Known(CardId::Bash), 2, true)
    );
    assert_eq!(hand[0].valid_targets.as_deref(), Some(&[0, 1][..]));
    assert_eq!(hand[1].valid_targets, None);
    assert!(!hand[4].playable);
    assert_eq!(hand[4].unplayable_reason.as_deref(), Some("Unplayable"));

    assert_eq!(combat.piles.draw.len(), 3);
    assert_eq!(combat.piles.discard.len(), 2);
    assert_eq!(hand[0].id, Entity::Known(CardId::StrikeIronclad));
    assert_eq!(
        combat.piles.exhaust[0].id,
        Entity::Known(CardId::DefendIronclad)
    );
    assert!(combat.piles.exhaust[0].upgraded);
}

#[test]
fn combat_state_round_trips_through_serde() {
    let state = combat_state();
    let json = serde_json::to_string(&state).unwrap();
    let back: GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, state);
}

#[test]
fn unwrapped_and_double_encoded_replies_give_the_same_state() {
    let expected = combat_state();
    assert_eq!(backend("combat_unwrapped").state().unwrap(), expected);
    assert_eq!(backend("combat_double_encoded").state().unwrap(), expected);
}

#[test]
fn actions_carry_screen_and_typed_entries() {
    let actions = backend("combat").actions().unwrap();
    assert_eq!(actions.screen.as_str(), "COMBAT_PLAYER_TURN");
    let play = &actions.actions[1];
    assert_eq!(play.action, "play_card");
    assert_eq!((play.card_index, play.target_index), (Some(0), Some(1)));
    assert!(actions.actions.iter().any(|a| a.action == "end_turn"));

    let menu = backend("main_menu").actions().unwrap();
    assert_eq!(menu.screen.as_str(), "MAIN_MENU");
    assert_eq!(menu.actions[0].action, "console");
}

#[test]
fn health_reports_ping_fields() {
    let health = backend("main_menu").health().unwrap();
    assert!(health.ok);
    assert_eq!(health.backend, "MCPTest");
    assert_eq!(health.version.as_deref(), Some("2.0.0"));
    assert_eq!(health.run_in_progress, Some(false));
}

fn with_combat_state_edit(edit: impl FnOnce(&mut Value)) -> McpTestBackend<ReplayTransport> {
    let mut transport = replay("combat");
    let mut envelope: Value =
        serde_json::from_slice(transport.get("get_combat_state").unwrap()).unwrap();
    edit(&mut envelope["result"]);
    transport.insert("get_combat_state", envelope.to_string());
    McpTestBackend::new(transport)
}

#[test]
fn entities_the_registry_does_not_know_are_unknown_not_errors() {
    let mut backend = with_combat_state_edit(|r| {
        r["enemies"][0]["name"] = Value::from("GremlinWizard");
        r["enemies"][0]["powers"][0]["name"] = Value::from("MysteryPower");
        r["players"][0]["hand"][1]["name"] = Value::from("Defend");
    });
    let combat = backend
        .state()
        .expect("unknown names still parse")
        .combat
        .unwrap();
    let enemy = &combat.enemies[0];
    assert_eq!(enemy.name, Entity::Unknown("GremlinWizard".into()));
    assert_eq!(enemy.powers[0].id, Entity::Unknown("MysteryPower".into()));
    assert_eq!(combat.piles.hand[1].id, Entity::Unknown("Defend".into()));
    assert_eq!(enemy.name.name(), "GremlinWizard");
    assert_eq!(combat.enemies[1].name, Entity::Known(MonsterId::Nibbit));
}

#[test]
fn known_entities_serialize_as_their_canonical_bridge_name() {
    let combat = combat_state().combat.unwrap();
    let json = serde_json::to_value(&combat).unwrap();
    assert_eq!(json["enemies"][0]["name"], "Nibbit");
    assert_eq!(json["enemies"][0]["powers"][0]["id"], "Vulnerable");
    assert_eq!(json["piles"]["hand"][0]["id"], "StrikeIronclad");
}

#[test]
fn missing_required_field_is_a_schema_error_naming_the_path() {
    let mut backend = with_combat_state_edit(|r| {
        r["players"][0].as_object_mut().unwrap().remove("energy");
    });
    match backend.state() {
        Err(BridgeError::Schema {
            method,
            path,
            message,
        }) => {
            assert_eq!(method, "get_combat_state");
            assert_eq!(path, "players[0]");
            assert!(message.contains("energy"), "{message}");
        }
        other => panic!("expected a Schema error, got {other:?}"),
    }
}

#[test]
fn mistyped_field_is_a_schema_error_naming_the_path() {
    let mut backend = with_combat_state_edit(|r| r["enemies"][1]["hp"] = Value::from("44"));
    match backend.state() {
        Err(BridgeError::Schema { path, .. }) => assert_eq!(path, "enemies[1].hp"),
        other => panic!("expected a Schema error, got {other:?}"),
    }
}

#[test]
fn remote_error_in_a_sub_call_fails_state() {
    let mut transport = replay("combat");
    transport.insert(
        "get_card_piles",
        r#"{"result":{"error":"Not in combat"},"id":4}"#,
    );
    match McpTestBackend::new(transport).state() {
        Err(BridgeError::Remote { method, message }) => {
            assert_eq!(method, "get_card_piles");
            assert_eq!(message, "Not in combat");
        }
        other => panic!("expected a Remote error, got {other:?}"),
    }
}

#[test]
fn top_level_error_is_a_remote_error() {
    let mut transport = replay("main_menu");
    transport.insert(
        "ping",
        r#"{"error":"Internal serialization failure","id":0}"#,
    );
    assert!(matches!(
        McpTestBackend::new(transport).health(),
        Err(BridgeError::Remote { .. })
    ));
}

#[test]
fn checked_in_schema_matches_generated() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema/game_state.schema.json");
    let generated = schema::game_state_schema_json();
    if std::env::var_os("UPDATE_SCHEMA").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &generated).unwrap();
    }
    let checked_in = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        checked_in == generated,
        "schema/game_state.schema.json is stale; regenerate with `UPDATE_SCHEMA=1 cargo test -p sts-bridge --test offline`"
    );
}

#[test]
fn parsed_fixtures_validate_against_the_schema() {
    let schema = serde_json::to_value(schema::game_state_schema()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for state in [combat_state(), backend("main_menu").state().unwrap()] {
        let instance = serde_json::to_value(&state).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&instance)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
    }
}

#[test]
fn in_combat_overlay_screen_still_reads_combat() {
    let mut transport = replay("combat");
    let mut envelope: Value =
        serde_json::from_slice(transport.get("get_available_actions").unwrap()).unwrap();
    envelope["result"]["screen"] = Value::from("HAND_SELECT");
    transport.insert("get_available_actions", envelope.to_string());
    let state = McpTestBackend::new(transport).state().unwrap();
    assert_eq!(state.screen.as_str(), "HAND_SELECT");
    assert_eq!(state.combat, combat_state().combat);
}

#[test]
fn in_run_outside_combat_has_run_but_no_combat() {
    let mut transport = replay("combat");
    transport.insert(
        "get_available_actions",
        r#"{"result":{"screen":"MAP","actions":[]},"id":1}"#,
    );
    transport.insert(
        "get_combat_state",
        r#"{"result":{"in_combat":false,"screen":"MAP"},"id":2}"#,
    );
    let state = McpTestBackend::new(transport).state().unwrap();
    assert_eq!(state.screen.as_str(), "MAP");
    assert!(state.run.is_some());
    assert_eq!(state.combat, None);
}

#[test]
fn json_rpc_envelope_with_null_error_is_unwrapped() {
    let mut transport = replay("main_menu");
    transport.insert(
        "ping",
        r#"{"jsonrpc":"2.0","result":{"status":"ok","mod":"MCPTest"},"error":null,"id":3}"#,
    );
    assert!(McpTestBackend::new(transport).health().unwrap().ok);
}

/// Live recordings (`record_fixtures`) land in `recorded/<screen>/`; whatever
/// is there must parse and match the screen it was recorded on.
#[test]
fn recorded_fixtures_parse() {
    for (screen, in_combat) in [("main_menu", false), ("combat", true)] {
        let dir = fixture_dir("recorded").join(screen);
        if !dir.is_dir() {
            continue;
        }
        let state = McpTestBackend::new(ReplayTransport::from_dir(&dir).unwrap())
            .state()
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        assert_eq!(state.combat.is_some(), in_combat, "{}", dir.display());
        assert_eq!(state.run.is_some(), in_combat, "{}", dir.display());
    }
}
