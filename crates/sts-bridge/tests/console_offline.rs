//! Typed console commands, settled and verified, over a scripted bridge; no
//! game needed.

mod common;

use std::time::Duration;

use common::{Scene, WireState};
use serde_json::{json, Value};
use sts_bridge::console::{catalog, slugify};
use sts_bridge::mcptest::{ReplayTransport, Request, Transport};
use sts_bridge::{
    Backend, BridgeError, CardId, CombatPile, ConsoleCommand, ConsoleWait, Encounter, GameState,
    McpTestBackend, PowerName, Relic, Session, SettlePolicy, Target, Timeouts, UnknownId,
    CONSOLE_TIMEOUT, NIBBITS_WEAK,
};

type Scripted = Session<McpTestBackend<ReplayTransport>>;

const OK: &str = r#"{"result":{"success":true,"command":"x"},"id":1}"#;

/// Fast polling, short deadline: the scripted game answers instantly.
fn policy() -> SettlePolicy {
    SettlePolicy::default()
        .with_timeout(Duration::from_millis(150))
        .with_poll(Duration::from_millis(1), Duration::from_millis(5))
}

fn wait() -> ConsoleWait {
    ConsoleWait::for_session(&policy())
        .with_settle(policy())
        .with_room_ready(Timeouts::uniform(
            Duration::from_millis(150),
            Duration::from_millis(1),
        ))
}

fn requests(s: &Scripted) -> Vec<Request> {
    s.backend().transport().requests().to_vec()
}

/// The `console` lines sent, in order.
fn console_lines(s: &Scripted) -> Vec<String> {
    requests(s)
        .into_iter()
        .filter(|r| r.method == "console")
        .map(|r| r.params.unwrap()["command"].as_str().unwrap().to_string())
        .collect()
}

fn has_state_reads(s: &Scripted) -> bool {
    requests(s)
        .iter()
        .any(|r| r.method == "get_available_actions")
}

/// The `combat` fixture with enemy 0 at full HP and no powers.
fn combat() -> WireState {
    WireState::combat().edit(|w| {
        w.enemy(0)["hp"] = json!(44);
        w.enemy(0)["powers"] = json!([]);
    })
}

const STRIKE: CardId = CardId::StrikeIronclad;

/// A session whose game shows `before` until the command is sent, then `after`.
fn scripted(scene: &Scene, before: &WireState, after: &WireState) -> Scripted {
    let mut t = ReplayTransport::new();
    before.push_n(&mut t, 2);
    after.push(&mut t);
    scene.push_probes(&mut t, 1);
    t.insert("console", OK);
    Session::with_policy(McpTestBackend::new(t), policy())
}

fn run(
    command: &ConsoleCommand,
    before: &WireState,
    after: &WireState,
) -> (Result<GameState, BridgeError>, Scripted) {
    let mut s = scripted(&Scene::combat(), before, after);
    let result = s.console_with(command, &wait());
    (result, s)
}

fn passes(command: ConsoleCommand, before: &WireState, after: &WireState) {
    let (result, s) = run(&command, before, after);
    result.unwrap_or_else(|e| panic!("{command}: {e}"));
    assert_eq!(console_lines(&s), [command.line()]);
}

fn refused(command: ConsoleCommand, before: &WireState) -> String {
    let (result, s) = run(&command, before, before);
    assert!(console_lines(&s).is_empty(), "{command} was sent");
    match result {
        Err(BridgeError::ActionRefused { reason, .. }) => reason,
        other => panic!("{command}: expected ActionRefused, got {other:?}"),
    }
}

fn hand_card(w: &mut WireState, index: usize) -> Value {
    let mut c = w.hand()[2].clone();
    c["name"] = json!("Bash");
    c["upgraded"] = json!(false);
    c["index"] = json!(index);
    c
}

fn pile_card(name: &str) -> Value {
    json!({"index": 0, "name": name, "type": "Attack", "energy_cost": 1, "upgraded": false})
}

// ---- the exact lines ----

