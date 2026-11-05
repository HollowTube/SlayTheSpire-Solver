//! The sim's public Rust API, exercised from outside the crate with no Python.
use sts_sim::engine::Status;
use sts_sim::ids::{CardId, MonsterId};
use sts_sim::state::CardInstance;
use sts_sim::{
    apply, is_terminal, legal_actions, CombatParts, CombatState, Counters, FighterParts,
    MonsterCounters, MonsterParts, SimAction, SimError,
};

fn strike() -> CardInstance {
    CardInstance::new(CardId::StrikeIronclad)
}

fn defend() -> CardInstance {
    CardInstance::new(CardId::DefendIronclad)
}

fn parts() -> CombatParts {
    CombatParts {
        player: FighterParts {
            hp: 50,
            max_hp: 80,
            block: 3,
            statuses: vec![("Strength".to_string(), 2)],
        },
        energy: 3,
        max_energy: 3,
        monsters: vec![MonsterParts {
            id: Some(MonsterId::Nibbit),
            attack: 0,
            hp: 40,
            max_hp: 44,
            block: 0,
            statuses: vec![("Vulnerable".to_string(), 1)],
            intent: Some("BUTT_MOVE".to_string()),
        }],
        hand: vec![strike(), defend()],
        draw_pile: vec![defend(), strike(), strike()],
        discard_pile: vec![strike()],
        exhaust_pile: vec![],
        turn: 2,
        seed: 7,
        counters: Counters {
            cards_played_this_turn: 1,
            attacks_played_this_turn: 1,
            times_damaged_this_combat: 2,
            hp_lost_this_turn: true,
            exhausted_this_turn: false,
            blocks_gained_this_turn: 1,
            last_attacker: Some(0),
            combat_counters: [("Rampage".to_string(), 5)].into_iter().collect(),
            monsters: vec![MonsterCounters {
                last_move: Some("Hiss".to_string()),
                move_streak: 1,
                moves_used: vec![],
            }],
        },
    }
}

#[test]
fn builds_a_combat_applies_actions_and_reads_the_result() {
    let state = CombatState::from_parts(parts()).unwrap();
    let after = apply(
        &state,
        &SimAction::PlayCard { card: CardId::DefendIronclad, upgrades: 0 },
    )
    .unwrap();
    assert_eq!(after.energy(), 2);
    assert_eq!(after.hand().len(), 1);
    assert_eq!(after.discard_pile().len(), 2);
    assert!(after.player().block > 3);
    // the original is untouched
    assert_eq!(state.energy(), 3);
    assert!(!is_terminal(&after));
}

#[test]
fn illegal_action_is_a_typed_error() {
    let state = CombatState::from_parts(parts()).unwrap();
    let err = apply(
        &state,
        &SimAction::PlayCard { card: CardId::Bash, upgrades: 0 },
    )
    .unwrap_err();
    assert_eq!(err, SimError::NotInHand { card: "Bash".to_string() });
    assert_eq!(err.to_string(), "Bash is not in hand");
}

#[test]
fn legal_actions_list_hand_plays_then_end_turn() {
    let state = CombatState::from_parts(parts()).unwrap();
    let actions = legal_actions(&state);
    assert_eq!(
        actions,
        vec![
            SimAction::PlayCard { card: CardId::StrikeIronclad, upgrades: 0 },
            SimAction::PlayCard { card: CardId::DefendIronclad, upgrades: 0 },
            SimAction::EndTurn,
        ]
    );
    assert_eq!(actions[0].to_string(), "PlayCard:Strike");
    assert_eq!("PlayCard:Strike+".parse::<SimAction>().unwrap().to_string(), "PlayCard:Strike+");
}

#[test]
fn targeted_card_goes_through_a_pending_target_choice() {
    let state = CombatState::from_parts(parts()).unwrap();
    let pending = state.apply(&SimAction::PlayCard { card: CardId::StrikeIronclad, upgrades: 0 }).unwrap();
    assert!(pending.pending().is_some());
    assert_eq!(legal_actions(&pending), vec![SimAction::SelectTarget(0)]);
    let done = pending.apply(&SimAction::SelectTarget(0)).unwrap();
    assert!(done.monsters()[0].fighter.hp < 40);
    assert!(done.pending().is_none());
}

#[test]
fn step_advances_in_place() {
    let mut state = CombatState::from_parts(parts()).unwrap();
    state.step(&SimAction::EndTurn).unwrap();
    assert_eq!(state.turn(), 3);
}

