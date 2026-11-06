//! Offline checks of the parity harness: no game, no live code.

use std::collections::BTreeSet;

use sts_bridge::console::slugify;
use sts_bridge::{
    CardRef, CombatView, EnemyView, Energy, Entity, Fixture, GameState, HandCard, Hp, Intent,
    MonsterId, Piles, PlayerView, Power, Screen, Setters, StatusId,
};
use sts_ids::{BridgeId, CardId};
use sts_parity::cases::cases;
use sts_parity::{
    classify, diff, guarded, record_step, sim_step, Carry, CaseError, Expected, Needs, ParityCase,
    ParityOutcome, ParityState, ParityStep, SimFailure, StepAction, StepOutcome, Verdict, EXPECTED,
};
use sts_sim::cards::card_data;

fn power(name: &str, amount: i32) -> Power {
    let class = StatusId::all()
        .iter()
        .flat_map(|s| s.bridge_names().iter().copied())
        .find(|n| slugify(n) == name)
        .unwrap_or_else(|| panic!("no status for {name}"));
    Power {
        id: Entity::parse(class.to_string()),
        amount,
        kind: None,
    }
}

fn card_ref(c: &sts_bridge::CardSpec) -> CardRef {
    CardRef {
        id: Entity::Known(c.id),
        upgraded: c.upgraded,
        cost: card_data(c.id, u8::from(c.upgraded)).cost,
        kind: "Attack".into(),
    }
}

/// The game state a verified fixture would show.
fn synth(f: &Fixture) -> GameState {
    let mut powers = Vec::new();
    for (p, n) in &f.player.powers {
        powers.push(power(p.as_str(), *n));
    }
    let energy = match f.player.energy {
        Energy::Start => 3,
        Energy::Exact(e) => e,
    };
    let hp = match f.player.hp {
        Hp::Max => 80,
        Hp::Exact(h) => h,
    };
    let enemies = f
        .enemies
        .iter()
        .enumerate()
        .map(|(i, e)| EnemyView {
            index: i,
            entity_id: format!("Nibbit_{i}"),
            name: Entity::Known(MonsterId::Nibbit),
            hp: e.hp,
            max_hp: 44,
            block: e.block,
            is_alive: true,
            powers: match &e.powers {
                sts_bridge::EnemyPowers::AsSpawned => Vec::new(),
                sts_bridge::EnemyPowers::Exact(ps) => {
                    ps.iter().map(|(p, n)| power(p.as_str(), *n)).collect()
                }
            },
            intent: Some(Intent {
                move_id: "BUTT_MOVE".into(),
                parts: Vec::new(),
            }),
        })
        .collect();
    GameState {
        screen: Screen("COMBAT_PLAYER_TURN".into()),
        run: None,
        combat: Some(CombatView {
            round: 1,
            is_player_turn: true,
            player: PlayerView {
                hp,
                max_hp: 80,
                block: f.player.block,
                energy,
                max_energy: 3,
                powers,
            },
            enemies,
            piles: Piles {
                hand: f
                    .hand
                    .iter()
                    .enumerate()
                    .map(|(i, c)| HandCard {
                        index: i,
                        id: Entity::Known(c.id),
                        upgraded: c.upgraded,
                        cost: card_data(c.id, u8::from(c.upgraded)).cost,
                        kind: "Attack".into(),
                        playable: true,
                        unplayable_reason: None,
                        target_type: "AnyEnemy".into(),
                        valid_targets: Some(vec![0]),
                    })
                    .collect(),
                draw: f.draw.iter().map(card_ref).collect(),
                discard: f.discard.iter().map(card_ref).collect(),
                exhaust: f.exhaust.iter().map(card_ref).collect(),
            },
        }),
    }
}