#[test]
fn every_typed_command_renders_its_console_line() {
    use ConsoleCommand as C;
    let lines: Vec<(ConsoleCommand, &str)> = vec![
        (C::Fight(NIBBITS_WEAK), "fight NIBBITS_WEAK"),
        (
            C::Damage {
                target: Target::Enemy(0),
                amount: 7,
            },
            "damage 7 1",
        ),
        (
            C::Damage {
                target: Target::Player,
                amount: 7,
            },
            "damage 7 0",
        ),
        (C::DamageAllEnemies { amount: 4 }, "damage 4"),
        (C::HealPlayer { amount: 9 }, "heal 9"),
        (
            C::Block {
                target: Target::Enemy(1),
                amount: 5,
            },
            "block 5 2",
        ),
        (C::Energy { amount: 2 }, "energy 2"),
        (
            C::Power {
                power: PowerName::new("VULNERABLE_POWER").unwrap(),
                amount: 3,
                target: Target::Enemy(1),
            },
            "power VULNERABLE_POWER 3 2",
        ),
        (
            C::AddCard {
                card: CardId::Bash,
                pile: CombatPile::Hand,
            },
            "card BASH hand",
        ),
        (
            C::AddCard {
                card: STRIKE,
                pile: CombatPile::Exhaust,
            },
            "card STRIKE_IRONCLAD exhaust",
        ),
        (
            C::RemoveCardFromHand { card: CardId::Bash },
            "remove_card BASH hand",
        ),
        (
            C::RemoveCardFromDeck { card: STRIKE },
            "remove_card STRIKE_IRONCLAD deck",
        ),
        (
            C::RelicAdd(Relic::new("ANCHOR").unwrap()),
            "relic add ANCHOR",
        ),
        (
            C::RelicRemove(Relic::new("ANCHOR").unwrap()),
            "relic remove ANCHOR",
        ),
        (C::Draw { count: 2 }, "draw 2"),
        (C::Upgrade { hand_index: 3 }, "upgrade 3"),
        (C::Win, "win"),
        (C::ToggleInstant, "instant"),
        (
            C::Probe("remove_card BASH discard".into()),
            "remove_card BASH discard",
        ),
    ];
    for (command, line) in lines {
        assert_eq!(command.line(), line);
        assert_eq!(
            sts_bridge::Command::Console(command.clone()).to_string(),
            format!("console({line})")
        );
        let first = line.split_whitespace().next().unwrap();
        assert_eq!(command.spec().map(|s| s.name), Some(first), "{line}");
    }
}

#[test]
fn only_the_five_room_changers_enter_rooms() {
    let names: Vec<&str> = catalog::ALL
        .iter()
        .filter(|c| c.enters_room)
        .map(|c| c.name)
        .collect();
    assert_eq!(names, ["act", "ancient", "event", "fight", "room"]);
    assert!(ConsoleCommand::Fight(NIBBITS_WEAK).enters_room());
    assert!(ConsoleCommand::Probe("event NEOW".into()).enters_room());
    assert!(!ConsoleCommand::Probe("travel".into()).enters_room());
    assert!(!ConsoleCommand::Win.enters_room());
}

#[test]
fn the_catalog_is_the_games_39_commands_plus_help() {
    assert_eq!(catalog::ALL.len(), 40);
    assert_eq!(catalog::GAME_VERSION, "0.107.1");
    assert_eq!(catalog::CARD.args, "<card-id:string> [pileName:string]");
    assert!(catalog::find("remove_card").is_some());
    assert!(catalog::find("jaw_worm").is_none());
}

#[test]
fn unknown_ids_are_rejected_against_the_catalog() {
    assert_eq!(
        Encounter::new("JAW_WORM"),
        Err(UnknownId {
            domain: "encounter",
            id: "JAW_WORM".into()
        })
    );
    assert_eq!(Encounter::new("NIBBITS_WEAK"), Ok(NIBBITS_WEAK));
    assert!(PowerName::new("Vulnerable").is_err());
    assert!(Relic::new("NOT_A_RELIC").is_err());
    assert!(PowerName::new("VULNERABLE_POWER").is_ok());
}

#[test]
fn slugify_maps_bridge_class_names_to_console_ids() {
    assert_eq!(slugify("VulnerablePower"), "VULNERABLE_POWER");
    assert_eq!(slugify("BurningBlood"), "BURNING_BLOOD");
    assert_eq!(slugify("ExpectAFight"), "EXPECT_A_FIGHT");
    assert_eq!(slugify("Bash"), "BASH");
}

// ---- postconditions: an effect that shows passes, none fails ----

#[test]
fn a_card_added_to_the_hand_shows_in_the_hand() {
    let before = combat();
    let after = before.clone().edit(|w| {
        let card = hand_card(w, 5);
        w.hand().push(card);
    });
    passes(
        ConsoleCommand::AddCard {
            card: CardId::Bash,
            pile: CombatPile::Hand,
        },
        &before,
        &after,
    );
}

