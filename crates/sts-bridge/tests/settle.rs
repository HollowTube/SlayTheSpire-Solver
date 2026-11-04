//! Write commands and settling over a scripted bridge; no game needed.

mod common;

use std::time::{Duration, Instant};

use common::{as_pile_card, WireState};
use serde_json::{json, Value};
use sts_bridge::mcptest::{ReplayTransport, Request, Transport};
use sts_bridge::{
    AvailableAction, Backend, BridgeError, CardId, Command, Entity, GameState, McpTestBackend,
    Session, SettlePolicy, StatusId, Unmet,
};

type Scripted = Session<McpTestBackend<ReplayTransport>>;

const COMMANDS: [&str; 3] = ["play_card", "end_turn", "execute_action"];

/// Fast polling, short deadline: the scripted game answers instantly.
fn policy() -> SettlePolicy {
    SettlePolicy::default()
        .with_timeout(Duration::from_millis(150))
        .with_poll(Duration::from_millis(1), Duration::from_millis(5))
}

fn session(t: ReplayTransport) -> Scripted {
    Session::with_policy(McpTestBackend::new(t), policy())
}

fn methods(s: &Scripted) -> Vec<String> {
    s.backend()
        .transport()
        .requests()
        .iter()
        .map(|r| r.method.clone())
        .collect()
}

fn commands_sent(s: &Scripted) -> Vec<String> {
    methods(s)
        .into_iter()
        .filter(|m| COMMANDS.contains(&m.as_str()))
        .collect()
}

/// How many `state()` reads happened (each starts with this call).
fn reads(s: &Scripted) -> usize {
    methods(s)
        .iter()
        .filter(|m| *m == "get_available_actions")
        .count()
}

fn enemy_turn() -> WireState {
    WireState::combat()
        .screen("COMBAT_ENEMY_TURN")
        .edit(|w| w.combat["is_player_turn"] = json!(false))
}

/// The `combat` fixture with enemy 0 at full HP and no powers.
fn fresh_combat() -> WireState {
    WireState::combat().edit(|w| {
        w.enemy(0)["hp"] = json!(44);
        w.enemy(0)["powers"] = json!([]);
    })
}

const BASH: usize = 2;
const WOUND: usize = 4;

fn bash_enemy_0() -> Command {
    Command::PlayCard {
        card_index: BASH,
        target: Some(0),
    }
}

/// Bash has left the hand and spent its energy but is in no pile yet, and
/// enemy 0 is untouched: what the bridge shows while the card resolves.
fn bash_in_flight() -> WireState {
    fresh_combat().edit(|w| {
        w.hand().remove(BASH);
        w.player()["energy"] = json!(1);
    })
}

fn bash_landed() -> WireState {
    let bash = fresh_combat().hand()[BASH].clone();
    bash_in_flight().edit(|w| {
        w.enemy(0)["hp"] = json!(34);
        w.enemy(0)["powers"] = json!([{"name": "VulnerablePower", "amount": 3, "type": "Debuff"}]);
        w.pile("discard_pile").push(as_pile_card(&bash));
    })
}

fn sent(command: &Command, reply: &str) -> (Result<(), BridgeError>, Vec<Request>) {
    let mut t = ReplayTransport::new();
    for method in ["play_card", "end_turn", "execute_action"] {
        t.insert(method, reply);
    }
    let mut backend = McpTestBackend::new(t);
    let result = backend.send(command).map(|_| ());
    (result, backend.transport().requests().to_vec())
}

fn request(method: &str, params: Option<Value>) -> Request {
    Request {
        method: method.into(),
        params,
    }
}

const OK: &str = r#"{"result":{"success":true},"id":1}"#;

#[test]
fn play_card_sends_the_card_and_target() {
    let cmd = Command::PlayCard {
        card_index: 2,
        target: Some(1),
    };
    let (result, log) = sent(&cmd, OK);
    result.unwrap();
    assert_eq!(
        log,
        [request(
            "play_card",
            Some(json!({"card_index": 2, "target_index": 1}))
        )]
    );
}