/// A game state showing `after`, with `template`'s intents.
fn to_game(after: &ParityState, template: &GameState) -> GameState {
    let mut g = template.clone();
    let c = g.combat.as_mut().unwrap();
    let power_list = |m: &std::collections::BTreeMap<String, i32>| -> Vec<Power> {
        m.iter()
            .map(|(k, v)| Power {
                id: Entity::parse(k.clone()),
                amount: *v,
                kind: None,
            })
            .collect()
    };
    let card = |name: &str| -> (Entity<CardId>, bool) {
        let up = name.ends_with('+');
        (Entity::parse(name.trim_end_matches('+').to_string()), up)
    };
    c.round = after.round;
    c.player.hp = after.player.hp;
    c.player.max_hp = after.player.max_hp;
    c.player.block = after.player.block;
    c.player.energy = after.player.energy;
    c.player.powers = power_list(&after.player.powers);
    for (e, s) in c.enemies.iter_mut().zip(&after.enemies) {
        e.hp = s.hp;
        e.max_hp = s.max_hp;
        e.block = s.block;
        e.is_alive = s.alive;
        e.powers = power_list(&s.powers);
    }
    let refs = |names: &[String]| -> Vec<CardRef> {
        names
            .iter()
            .map(|n| {
                let (id, upgraded) = card(n);
                CardRef {
                    id,
                    upgraded,
                    cost: 1,
                    kind: "Attack".into(),
                }
            })
            .collect()
    };
    c.piles.hand = after
        .hand
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let (id, upgraded) = card(n);
            HandCard {
                index: i,
                id,
                upgraded,
                cost: 1,
                kind: "Attack".into(),
                playable: true,
                unplayable_reason: None,
                target_type: "AnyEnemy".into(),
                valid_targets: Some(vec![0]),
            }
        })
        .collect();
    c.piles.draw = refs(&after.draw);
    c.piles.discard = refs(&after.discard);
    c.piles.exhaust = refs(&after.exhaust);
    g
}

/// Runs a case's steps through the sim; the sim's own after-state is the next
/// before-state.
fn run_case(case: &ParityCase) -> Vec<Result<ParityState, SimFailure>> {
    let template = synth(&case.fixture);
    let mut state = template.clone();
    let mut carry = Carry::fresh(case.fixture.enemies.len());
    let mut out = Vec::new();
    for step in &case.steps {
        match sim_step(&state, &carry, step) {
            Ok(s) => {
                state = to_game(&s.after, &template);
                carry = s.carry;
                out.push(Ok(s.after));
            }
            Err(f) => {
                out.push(Err(f));
                break;
            }
        }
    }
    out
}

fn unblocked() -> Vec<ParityCase> {
    cases(&[])
        .into_iter()
        .filter(|c| c.blocked.is_none())
        .collect()
}

#[test]
fn every_case_runs_through_the_sim() {
    let mut problems = Vec::new();
    for case in unblocked() {
        let kill_ok = case.name.contains("feed") && case.fixture.enemies.len() == 2;
        let results = run_case(&case);
        if results.len() != case.steps.len() || results.iter().any(Result::is_err) {
            problems.push(format!("{}: {:?}", case.name, results.last()));
            continue;
        }
        for (i, r) in results.iter().enumerate() {
            let s = r.as_ref().unwrap();
            if s.hand.len() > 10 {
                problems.push(format!("{} step {i}: hand of {}", case.name, s.hand.len()));
            }
            let dead = case.fixture.enemies.len() - s.enemies.len();
            let fatal_feed = kill_ok && case.steps[i].card.starts_with("Feed");
            if dead > 0 && !fatal_feed {
                problems.push(format!("{} step {i}: an enemy died", case.name));
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn the_feed_kill_is_the_only_death_and_is_not_final() {
    let case = unblocked()
        .into_iter()
        .find(|c| c.name.starts_with("feed") && c.fixture.enemies.len() == 2)
        .expect("a fatal Feed case");
    let last = run_case(&case).pop().unwrap().unwrap();
    // The dead enemy is not in the projection, as in the game's list.
    assert_eq!(last.enemies.len(), 1);
    assert!(last.enemies[0].hp > 0);
}

fn ironclad_registry_cards() -> Vec<CardId> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/game/0.107.1/ironclad_cards.json"
    );
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    v["cards"]
        .as_object()
        .unwrap()
        .values()
        .filter_map(|c| CardId::from_bridge(c["class"].as_str().unwrap()))
        .collect()
}

#[test]
fn every_ironclad_registry_card_has_a_case_for_both_variants() {
    let cards = ironclad_registry_cards();
    assert_eq!(cards.len(), 85);
    let covered: BTreeSet<String> = cases(&[])
        .iter()
        .flat_map(|c| c.cards.iter().cloned())
        .collect();
    let missing: Vec<String> = cards
        .iter()
        .flat_map(|c| [c.as_str().to_string(), format!("{}+", c.as_str())])
        .filter(|t| !covered.contains(t))
        .collect();
    assert!(
        missing.is_empty(),
        "no case or blocked entry for {missing:?}"
    );
}

fn first_case(name: &str) -> ParityCase {
    if name == "bash" {
        return cases(&["Bash".to_string()]).remove(0);
    }
    cases(&[])
        .into_iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no case {name}"))
}

#[test]
fn a_one_off_enemy_hp_names_the_field_and_both_values() {
    let case = first_case("bash");
    let a = run_case(&case).pop().unwrap().unwrap();
    let mut b = a.clone();
    b.enemies[0].hp += 1;
    let d = diff(&a, &b, &[]);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].to_string(), "enemies[0].hp: sim 40, game 41");
}