#[test]
fn parts_round_trip_with_non_default_counters() {
    let state = CombatState::from_parts(parts()).unwrap();
    let round = state.parts();
    assert_eq!(round.draw_pile, parts().draw_pile);
    assert_eq!(round.counters, parts().counters);
    assert_eq!(round.seed, 7);
    assert_eq!(CombatState::from_parts(round.clone()).unwrap().parts(), round);
}

#[test]
fn intent_ids_are_translated_to_move_names() {
    let state = CombatState::from_parts(parts()).unwrap();
    assert_eq!(state.parts().monsters[0].intent.as_deref(), Some("Butt"));
}

#[test]
fn draw_pile_is_listed_top_first() {
    let state = CombatState::from_parts(parts()).unwrap();
    // The sim draws from the end of its pile; `parts` lists the top first.
    assert_eq!(state.draw_pile().last(), Some(&defend()));
    assert_eq!(state.parts().draw_pile[0], defend());
}

#[test]
fn from_parts_rejects_what_the_lenient_constructor_would_drop() {
    let mut p = parts();
    p.player.statuses.push(("NotAStatus".to_string(), 1));
    assert_eq!(
        CombatState::from_parts(p).unwrap_err(),
        SimError::UnknownStatus { name: "NotAStatus".to_string() }
    );

    let mut p = parts();
    p.counters.combat_counters.insert("Nope".to_string(), 1);
    assert!(matches!(CombatState::from_parts(p), Err(SimError::BadParts { .. })));

    let mut p = parts();
    p.counters.monsters.clear();
    assert!(matches!(CombatState::from_parts(p), Err(SimError::BadParts { .. })));

    let mut p = parts();
    p.counters.last_attacker = Some(4);
    assert!(matches!(CombatState::from_parts(p), Err(SimError::BadParts { .. })));

    let mut p = parts();
    p.monsters[0].intent = Some("NOT_A_MOVE".to_string());
    assert!(matches!(CombatState::from_parts(p), Err(SimError::UnknownMove { .. })));
}

#[test]
fn states_and_errors_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CombatState>();
    assert_send_sync::<CombatParts>();
    assert_send_sync::<SimAction>();
    assert_send_sync::<SimError>();
    let _ = Status::Vulnerable;
}

#[test]
fn display_matches_the_messages_python_used_to_raise() {
    let cases: Vec<(SimError, &str)> = vec![
        (
            SimError::HandChoicePending { action: "EndTurn".into() },
            "EndTurn is not legal until the pending hand choice is made",
        ),
        (SimError::NoUpgradable { card: "Strike".into() }, "no upgradable Strike in hand"),
        (SimError::NotInHand { card: "Strike".into() }, "Strike is not in hand"),
        (
            SimError::CannotAfford { card: "Bash".into(), cost: 2, energy: 1 },
            "cannot afford Bash: costs 2 but only 1 energy available",
        ),
        (SimError::UnknownAction { action: "Dance".into() }, "unknown action: Dance"),
        (SimError::UnknownCard { card: "Zzz".into() }, "unknown card: Zzz"),
        (SimError::UnknownEncounter { name: "Nope".into() }, "unknown encounter: Nope"),
        (SimError::NotOnOffer { card: "Bash".into() }, "Bash is not on offer"),
        (SimError::UnknownRunAction { action: "Bogus".into() }, "unknown run action: Bogus"),
        (
            SimError::InvalidUpgradeIndex { action: "Upgrade:x".into() },
            "invalid upgrade index: Upgrade:x",
        ),
        (SimError::NoCardAtIndex { index: 9 }, "no card at index 9"),
        (SimError::AlreadyUpgraded { index: 2 }, "card at index 2 is already upgraded"),
        (SimError::RunAtTerminal { action: "Heal".into() }, "Heal at a terminal run"),
        (SimError::ResolveAtNonCombat, "ResolveCombat at a terminal or non-combat node"),
    ];
    for (err, msg) in cases {
        assert_eq!(err.to_string(), msg);
    }
}

#[test]
fn a_monster_without_an_intent_round_trips_without_one() {
    let mut p = parts();
    p.monsters[0].intent = None;
    let state = CombatState::from_parts(p.clone()).unwrap();
    assert_eq!(state.parts().monsters[0].intent, None);
    assert_eq!(state.parts(), p);
}

#[test]
fn infested_keeps_its_count_and_non_positive_binary_statuses_are_rejected() {
    let mut p = parts();
    p.monsters[0].statuses = vec![("Infested".to_string(), 3)];
    let state = CombatState::from_parts(p.clone()).unwrap();
    assert_eq!(state.parts().monsters[0].statuses, p.monsters[0].statuses);

    let mut p = parts();
    p.player.statuses = vec![("Vulnerable".to_string(), 0)];
    assert!(matches!(CombatState::from_parts(p), Err(SimError::BadParts { .. })));
}