#[test]
fn untargeted_play_card_sends_no_target() {
    let cmd = Command::PlayCard {
        card_index: 1,
        target: None,
    };
    let (_, log) = sent(&cmd, OK);
    assert_eq!(log, [request("play_card", Some(json!({"card_index": 1})))]);
}

#[test]
fn end_turn_sends_end_turn() {
    let (result, log) = sent(
        &Command::EndTurn,
        r#"{"result":{"success":true,"round":2}}"#,
    );
    result.unwrap();
    assert_eq!(log, [request("end_turn", None)]);
}

#[test]
fn choose_sends_the_action_name_and_its_params_through_execute_action() {
    let action: AvailableAction = serde_json::from_value(json!({
        "action": "map_travel", "label": "Monster", "row": 1, "col": 3
    }))
    .unwrap();
    let (result, log) = sent(&Command::Choose(action), OK);
    result.unwrap();
    assert_eq!(
        log,
        [request(
            "execute_action",
            Some(json!({"action": "map_travel", "row": 1, "col": 3}))
        )]
    );
}

#[test]
fn a_bridge_error_on_a_command_is_action_refused() {
    let cmd = Command::PlayCard {
        card_index: 4,
        target: None,
    };
    let (result, _) = sent(
        &cmd,
        r#"{"result":{"error":"Card Wound cannot be played: Unplayable"},"id":1}"#,
    );
    match result {
        Err(BridgeError::ActionRefused { action, reason }) => {
            assert!(action.contains("play_card"), "{action}");
            assert_eq!(reason, "Card Wound cannot be played: Unplayable");
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
}

#[test]
fn a_request_level_bridge_failure_stays_remote() {
    let (result, _) = sent(
        &Command::EndTurn,
        r#"{"error":"Main thread dispatch timed out after 10s","id":0}"#,
    );
    assert!(
        matches!(result, Err(BridgeError::Remote { .. })),
        "the command may still run: {result:?}"
    );
}

#[test]
fn success_false_is_action_refused() {
    let cmd = Command::PlayCard {
        card_index: 0,
        target: Some(0),
    };
    let (result, _) = sent(
        &cmd,
        r#"{"result":{"success":false,"card":"StrikeIronclad"},"id":1}"#,
    );
    match result {
        Err(BridgeError::ActionRefused { reason, .. }) => {
            assert!(reason.contains("success: false"), "{reason}")
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
}

#[test]
fn queued_replies_are_served_in_order_and_the_last_one_sticks() {
    let mut t = ReplayTransport::new();
    t.push("ping", "a").push("ping", "b");
    assert_eq!(t.send("ping", None).unwrap(), b"a");
    assert_eq!(t.send("ping", None).unwrap(), b"b");
    assert_eq!(t.send("ping", None).unwrap(), b"b");
}

#[test]
fn insert_replaces_the_queue_with_one_sticky_reply() {
    let mut t = ReplayTransport::new();
    t.push("ping", "a").push("ping", "b").insert("ping", "c");
    assert_eq!(t.get("ping"), Some(&b"c"[..]));
    assert_eq!(t.send("ping", None).unwrap(), b"c");
    assert_eq!(t.send("ping", None).unwrap(), b"c");
}

#[test]
fn every_request_is_logged_with_its_params() {
    let mut t = ReplayTransport::new();
    t.insert("play_card", "{}");
    let params = json!({"card_index": 2});
    t.send("play_card", Some(&params)).unwrap();
    let _ = t.send("missing", None);
    assert_eq!(
        t.requests(),
        &[
            Request {
                method: "play_card".into(),
                params: Some(params)
            },
            Request {
                method: "missing".into(),
                params: None
            },
        ]
    );
}

// ---- wait_until / wait_settled ----

#[test]
fn settled_needs_identical_consecutive_reads() {
    let mut t = ReplayTransport::new();
    fresh_combat().push(&mut t);
    bash_in_flight().push(&mut t); // sticky from here on
    let mut s = session(t);
    let state = s.wait_settled().unwrap();
    assert_eq!(state.combat.unwrap().player.energy, 1);
    assert_eq!(reads(&s), 3, "one read of the old state, two of the new");
}

#[test]
fn the_enemy_turn_and_loading_are_never_settled() {
    let mut t = ReplayTransport::new();
    enemy_turn().push_n(&mut t, 3);
    WireState::out_of_combat("COMBAT_LOADING").push_n(&mut t, 3);
    fresh_combat().push(&mut t);
    let mut s = session(t);
    let state = s.wait_settled().unwrap();
    assert_eq!(state.screen.as_str(), "COMBAT_PLAYER_TURN");
    assert_eq!(reads(&s), 8);
}

#[test]
fn a_wait_that_never_settles_is_not_settled_naming_the_screen() {
    let mut t = ReplayTransport::new();
    enemy_turn().push(&mut t);
    let mut s = session(t);
    let started = Instant::now();
    match s.wait_settled() {
        Err(BridgeError::NotSettled {
            screen,
            unmet,
            waited,
        }) => {
            assert_eq!(screen.unwrap().as_str(), "COMBAT_ENEMY_TURN");
            assert!(unmet.contains("interactive"), "{unmet}");
            assert!(waited >= Duration::from_millis(150), "{waited:?}");
        }
        other => panic!("expected NotSettled, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_stable_state_returns_after_one_short_confirming_pause() {
    let mut t = ReplayTransport::new();
    fresh_combat().push(&mut t);
    let mut s = Session::with_policy(
        McpTestBackend::new(t),
        SettlePolicy::default()
            .with_timeout(Duration::from_secs(10))
            .with_poll(Duration::from_millis(50), Duration::from_secs(5)),
    );
    let started = Instant::now();
    s.wait_settled().unwrap();
    let elapsed = started.elapsed();
    assert!(
        Duration::from_millis(50) <= elapsed && elapsed < Duration::from_millis(500),
        "{elapsed:?}"
    );
    assert_eq!(reads(&s), 2);
}

#[test]
fn wait_until_polls_any_predicate() {
    let mut t = ReplayTransport::from_dir(common::fixture_dir("main_menu")).unwrap();
    let main_menu = t.get("get_available_actions").unwrap().to_vec();
    let no_run = t.get("get_run_state").unwrap().to_vec();
    // Leaving a run: the map, then the main menu.
    WireState::out_of_combat("MAP").push_n(&mut t, 2);
    t.push("get_available_actions", main_menu)
        .push("get_run_state", no_run);
    let mut s = session(t);
    let state = s
        .wait_until("the main menu with no run", |s: &GameState| {
            s.screen.as_str() == "MAIN_MENU" && s.run.is_none()
        })
        .unwrap();
    assert_eq!(state.screen.as_str(), "MAIN_MENU");
}

#[test]
fn poll_until_works_over_any_observation() {
    let mut t = ReplayTransport::new();
    for screen in ["MAP", "COMBAT_LOADING", "EVENT"] {
        t.push(
            "get_available_actions",
            json!({"result": {"screen": screen, "actions": []}}).to_string(),
        );
    }
    let mut backend = McpTestBackend::new(t);
    let actions = sts_bridge::poll_until(
        &policy().with_stable_reads(1),
        "the event screen",
        || backend.actions(),
        |a| a.screen.as_str() == "EVENT",
    )
    .unwrap();
    assert_eq!(actions.screen.as_str(), "EVENT");
}

// ---- act(PlayCard) ----

#[test]
fn play_card_returns_only_once_the_card_landed_and_its_effects_show() {
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    bash_in_flight().push_n(&mut t, 4);
    bash_landed().push(&mut t);
    t.insert("play_card", OK);
    let mut s = session(t);

    let state = s.act(&bash_enemy_0()).unwrap();

    let combat = state.combat.unwrap();
    let enemy = &combat.enemies[0];
    assert_eq!(enemy.hp, 34);
    assert_eq!(enemy.powers[0].id, Entity::Known(StatusId::Vulnerable));
    assert!(combat
        .piles
        .discard
        .iter()
        .any(|c| c.id == Entity::Known(CardId::Bash)));
    assert_eq!(commands_sent(&s), ["play_card"]);
    assert_eq!(
        reads(&s),
        2 + 4 + 2,
        "kept polling through the in-flight reads"
    );
}

#[test]
fn an_unplayable_card_is_refused_without_sending() {
    let mut t = ReplayTransport::new();
    fresh_combat().push(&mut t);
    let mut s = session(t);
    let cmd = Command::PlayCard {
        card_index: WOUND,
        target: None,
    };
    match s.act(&cmd) {
        Err(BridgeError::ActionRefused { reason, .. }) => {
            assert!(
                reason.contains("Wound") && reason.contains("Unplayable"),
                "{reason}"
            )
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
    assert!(commands_sent(&s).is_empty());
}

#[test]
fn a_bad_target_or_index_is_refused_without_sending() {
    for (card_index, target, needle) in [
        (BASH, Some(5), "not a valid target"),
        (BASH, None, "needs a target"),
        (1, Some(0), "takes no target"),
        (9, None, "out of range"),
    ] {
        let mut t = ReplayTransport::new();
        fresh_combat().push(&mut t);
        let mut s = session(t);
        match s.act(&Command::PlayCard { card_index, target }) {
            Err(BridgeError::ActionRefused { reason, .. }) => {
                assert!(reason.contains(needle), "{reason}")
            }
            other => panic!("expected ActionRefused for {card_index}/{target:?}, got {other:?}"),
        }
        assert!(commands_sent(&s).is_empty());
    }
}

#[test]
fn a_refusal_from_the_bridge_is_action_refused() {
    let mut t = ReplayTransport::new();
    fresh_combat().push(&mut t);
    t.insert(
        "play_card",
        r#"{"result":{"success":false,"card":"Bash"},"id":1}"#,
    );
    let mut s = session(t);
    assert!(matches!(
        s.act(&bash_enemy_0()),
        Err(BridgeError::ActionRefused { .. })
    ));
}

#[test]
fn a_play_the_game_silently_cancels_is_a_postcondition_failure() {
    let mut t = ReplayTransport::new();
    fresh_combat().push(&mut t);
    t.insert("play_card", OK);
    let mut s = session(t);
    match s.act(&bash_enemy_0()) {
        Err(BridgeError::PostconditionFailed {
            action,
            expected,
            observed,
        }) => {
            assert!(action.contains("play_card"), "{action}");
            assert!(expected.contains("Bash"), "{expected}");
            assert!(observed.contains("in hand 1"), "{observed}");
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
}

#[test]
fn a_play_that_never_settles_is_not_settled() {
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    WireState::out_of_combat("COMBAT_LOADING").push(&mut t);
    t.insert("play_card", OK);
    let mut s = session(t);
    match s.act(&bash_enemy_0()) {
        Err(BridgeError::NotSettled { screen, unmet, .. }) => {
            assert_eq!(screen.unwrap().as_str(), "COMBAT_LOADING");
            assert!(unmet.contains("interactive"), "{unmet}");
        }
        other => panic!("expected NotSettled, got {other:?}"),
    }
}

#[test]
fn a_power_card_leaves_combat_instead_of_landing_in_a_pile() {
    let inflame = |w: &mut WireState| {
        let card = &mut w.hand()[1];
        card["name"] = json!("Inflame");
        card["type"] = json!("Power");
    };
    let mut t = ReplayTransport::new();
    fresh_combat().edit(inflame).push_n(&mut t, 2);
    fresh_combat()
        .edit(|w| {
            w.hand().remove(1);
            w.player()["powers"] = json!([{"name": "StrengthPower", "amount": 3, "type": "Buff"}]);
        })
        .push(&mut t);
    t.insert("play_card", OK);
    let mut s = session(t);
    let cmd = Command::PlayCard {
        card_index: 1,
        target: None,
    };
    let state = s.act(&cmd).unwrap();
    assert_eq!(state.combat.unwrap().player.powers[0].amount, 3);
}

#[test]
fn a_play_that_draws_its_own_copy_still_counts() {
    let bash = fresh_combat().hand()[BASH].clone();
    let with_bash_on_draw = |w: &mut WireState| w.pile("draw_pile").push(as_pile_card(&bash));
    let mut t = ReplayTransport::new();
    fresh_combat().edit(with_bash_on_draw).push_n(&mut t, 2);
    bash_in_flight().edit(with_bash_on_draw).push(&mut t);
    // Landed in discard; the draw pile's copy is now in hand.
    bash_landed()
        .edit(|w| {
            let mut drawn = bash.clone();
            drawn["index"] = json!(4);
            w.hand().push(drawn);
        })
        .push(&mut t);
    t.insert("play_card", OK);
    let state = session(t).act(&bash_enemy_0()).unwrap();
    assert_eq!(state.combat.unwrap().enemies[0].hp, 34);
}

#[test]
fn a_play_that_copies_itself_still_counts() {
    let bash = fresh_combat().hand()[BASH].clone();
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    bash_landed()
        .edit(|w| w.pile("discard_pile").push(as_pile_card(&bash)))
        .push(&mut t);
    t.insert("play_card", OK);
    session(t).act(&bash_enemy_0()).unwrap();
}

#[test]
fn a_play_that_opens_a_hand_selection_returns_on_it() {
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    bash_in_flight().screen("HAND_SELECT").push(&mut t);
    t.insert("play_card", OK);
    let state = session(t).act(&bash_enemy_0()).unwrap();
    assert_eq!(state.screen.as_str(), "HAND_SELECT");
}

#[test]
fn a_killing_blow_ends_the_combat_and_still_counts() {
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    WireState::out_of_combat("REWARD").push(&mut t);
    t.insert("play_card", OK);
    let mut s = session(t);
    let state = s.act(&bash_enemy_0()).unwrap();
    assert_eq!(state.screen.as_str(), "REWARD");
    assert_eq!(state.combat, None);
}

#[test]
fn commands_strictly_alternate_with_settled_reads() {
    let strike_landed = |w: &mut WireState| {
        let strike = w.hand()[0].clone();
        w.hand().remove(0);
        w.pile("discard_pile").push(as_pile_card(&strike));
        w.player()["energy"] = json!(0);
    };
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    bash_in_flight().push(&mut t);
    bash_landed().push_n(&mut t, 4);
    bash_landed().edit(strike_landed).push(&mut t);
    t.insert("play_card", OK);
    let mut s = session(t);

    s.act(&bash_enemy_0()).unwrap();
    s.act(&Command::PlayCard {
        card_index: 0,
        target: Some(0),
    })
    .unwrap();

    let log = methods(&s);
    let sends: Vec<usize> = log
        .iter()
        .enumerate()
        .filter(|(_, m)| *m == "play_card")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(sends.len(), 2, "{log:?}");
    let reads_between = log[sends[0]..sends[1]]
        .iter()
        .filter(|m| *m == "get_available_actions")
        .count();
    assert!(
        reads_between >= 2,
        "the second command waited for a stable read: {log:?}"
    );
    assert!(
        log[..sends[0]]
            .iter()
            .filter(|m| *m == "get_available_actions")
            .count()
            >= 2
    );
}

#[test]
fn act_with_adds_a_caller_postcondition() {
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    bash_landed().push(&mut t);
    t.insert("play_card", OK);
    let mut s = session(t);
    let result = s.act_with(&bash_enemy_0(), |before, after| {
        let hp = |s: &GameState| s.combat.as_ref().unwrap().enemies[0].hp;
        if hp(after) == hp(before) - 12 {
            Ok(())
        } else {
            Err(Unmet::new(
                format!("enemy 0 hp {}", hp(before) - 12),
                format!("enemy 0 hp {}", hp(after)),
            ))
        }
    });
    match result {
        Err(BridgeError::PostconditionFailed {
            expected, observed, ..
        }) => {
            assert_eq!(expected, "enemy 0 hp 32");
            assert_eq!(observed, "enemy 0 hp 34");
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
}

// ---- act(EndTurn) / act(Choose) ----

#[test]
fn end_turn_returns_on_the_next_player_turn() {
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    enemy_turn().push_n(&mut t, 3);
    fresh_combat()
        .edit(|w| w.combat["round"] = json!(3))
        .push(&mut t);
    t.insert(
        "end_turn",
        r#"{"result":{"success":true,"round":2},"id":1}"#,
    );
    let mut s = session(t);
    let state = s.act(&Command::EndTurn).unwrap();
    assert_eq!(state.screen.as_str(), "COMBAT_PLAYER_TURN");
    assert_eq!(state.combat.unwrap().round, 3);
    assert_eq!(commands_sent(&s), ["end_turn"]);
}

#[test]
fn end_turn_counts_the_combat_ending() {
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    enemy_turn().push(&mut t);
    WireState::out_of_combat("GAME_OVER").push(&mut t);
    t.insert("end_turn", OK);
    let state = session(t).act(&Command::EndTurn).unwrap();
    assert_eq!(state.screen.as_str(), "GAME_OVER");
}

#[test]
fn end_turn_outside_the_player_turn_is_refused() {
    let mut t = ReplayTransport::new();
    WireState::out_of_combat("MAP").push(&mut t);
    let mut s = session(t);
    assert!(matches!(
        s.act(&Command::EndTurn),
        Err(BridgeError::ActionRefused { .. })
    ));
    assert!(commands_sent(&s).is_empty());
}

fn map_travel() -> AvailableAction {
    serde_json::from_value(json!({"action": "map_travel", "row": 1, "col": 3})).unwrap()
}

fn on_map() -> WireState {
    WireState::out_of_combat("MAP").edit(|w| {
        w.actions["actions"] = json!([{"action": "map_travel", "row": 1, "col": 3}]);
    })
}

/// A state as `act(Choose)` reads it: the state, then the actions again.
fn push_viewed(w: &WireState, t: &mut ReplayTransport) {
    w.push(t);
    t.push(
        "get_available_actions",
        json!({"result": w.actions}).to_string(),
    );
}

#[test]
fn choose_returns_once_the_screen_changed() {
    let mut t = ReplayTransport::new();
    on_map().push_n(&mut t, 2);
    t.push(
        "get_available_actions",
        json!({"result": on_map().actions}).to_string(),
    );
    push_viewed(&on_map(), &mut t);
    push_viewed(&WireState::out_of_combat("COMBAT_LOADING"), &mut t);
    push_viewed(&fresh_combat(), &mut t);
    t.insert("execute_action", OK);
    let mut s = session(t);
    let state = s.act(&Command::Choose(map_travel())).unwrap();
    assert_eq!(state.screen.as_str(), "COMBAT_PLAYER_TURN");
    assert_eq!(commands_sent(&s), ["execute_action"]);
}

#[test]
fn a_choice_with_no_effect_is_a_postcondition_failure() {
    let mut t = ReplayTransport::new();
    on_map().push_n(&mut t, 2);
    t.push(
        "get_available_actions",
        json!({"result": on_map().actions}).to_string(),
    );
    push_viewed(&on_map(), &mut t);
    t.insert("execute_action", OK);
    match session(t).act(&Command::Choose(map_travel())) {
        Err(BridgeError::PostconditionFailed { observed, .. }) => {
            assert!(observed.contains("MAP"), "{observed}")
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
}

#[test]
fn a_choice_not_on_offer_is_refused_without_sending() {
    let mut t = ReplayTransport::new();
    WireState::out_of_combat("MAP").push_n(&mut t, 2);
    let mut s = session(t);
    match s.act(&Command::Choose(map_travel())) {
        Err(BridgeError::ActionRefused { reason, .. }) => {
            assert!(reason.contains("not on offer"), "{reason}")
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
    assert!(commands_sent(&s).is_empty());
}

#[test]
fn choosing_a_listed_card_play_is_a_checked_play_card() {
    let mut t = ReplayTransport::new();
    fresh_combat().push_n(&mut t, 2);
    bash_landed().push(&mut t);
    t.insert("play_card", OK);
    let mut s = session(t);
    let listed: AvailableAction = serde_json::from_value(json!({
        "action": "play_card", "card_index": BASH, "target_index": 0, "card_name": "Bash"
    }))
    .unwrap();
    s.act(&Command::Choose(listed)).unwrap();
    assert_eq!(commands_sent(&s), ["play_card"]);
}