fn step_play(id: CardId) -> ParityStep {
    ParityStep {
        action: StepAction::PlayCard {
            card: id.into(),
            target: Some(0),
        },
        card: id.as_str().into(),
    }
}

fn base_state() -> GameState {
    let f = Fixture::new(sts_bridge::NIBBITS_WEAK)
        .enemy(sts_bridge::EnemySpec::new(40).block(100))
        .hand([CardId::StrikeIronclad, CardId::Bludgeon]);
    synth(&f)
}

#[test]
fn sim_failures_are_failures_never_skips() {
    let carry = Carry::fresh(1);
    // An unmapped game power.
    let mut s = base_state();
    s.combat.as_mut().unwrap().player.powers.push(Power {
        id: Entity::Unknown("ManglePower".into()),
        amount: 1,
        kind: None,
    });
    assert!(matches!(
        sim_step(&s, &carry, &step_play(CardId::StrikeIronclad)),
        Err(SimFailure::Mapping(m)) if m.contains("ManglePower")
    ));
    // An unknown card.
    let mut s = base_state();
    s.combat.as_mut().unwrap().piles.hand[0].id = Entity::Unknown("NoSuchCard".into());
    assert!(matches!(
        sim_step(&s, &carry, &step_play(CardId::Bludgeon)),
        Err(SimFailure::Mapping(m)) if m.contains("NoSuchCard")
    ));
    // A sim error: Bludgeon costs 3 with 0 energy.
    let mut s = base_state();
    s.combat.as_mut().unwrap().player.energy = 0;
    assert!(matches!(
        sim_step(&s, &carry, &step_play(CardId::Bludgeon)),
        Err(SimFailure::Sim(_))
    ));
    // A panic.
    assert!(matches!(
        guarded::<()>(|| panic!("boom")),
        Err(SimFailure::Panic(m)) if m == "boom"
    ));
    // An unresolved pending choice: Armaments with two upgradable cards.
    let f = Fixture::new(sts_bridge::NIBBITS_WEAK)
        .enemy(sts_bridge::EnemySpec::new(40))
        .hand([
            CardId::StrikeIronclad,
            CardId::StrikeIronclad,
            CardId::Armaments,
        ]);
    assert!(matches!(
        sim_step(&synth(&f), &carry, &step_play(CardId::Armaments)),
        Err(SimFailure::PendingChoice(_))
    ));
}

fn outcome(case: &ParityCase, steps: Vec<(&str, Vec<(&str, &str, &str)>)>) -> ParityOutcome {
    let state = synth(&case.fixture);
    ParityOutcome {
        case: case.clone(),
        verified: Some(state.clone()),
        steps: steps
            .into_iter()
            .map(|(card, diffs)| StepOutcome {
                step: ParityStep {
                    action: StepAction::EndTurn,
                    card: card.into(),
                },
                before: state.clone(),
                carry: Carry::default(),
                game_after: state.clone(),
                sim_after: Ok(ParityState::from_game(&state).unwrap()),
                diff: diffs
                    .into_iter()
                    .map(|(f, s, g)| sts_parity::ParityDiff {
                        field: f.into(),
                        sim: s.into(),
                        game: g.into(),
                    })
                    .collect(),
            })
            .collect(),
        error: None,
        game_version: None,
    }
}

#[test]
fn expected_divergences_classify_with_their_issue_and_go_stale() {
    let case = first_case("bash");
    let list = [Expected {
        card: "Bash",
        field: "enemies[0].hp",
        issue: "HOL-1",
    }];
    let listed = outcome(&case, vec![("Bash", vec![("enemies[0].hp", "1", "2")])]);
    assert_eq!(
        classify(&listed, &list),
        Verdict::Expected(vec!["HOL-1".into()])
    );
    let unlisted = outcome(&case, vec![("Bash", vec![("player.hp", "1", "2")])]);
    assert!(matches!(classify(&unlisted, &list), Verdict::Fail(_)));
    let clean = outcome(&case, vec![("Bash", vec![])]);
    assert!(matches!(classify(&clean, &list), Verdict::Stale(_)));
    assert_eq!(classify(&clean, &[]), Verdict::Pass);
    let mut errored = clean.clone();
    errored.error = Some(CaseError {
        kind: "ActionRefused".into(),
        message: "x".into(),
    });
    assert!(matches!(classify(&errored, &[]), Verdict::Fail(_)));
    let mut sim_failed = clean;
    sim_failed.steps[0].sim_after = Err(SimFailure::Sim("x".into()));
    assert!(matches!(classify(&sim_failed, &[]), Verdict::Fail(_)));
}