#[test]
fn a_card_added_to_a_pile_shows_in_that_pile_only() {
    let before = combat();
    let draw = before
        .clone()
        .edit(|w| w.pile("draw_pile").push(pile_card("Bash")));
    let command = ConsoleCommand::AddCard {
        card: CardId::Bash,
        pile: CombatPile::Draw,
    };
    passes(command.clone(), &before, &draw);
    // Landing in the discard pile instead is not what was asked.
    let discard = before
        .clone()
        .edit(|w| w.pile("discard_pile").push(pile_card("Bash")));
    let (result, _) = run(&command, &before, &discard);
    match result {
        Err(BridgeError::PostconditionFailed {
            expected, observed, ..
        }) => {
            assert!(expected.contains("piles.draw"), "{expected}");
            assert!(observed.contains("piles.draw"), "{observed}");
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
}

#[test]
fn a_command_that_reports_success_but_changes_nothing_is_a_postcondition_failure() {
    let before = combat();
    let command = ConsoleCommand::AddCard {
        card: CardId::Bash,
        pile: CombatPile::Hand,
    };
    let (result, s) = run(&command, &before, &before);
    match result {
        Err(BridgeError::PostconditionFailed {
            action,
            expected,
            observed,
        }) => {
            assert_eq!(action, "console(card BASH hand)");
            assert!(expected.contains("piles.hand"), "{expected}");
            assert!(observed.contains("piles.hand"), "{observed}");
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
    assert_eq!(console_lines(&s), ["card BASH hand"]);
}

#[test]
fn every_typed_command_with_no_effect_is_a_postcondition_failure() {
    use ConsoleCommand as C;
    let before = combat();
    let commands = vec![
        C::Fight(NIBBITS_WEAK),
        C::Damage {
            target: Target::Enemy(1),
            amount: 8,
        },
        C::DamageAllEnemies { amount: 8 },
        C::HealPlayer { amount: 5 },
        C::Block {
            target: Target::Player,
            amount: 5,
        },
        C::Energy { amount: 1 },
        C::Power {
            power: PowerName::new("VULNERABLE_POWER").unwrap(),
            amount: 1,
            target: Target::Enemy(1),
        },
        C::AddCard {
            card: STRIKE,
            pile: CombatPile::Discard,
        },
        C::RemoveCardFromHand { card: STRIKE },
        C::RelicAdd(Relic::new("ANCHOR").unwrap()),
        C::Draw { count: 1 },
        C::Upgrade { hand_index: 0 },
        C::Win,
        C::Probe("remove_card BASH discard".into()),
    ];
    for command in commands {
        let (result, s) = run(&command, &before, &before);
        assert!(
            matches!(result, Err(BridgeError::PostconditionFailed { .. })),
            "{command}: {result:?}"
        );
        assert_eq!(console_lines(&s), [command.line()]);
    }
}

#[test]
fn damage_goes_through_block_first() {
    let before = combat();
    // Enemy 1 has 44 hp and 5 block: 8 damage costs 3 hp.
    let after = before.clone().edit(|w| {
        w.enemy(1)["hp"] = json!(41);
        w.enemy(1)["block"] = json!(0);
    });
    passes(
        ConsoleCommand::Damage {
            target: Target::Enemy(1),
            amount: 8,
        },
        &before,
        &after,
    );
    // Taking the whole hit from hp ignores the block.
    let wrong = before.clone().edit(|w| w.enemy(1)["hp"] = json!(36));
    let (result, _) = run(
        &ConsoleCommand::Damage {
            target: Target::Enemy(1),
            amount: 8,
        },
        &before,
        &wrong,
    );
    assert!(matches!(
        result,
        Err(BridgeError::PostconditionFailed { .. })
    ));
}

#[test]
fn damage_to_an_enemy_leaves_the_other_untouched() {
    let before = combat();
    let after = before.clone().edit(|w| {
        w.enemy(0)["hp"] = json!(34);
        w.enemy(1)["hp"] = json!(40);
    });
    let (result, _) = run(
        &ConsoleCommand::Damage {
            target: Target::Enemy(0),
            amount: 10,
        },
        &before,
        &after,
    );
    match result {
        Err(BridgeError::PostconditionFailed { expected, .. }) => {
            assert!(expected.contains("enemies[1].hp"), "{expected}")
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
}

#[test]
fn damage_all_enemies_hits_each_living_enemy() {
    let before = combat();
    let after = before.clone().edit(|w| {
        w.enemy(0)["hp"] = json!(40);
        w.enemy(1)["block"] = json!(1);
    });
    passes(
        ConsoleCommand::DamageAllEnemies { amount: 4 },
        &before,
        &after,
    );
}

#[test]
fn a_lethal_hit_that_ends_the_combat_counts() {
    let before = combat().edit(|w| {
        w.enemy(1)["hp"] = json!(1);
        w.enemy(0)["is_alive"] = json!(false);
        w.enemy(0)["hp"] = json!(0);
    });
    let after = WireState::out_of_combat("REWARD");
    passes(
        ConsoleCommand::Damage {
            target: Target::Enemy(1),
            amount: 99,
        },
        &before,
        &after,
    );
}

#[test]
fn healing_stops_at_max_hp() {
    let before = combat();
    let after = before.clone().edit(|w| w.player()["hp"] = json!(80));
    passes(ConsoleCommand::HealPlayer { amount: 50 }, &before, &after);
}

#[test]
fn block_and_energy_go_up_by_exactly_the_amount() {
    let before = combat();
    let block = before.clone().edit(|w| w.player()["block"] = json!(8));
    passes(
        ConsoleCommand::Block {
            target: Target::Player,
            amount: 5,
        },
        &before,
        &block,
    );
    let enemy = before.clone().edit(|w| w.enemy(1)["block"] = json!(12));
    passes(
        ConsoleCommand::Block {
            target: Target::Enemy(1),
            amount: 7,
        },
        &before,
        &enemy,
    );
    let energy = before.clone().edit(|w| w.player()["energy"] = json!(5));
    passes(ConsoleCommand::Energy { amount: 2 }, &before, &energy);
    let short = before.clone().edit(|w| w.player()["energy"] = json!(4));
    let (result, _) = run(&ConsoleCommand::Energy { amount: 2 }, &before, &short);
    assert!(matches!(
        result,
        Err(BridgeError::PostconditionFailed { .. })
    ));
}

#[test]
fn a_power_adds_to_the_target_and_stacks_on_one_it_has() {
    let vulnerable = PowerName::new("VULNERABLE_POWER").unwrap();
    let before = combat();
    let added = before.clone().edit(|w| {
        w.enemy(1)["powers"] = json!([{"name": "VulnerablePower", "amount": 2, "type": "Debuff"}])
    });
    passes(
        ConsoleCommand::Power {
            power: vulnerable,
            amount: 2,
            target: Target::Enemy(1),
        },
        &before,
        &added,
    );
    // Enemy 0 in the fixture already has Vulnerable 2.
    let with_power = WireState::combat();
    let stacked = with_power.clone().edit(|w| {
        w.enemy(0)["powers"] = json!([{"name": "VulnerablePower", "amount": 5, "type": "Debuff"}])
    });
    passes(
        ConsoleCommand::Power {
            power: vulnerable,
            amount: 3,
            target: Target::Enemy(0),
        },
        &with_power,
        &stacked,
    );
    let on_player = before.clone().edit(|w| {
        w.player()["powers"] = json!([
            {"name": "StrengthPower", "amount": 1, "type": "Buff"},
            {"name": "VulnerablePower", "amount": 1, "type": "Debuff"}
        ])
    });
    passes(
        ConsoleCommand::Power {
            power: vulnerable,
            amount: 1,
            target: Target::Player,
        },
        &before,
        &on_player,
    );
}

#[test]
fn a_removed_card_leaves_only_the_hand() {
    let before = combat();
    let after = before.clone().edit(|w| {
        w.hand().remove(0);
    });
    passes(
        ConsoleCommand::RemoveCardFromHand { card: STRIKE },
        &before,
        &after,
    );
    let also_exhausted = after
        .clone()
        .edit(|w| w.pile("exhaust_pile").push(pile_card("StrikeIronclad")));
    let (result, _) = run(
        &ConsoleCommand::RemoveCardFromHand { card: STRIKE },
        &before,
        &also_exhausted,
    );
    match result {
        Err(BridgeError::PostconditionFailed { expected, .. }) => {
            assert!(expected.contains("piles.exhaust"), "{expected}")
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
}

#[test]
fn relics_are_counted_by_their_console_id() {
    let anchor = Relic::new("ANCHOR").unwrap();
    let before = combat();
    let added = before.clone().edit(|w| {
        w.player_state["players"][0]["relics"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name": "Anchor", "rarity": "Common"}))
    });
    passes(ConsoleCommand::RelicAdd(anchor), &before, &added);
    passes(ConsoleCommand::RelicRemove(anchor), &added, &before);
    let state = scripted(&Scene::combat(), &added, &added)
        .backend_mut()
        .state()
        .unwrap();
    assert_eq!(
        state.run.unwrap().relics,
        ["BurningBlood".to_string(), "Anchor".to_string()]
    );
}

fn deck(w: &mut WireState) -> &mut Vec<Value> {
    w.player_state["players"][0]["deck"].as_array_mut().unwrap()
}

#[test]
fn removing_a_deck_card_drops_one_copy_from_the_master_deck_outside_combat() {
    let before = WireState::out_of_combat("EVENT");
    let after = before.clone().edit(|w| {
        deck(w).remove(0);
    });
    let command = ConsoleCommand::RemoveCardFromDeck { card: STRIKE };
    let mut s = scripted(&Scene::event(), &before, &after);
    s.console_with(&command, &wait()).unwrap();
    assert_eq!(console_lines(&s), [command.line()]);

    let mut s = scripted(&Scene::event(), &before, &before);
    assert!(matches!(
        s.console_with(&command, &wait()),
        Err(BridgeError::PostconditionFailed { expected, .. }) if expected.contains("run.deck")
    ));
}

#[test]
fn removing_a_card_that_is_not_in_the_deck_is_refused() {
    let before = combat();
    let reason = refused(
        ConsoleCommand::RemoveCardFromDeck { card: CardId::Bash },
        &before,
    );
    assert!(reason.contains("not in the deck"), "{reason}");
}

#[test]
fn draw_moves_cards_from_the_draw_pile_into_the_hand() {
    let before = combat();
    let after = before.clone().edit(|w| {
        let cards = w.pile("draw_pile").drain(..2).collect::<Vec<_>>();
        for (i, c) in cards.into_iter().enumerate() {
            let mut hand = w.hand()[0].clone();
            hand["name"] = c["name"].clone();
            hand["index"] = json!(5 + i);
            w.hand().push(hand);
        }
    });
    passes(ConsoleCommand::Draw { count: 2 }, &before, &after);
}

#[test]
fn upgrade_turns_the_hand_card_into_its_upgraded_copy() {
    let before = combat();
    let after = before
        .clone()
        .edit(|w| w.hand()[0]["upgraded"] = json!(true));
    passes(ConsoleCommand::Upgrade { hand_index: 0 }, &before, &after);
}

#[test]
fn a_fight_gives_a_new_combat_on_round_one() {
    let before = WireState::out_of_combat("EVENT");
    let after = combat().edit(|w| w.combat["round"] = json!(1));
    let mut s = scripted(&Scene::event(), &before, &after);
    s.console_with(&ConsoleCommand::Fight(NIBBITS_WEAK), &wait())
        .unwrap();
    assert_eq!(console_lines(&s), ["fight NIBBITS_WEAK"]);
    // Round 2 is not a fresh combat.
    let mut s = scripted(&Scene::event(), &before, &combat());
    assert!(matches!(
        s.console_with(&ConsoleCommand::Fight(NIBBITS_WEAK), &wait()),
        Err(BridgeError::PostconditionFailed { .. })
    ));
}

#[test]
fn win_ends_the_combat() {
    let before = combat();
    passes(
        ConsoleCommand::Win,
        &before,
        &WireState::out_of_combat("REWARD"),
    );
}

// ---- preconditions: refused, nothing sent ----

#[test]
fn a_card_that_is_not_in_the_hand_is_refused_without_sending() {
    let reason = refused(
        ConsoleCommand::RemoveCardFromHand {
            card: CardId::Anger,
        },
        &combat(),
    );
    assert!(reason.contains("ANGER"), "{reason}");
}

#[test]
fn commands_the_game_would_reject_are_refused_without_sending() {
    use ConsoleCommand as C;
    let before = combat();
    let dead = before.clone().edit(|w| {
        w.enemy(1)["is_alive"] = json!(false);
        w.enemy(1)["hp"] = json!(0);
    });
    let full = before.clone().edit(|w| {
        for i in 5..10 {
            let mut c = w.hand()[0].clone();
            c["index"] = json!(i);
            w.hand().push(c);
        }
    });
    let enemy_turn = before
        .clone()
        .screen("COMBAT_ENEMY_TURN")
        .edit(|w| w.combat["is_player_turn"] = json!(false));
    let cases: Vec<(ConsoleCommand, &WireState, &str)> = vec![
        (
            C::Damage {
                target: Target::Enemy(1),
                amount: 5,
            },
            &dead,
            "dead",
        ),
        (
            C::Damage {
                target: Target::Enemy(7),
                amount: 5,
            },
            &before,
            "does not exist",
        ),
        (
            C::Power {
                power: PowerName::new("VULNERABLE_POWER").unwrap(),
                amount: 0,
                target: Target::Player,
            },
            &before,
            "positive",
        ),
        (
            C::AddCard {
                card: CardId::Bash,
                pile: CombatPile::Hand,
            },
            &full,
            "full",
        ),
        (C::Upgrade { hand_index: 9 }, &before, "out of range"),
        (C::Upgrade { hand_index: 2 }, &before, "already upgraded"),
        (C::ToggleInstant, &before, "set_instant"),
        (
            C::Probe("jaw_worm 1".into()),
            &before,
            "not a console command",
        ),
    ];
    for (command, state, why) in cases {
        let reason = refused(command.clone(), state);
        assert!(reason.contains(why), "{command}: {reason}");
    }
    // The enemy's turn never settles, so nothing is sent either.
    let mut s = scripted(&Scene::combat(), &enemy_turn, &enemy_turn);
    assert!(matches!(
        s.console_with(&ConsoleCommand::Win, &wait()),
        Err(BridgeError::NotSettled { .. })
    ));
    assert!(console_lines(&s).is_empty());
}

#[test]
fn a_console_command_with_no_run_is_refused() {
    let mut t = ReplayTransport::new();
    Scene::main_menu().show(&mut t);
    let mut s = Session::with_policy(McpTestBackend::new(t), policy());
    match s.console_with(&ConsoleCommand::Energy { amount: 1 }, &wait()) {
        Err(BridgeError::ActionRefused { reason, .. }) => {
            assert!(reason.contains("no run"), "{reason}")
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
    assert!(console_lines(&s).is_empty());
}

// ---- loading: nothing is ever sent ----

fn loading_scenes() -> Vec<(&'static str, Scene)> {
    vec![
        ("a run loading", Scene::run_loading()),
        ("a loading screen", Scene::screen("ROOM_LOADING", true)),
        ("no room yet", Scene::no_room_yet()),
    ]
}

#[test]
fn every_console_command_is_refused_at_once_while_a_run_loads() {
    use ConsoleCommand as C;
    let commands = vec![
        C::Fight(NIBBITS_WEAK),
        C::Energy { amount: 1 },
        C::RelicAdd(Relic::new("ANCHOR").unwrap()),
        C::Win,
        C::Probe("event NEOW".into()),
    ];
    for (name, scene) in loading_scenes() {
        for command in &commands {
            let mut t = ReplayTransport::new();
            scene.show(&mut t);
            t.insert("console", OK);
            let mut s = Session::with_policy(McpTestBackend::new(t), policy());
            let started = std::time::Instant::now();
            match s.console_with(command, &wait()) {
                Err(BridgeError::ActionRefused { reason, .. }) => {
                    assert!(reason.contains("loading"), "{name}: {reason}")
                }
                other => panic!("{name}, {command}: expected ActionRefused, got {other:?}"),
            }
            assert!(
                started.elapsed() < policy().timeout / 2,
                "{name}: not at once"
            );
            assert!(console_lines(&s).is_empty(), "{name}: {command} was sent");
            assert!(!has_state_reads(&s), "{name}: waited on a state first");
        }
    }
}

#[test]
fn set_instant_is_refused_while_a_run_loads() {
    for (name, scene) in loading_scenes() {
        let mut t = ReplayTransport::new();
        scene.show(&mut t);
        t.insert("console", OK);
        let mut s = Session::with_policy(McpTestBackend::new(t), policy());
        assert!(
            matches!(
                s.set_instant_with(true, &wait()),
                Err(BridgeError::ActionRefused { .. })
            ),
            "{name}"
        );
        assert!(console_lines(&s).is_empty(), "{name}");
    }
}

// ---- room changes wait for a held in-run screen ----

fn in_run(scene: &Scene) -> WireState {
    WireState::out_of_combat(scene.screen.unwrap())
}

#[test]
fn a_room_change_waits_for_the_screen_to_hold_and_sends_nothing_before() {
    // The screen keeps flipping between the map and an event.
    let mut t = ReplayTransport::new();
    for _ in 0..400 {
        Scene::map().push_probes(&mut t, 1);
        Scene::event().push_probes(&mut t, 1);
    }
    in_run(&Scene::map()).push_n(&mut t, 4);
    t.insert("console", OK);
    for command in [
        ConsoleCommand::Fight(NIBBITS_WEAK),
        ConsoleCommand::Probe("event NEOW".into()),
    ] {
        let mut s = Session::with_policy(McpTestBackend::new(t.clone()), policy());
        match s.console_with(&command, &wait()) {
            Err(BridgeError::NotSettled { unmet, .. }) => {
                assert!(unmet.contains("a stable in-run screen"), "{unmet}")
            }
            other => panic!("{command}: expected NotSettled, got {other:?}"),
        }
        assert!(console_lines(&s).is_empty(), "{command} was sent");
    }
}

#[test]
fn a_room_change_is_sent_once_the_screen_holds() {
    for scene in [Scene::event(), Scene::map()] {
        let before = in_run(&scene);
        let after = before.clone().screen("REWARD");
        let mut s = scripted(&scene, &before, &after);
        s.console_with(&ConsoleCommand::Probe("event NEOW".into()), &wait())
            .unwrap();
        assert_eq!(console_lines(&s), ["event NEOW"]);
    }
}

// ---- the game's own reply ----

#[test]
fn a_rejected_command_is_refused_with_the_games_message() {
    let before = combat();
    let mut s = scripted(&Scene::combat(), &before, &before);
    s.backend_mut().transport_mut().insert(
        "console",
        json!({"result": {"success": false, "msg": "Card 'BASH' not found"}, "id": 1}).to_string(),
    );
    match s.console_with(
        &ConsoleCommand::AddCard {
            card: CardId::Bash,
            pile: CombatPile::Hand,
        },
        &wait(),
    ) {
        Err(BridgeError::ActionRefused { action, reason }) => {
            assert_eq!(action, "console(card BASH hand)");
            assert!(reason.contains("Card 'BASH' not found"), "{reason}");
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
}

// ---- the per-call policy ----

/// Answers like the inner replay, but the first state read takes `delay`.
struct SlowFirstRead {
    inner: ReplayTransport,
    delay: Duration,
    slow: bool,
}

impl Transport for SlowFirstRead {
    fn send(&mut self, method: &str, params: Option<&Value>) -> Result<Vec<u8>, BridgeError> {
        if self.slow && method == "get_available_actions" {
            self.slow = false;
            std::thread::sleep(self.delay);
        }
        self.inner.send(method, params)
    }
}

fn slow_session(session_policy: SettlePolicy) -> Session<McpTestBackend<SlowFirstRead>> {
    let before = combat();
    let after = before.clone().edit(|w| w.player()["energy"] = json!(5));
    let mut inner = ReplayTransport::new();
    before.push_n(&mut inner, 2);
    after.push(&mut inner);
    Scene::combat().push_probes(&mut inner, 1);
    inner.insert("console", OK);
    let slow = SlowFirstRead {
        inner,
        delay: Duration::from_millis(120),
        slow: true,
    };
    Session::with_policy(McpTestBackend::new(slow), session_policy)
}

fn short() -> SettlePolicy {
    policy().with_timeout(Duration::from_millis(60))
}

#[test]
fn a_slow_first_read_times_out_under_the_session_policy() {
    let mut s = slow_session(short());
    let energy = ConsoleCommand::Energy { amount: 2 };
    assert!(matches!(
        s.act(&sts_bridge::Command::Console(energy)),
        Err(BridgeError::NotSettled { .. })
    ));
}

#[test]
fn console_runs_under_a_longer_timeout_and_leaves_the_session_policy_alone() {
    let mut s = slow_session(short());
    s.console(&ConsoleCommand::Energy { amount: 2 }).unwrap();
    assert_eq!(s.policy(), &short());

    let mut s = slow_session(short());
    let wait = ConsoleWait::for_session(s.policy())
        .with_settle(short().with_timeout(Duration::from_secs(2)));
    s.console_with(&ConsoleCommand::Energy { amount: 2 }, &wait)
        .unwrap();
    assert_eq!(s.policy(), &short());
}

#[test]
fn the_session_policy_is_restored_when_the_command_fails() {
    let mut s = slow_session(short());
    let wait = ConsoleWait::for_session(s.policy())
        .with_settle(short().with_timeout(Duration::from_millis(5)));
    assert!(s
        .console_with(&ConsoleCommand::Energy { amount: 2 }, &wait)
        .is_err());
    assert_eq!(s.policy(), &short());
    assert!(s.console(&ConsoleCommand::ToggleInstant).is_err());
    assert_eq!(s.policy(), &short());
}

#[test]
fn the_default_console_wait_raises_only_the_timeout() {
    let default = ConsoleWait::for_session(&SettlePolicy::default());
    assert_eq!(default.settle.timeout, CONSOLE_TIMEOUT);
    assert_eq!(
        default.settle.stable_reads,
        SettlePolicy::default().stable_reads
    );
    assert_eq!(default.room_ready.run_load, CONSOLE_TIMEOUT);
    let longer = SettlePolicy::default().with_timeout(Duration::from_secs(120));
    assert_eq!(
        ConsoleWait::for_session(&longer).settle.timeout,
        Duration::from_secs(120)
    );
}

// ---- instant mode as a state ----

fn ping(fast_mode: Option<bool>) -> String {
    let mut r = json!({
        "status": "ok", "mod": "MCPTest", "version": "2.0.0",
        "screen": "MAP", "run_in_progress": true, "in_combat": false
    });
    if let Some(f) = fast_mode {
        r["fast_mode"] = json!(f);
    }
    json!({"result": r, "id": 1}).to_string()
}

fn instant_session(pings: &[Option<bool>]) -> Scripted {
    let mut t = ReplayTransport::new();
    for p in pings {
        t.push("ping", ping(*p));
    }
    t.insert(
        "get_screen",
        json!({"result": {"screen": "MAP", "screen_source": "active_screen_context"}}).to_string(),
    );
    t.insert("console", OK);
    Session::with_policy(McpTestBackend::new(t), policy())
}

fn toggles(s: &Scripted) -> usize {
    console_lines(s).iter().filter(|l| *l == "instant").count()
}

#[test]
fn setting_instant_twice_sends_at_most_one_toggle() {
    // Probe, current mode, then the reads after the toggle; the last repeats.
    let mut s = instant_session(&[Some(false), Some(false), Some(false), Some(true)]);
    assert!(s.set_instant_with(true, &wait()).unwrap());
    assert_eq!(toggles(&s), 1);
    assert!(s.set_instant_with(true, &wait()).unwrap());
    assert_eq!(toggles(&s), 1);
}

#[test]
fn setting_instant_to_its_current_mode_sends_nothing() {
    let mut s = instant_session(&[Some(false)]);
    assert!(!s.set_instant_with(false, &wait()).unwrap());
    assert_eq!(toggles(&s), 0);
}

#[test]
fn a_bridge_that_does_not_report_the_mode_is_refused_without_sending() {
    let mut s = instant_session(&[None]);
    match s.set_instant_with(true, &wait()) {
        Err(BridgeError::ActionRefused { reason, .. }) => {
            assert!(reason.contains("fast_mode"), "{reason}")
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
    assert_eq!(toggles(&s), 0);
}

#[test]
fn a_toggle_that_does_not_take_is_a_failure_and_is_not_repeated() {
    let mut s = instant_session(&[Some(false)]);
    match s.set_instant_with(true, &wait()) {
        Err(BridgeError::PostconditionFailed {
            expected, observed, ..
        }) => {
            assert!(
                expected.contains("on") && observed.contains("off"),
                "{expected} / {observed}"
            )
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
    assert_eq!(toggles(&s), 1);
}

#[test]
fn a_toggle_whose_mode_never_reads_back_is_not_settled() {
    let mut s = instant_session(&[Some(false), Some(false), None]);
    assert!(matches!(
        s.set_instant_with(true, &wait()),
        Err(BridgeError::NotSettled { .. })
    ));
    assert_eq!(toggles(&s), 1);
}

#[test]
fn set_instant_leaves_the_session_policy_alone() {
    let mut s = instant_session(&[Some(true)]);
    assert!(s.set_instant(true).unwrap());
    assert_eq!(s.policy(), &policy());
}

#[test]
fn commands_that_would_change_nothing_are_refused_without_sending() {
    use ConsoleCommand as C;
    let before = combat();
    let full_hp = before.clone().edit(|w| w.player()["hp"] = json!(80));
    let cases: Vec<(ConsoleCommand, &WireState, &str)> = vec![
        (
            C::RelicRemove(Relic::new("ANCHOR").unwrap()),
            &before,
            "no ANCHOR",
        ),
        (C::Energy { amount: 0 }, &before, "changes nothing"),
        (C::Draw { count: 0 }, &before, "changes nothing"),
        (C::HealPlayer { amount: 5 }, &full_hp, "max hp"),
    ];
    for (command, state, why) in cases {
        let reason = refused(command.clone(), state);
        assert!(reason.contains(why), "{command}: {reason}");
    }
}

#[test]
fn a_combat_that_ended_without_a_lethal_hit_is_not_the_damages_effect() {
    let (result, _) = run(
        &ConsoleCommand::Damage {
            target: Target::Enemy(1),
            amount: 1,
        },
        &combat(),
        &WireState::out_of_combat("REWARD"),
    );
    assert!(matches!(
        result,
        Err(BridgeError::PostconditionFailed { .. })
    ));
}

#[test]
fn a_room_change_without_a_run_is_refused_at_once() {
    let mut t = ReplayTransport::new();
    Scene::main_menu().show(&mut t);
    let mut s = Session::with_policy(McpTestBackend::new(t), policy());
    let started = std::time::Instant::now();
    match s.console_with(&ConsoleCommand::Fight(NIBBITS_WEAK), &wait()) {
        Err(BridgeError::ActionRefused { reason, .. }) => assert!(reason.contains("no run")),
        other => panic!("expected ActionRefused, got {other:?}"),
    }
    assert!(started.elapsed() < policy().timeout / 2);
    assert!(console_lines(&s).is_empty());
}