#[test]
fn every_expected_entry_names_a_case_and_an_issue() {
    let tags: BTreeSet<String> = cases(&[]).iter().flat_map(|c| c.cards.clone()).collect();
    for e in EXPECTED {
        assert!(tags.contains(e.card), "{} is not a case card", e.card);
        let prefixes = [
            "player.", "enemies[", "round", "hand", "draw", "discard", "exhaust",
        ];
        assert!(
            prefixes.iter().any(|p| e.field.starts_with(p)),
            "{} is not a diff path",
            e.field
        );
        assert!(e.issue.starts_with("HOL-"), "{e:?}");
    }
}

#[test]
fn the_draw_pile_keeps_its_top_first() {
    let f = Fixture::new(sts_bridge::NIBBITS_WEAK)
        .enemy(sts_bridge::EnemySpec::new(40).block(100))
        .hand([CardId::PommelStrike])
        .draw([CardId::DefendIronclad, CardId::StrikeIronclad]);
    let s = sim_step(
        &synth(&f),
        &Carry::fresh(1),
        &step_play(CardId::PommelStrike),
    )
    .unwrap();
    assert_eq!(s.after.hand, ["DefendIronclad"]);
    assert_eq!(s.after.draw, ["StrikeIronclad"]);
}

#[test]
fn carried_counters_survive_a_step() {
    // Hp lost to the enemy's attack is carried into the next turn's Spite
    // (5 damage, twice) and Tear Asunder (5 damage, one more hit per hp loss).
    let block_lost = |id: CardId| {
        let f = Fixture::new(sts_bridge::NIBBITS_WEAK)
            .enemy(sts_bridge::EnemySpec::new(40).block(400))
            .draw([id]);
        let steps = vec![
            ParityStep {
                action: StepAction::EndTurn,
                card: id.as_str().into(),
            },
            step_play(id),
        ];
        let case = ParityCase {
            name: "carry".into(),
            cards: vec![id.as_str().into()],
            fixture: f,
            steps,
            masks: Vec::new(),
            blocked: None,
        };
        let r = run_case(&case);
        let blocks: Vec<i32> = r
            .iter()
            .map(|r| r.as_ref().unwrap().enemies[0].hp)
            .collect();
        // The enemy's block resets on its turn, so the hits land on hp.
        blocks[0] - blocks[1]
    };
    assert_eq!(block_lost(CardId::Spite), 10);
    assert_eq!(block_lost(CardId::TearAsunder), 10);
    // Stomp costs 1 after two Attacks.
    let stomp = run_case(&first_case("stomp"));
    let energy: Vec<i32> = stomp
        .iter()
        .map(|r| r.as_ref().unwrap().player.energy)
        .collect();
    assert_eq!(energy[2], energy[1] - 1, "Stomp costs 1 after two Attacks");
}

#[test]
fn outcomes_round_trip_through_serde_json() {
    let case = first_case("bash");
    let json = serde_json::to_string(&case).unwrap();
    assert_eq!(serde_json::from_str::<ParityCase>(&json).unwrap(), case);
    let o = outcome(&case, vec![("Bash", vec![("enemies[0].hp", "1", "2")])]);
    let json = serde_json::to_string(&o).unwrap();
    assert_eq!(serde_json::from_str::<ParityOutcome>(&json).unwrap(), o);
    let step = &o.steps[0];
    let json = serde_json::to_string(step).unwrap();
    assert_eq!(&serde_json::from_str::<StepOutcome>(&json).unwrap(), step);
}

#[test]
fn blocked_cases_are_unreachable_or_need_the_selection_action() {
    let blocked: Vec<ParityCase> = cases(&[])
        .into_iter()
        .filter(|c| c.blocked.is_some())
        .collect();
    assert_eq!(blocked.len(), 7);
    for case in blocked {
        let b = case.blocked.as_ref().unwrap();
        assert_eq!(b.by, "HOL-103");
        assert!(
            !case.steps.is_empty() && !case.cards.is_empty(),
            "{}",
            case.name
        );
        match b.needs {
            Needs::Setters => {
                assert!(
                    case.blocked_for(Setters::default()).is_some(),
                    "{} is reachable",
                    case.name
                );
                // It runs by itself once the setter exists.
                assert!(case
                    .fixture
                    .unreachable(Setters::default())
                    .iter()
                    .any(|f| f.starts_with("piles.exhaust")));
            }
            Needs::HandSelection => {
                assert!(b.reason.contains("hand-selection"), "{}", case.name);
                assert!(case.blocked_for(Setters::default()).is_some());
            }
        }
    }
}

#[test]
fn rage_gains_block_from_the_games_amount() {
    // The game reports RagePower 3: one sim copy, whose effect is 3 block.
    let f = Fixture::new(sts_bridge::NIBBITS_WEAK)
        .enemy(sts_bridge::EnemySpec::new(40).block(100))
        .hand([CardId::Rage, CardId::StrikeIronclad]);
    let case = ParityCase {
        name: "rage".into(),
        cards: vec!["Rage".into()],
        fixture: f,
        steps: vec![step_play(CardId::Rage), step_play(CardId::StrikeIronclad)],
        masks: Vec::new(),
        blocked: None,
    };
    let r = run_case(&case);
    let after_rage = r[0].as_ref().unwrap();
    assert_eq!(after_rage.player.powers.get("Rage"), Some(&3));
    assert_eq!(r[1].as_ref().unwrap().player.block, 3);
    // An amount that is not whole copies shows as a diff, not a silent round.
    let mut game = synth(&case.fixture);
    game.combat
        .as_mut()
        .unwrap()
        .player
        .powers
        .push(power("RAGE_POWER", 5));
    let s = sim_step(&game, &Carry::fresh(1), &step_play(CardId::StrikeIronclad)).unwrap();
    assert_eq!(s.after.player.powers.get("Rage"), Some(&3));
}

#[test]
fn setup_strike_strength_is_split_from_the_temporary_part() {
    let f = Fixture::new(sts_bridge::NIBBITS_WEAK)
        .enemy(sts_bridge::EnemySpec::new(40).block(100))
        .hand([CardId::SetupStrike]);
    let r = run_case(&ParityCase {
        name: "setup".into(),
        cards: vec!["Setup Strike".into()],
        fixture: f,
        steps: vec![step_play(CardId::SetupStrike)],
        masks: Vec::new(),
        blocked: None,
    });
    let p = &r[0].as_ref().unwrap().player.powers;
    // The game shows Strength 2 beside SetupStrikePower 2.
    assert_eq!(p.get("Strength"), Some(&2));
    assert_eq!(p.get("SetupStrikePower"), Some(&2));
    // And it maps back to the sim's StrengthThisTurn(2) with no permanent Strength.
    let mut game = synth(
        &Fixture::new(sts_bridge::NIBBITS_WEAK)
            .enemy(sts_bridge::EnemySpec::new(40).block(100))
            .hand([CardId::StrikeIronclad]),
    );
    let c = game.combat.as_mut().unwrap();
    c.player.powers.push(power("STRENGTH_POWER", 2));
    c.player.powers.push(power("SETUP_STRIKE_POWER", 2));
    let s = sim_step(&game, &Carry::fresh(1), &step_play(CardId::StrikeIronclad)).unwrap();
    assert_eq!(s.after.player.powers.get("Strength"), Some(&2));
    assert_eq!(s.after.enemies[0].block, 100 - 8);
}

#[test]
fn a_recorded_step_replays_to_the_same_sim_state() {
    let case = first_case("stomp");
    let mut state = synth(&case.fixture);
    let template = state.clone();
    let mut carry = Carry::fresh(1);
    for step in &case.steps {
        let s = sim_step(&state, &carry, step).unwrap();
        let after = to_game(&s.after, &template);
        let (rec, next) = record_step(&state, &carry, step, &after, &[]).unwrap();
        assert_eq!(rec.carry, carry, "the carry going in is recorded");
        let replay = sim_step(&rec.before, &rec.carry, &rec.step).unwrap();
        assert_eq!(Ok(replay.after), rec.sim_after);
        assert_eq!(replay.carry, next);
        state = after;
        carry = next;
    }
}

#[test]
fn a_filter_selects_a_card_and_its_variants() {
    let one = |f: &[&str]| -> Vec<String> {
        let f: Vec<String> = f.iter().map(|s| s.to_string()).collect();
        cases(&f).iter().flat_map(|c| c.cards.clone()).collect()
    };
    assert_eq!(one(&["Strike"]), ["Strike", "Strike+"]);
    assert_eq!(one(&["Strike+"]), ["Strike+"]);
    let shakedown = one(&["Strike", "Thunderclap", "Inflame"]);
    assert_eq!(shakedown.len(), 6, "{shakedown:?}");
}
