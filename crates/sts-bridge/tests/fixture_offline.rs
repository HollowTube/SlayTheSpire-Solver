//! Verified combat fixtures without a game: `Fixture::diff` and `plan` as
//! pure functions, and `Fixtures::ensure` against `FakeGame`.

mod common;

use common::{dirty, fake_fixtures, Dirt, FakeGame, FakeScreen, IN_RUN_DIRT};
use sts_bridge::fixture::{DEFAULT_SEED, NEEDS_SETTERS};
use sts_bridge::{
    plan, Backend, BridgeError, CardId, CardRef, CardSpec, CombatView, Command, ConsoleCommand,
    EnemyPowers, EnemySpec, EnemyView, Energy, Entity, Fixture, FixtureError, FixturePath,
    GameState, HandCard, Hp, InstantMode, Piles, PlayerView, Power, PowerName, Relic, RunInfo,
    RunSpec, Screen, Session, Setters, Step, NIBBITS_WEAK,
};

const STRIKE: CardId = CardId::StrikeIronclad;
const DEFEND: CardId = CardId::DefendIronclad;
const BASH: CardId = CardId::Bash;

fn power(id: &str) -> PowerName {
    PowerName::new(id).unwrap()
}

fn spec() -> RunSpec {
    RunSpec::new("Ironclad", DEFAULT_SEED)
}

fn pile_card(id: CardId) -> CardRef {
    CardRef {
        id: Entity::Known(id),
        upgraded: false,
        cost: 1,
        kind: "Attack".into(),
    }
}

fn in_hand(id: CardId, upgraded: bool, index: usize) -> HandCard {
    HandCard {
        index,
        id: Entity::Known(id),
        upgraded,
        cost: 1,
        kind: "Attack".into(),
        playable: true,
        unplayable_reason: None,
        target_type: "AnyEnemy".into(),
        valid_targets: Some(vec![0]),
    }
}

fn game_power(name: &str, amount: i32) -> Power {
    Power {
        id: Entity::parse(name.into()),
        amount,
        kind: None,
    }
}

/// A fight just started on a blank deck: player 80/80, energy 3, one Nibbit
/// at 45, every pile empty.
fn fresh() -> GameState {
    GameState {
        screen: Screen("COMBAT_PLAYER_TURN".into()),
        run: Some(RunInfo {
            act: 1,
            floor: 1,
            hp: 80,
            max_hp: 80,
            gold: 99,
            character: Some("Ironclad".into()),
            ascension: 0,
            seed: DEFAULT_SEED.into(),
            relics: vec!["BurningBlood".into()],
            deck: Vec::new(),
        }),
        combat: Some(CombatView {
            round: 1,
            is_player_turn: true,
            player: PlayerView {
                hp: 80,
                max_hp: 80,
                block: 0,
                energy: 3,
                max_energy: 3,
                powers: Vec::new(),
            },
            enemies: vec![EnemyView {
                index: 0,
                entity_id: "Nibbit_0".into(),
                name: Entity::parse("Nibbit".into()),
                hp: 45,
                max_hp: 45,
                block: 0,
                is_alive: true,
                powers: Vec::new(),
                intent: None,
            }],
            piles: Piles {
                hand: Vec::new(),
                draw: Vec::new(),
                discard: Vec::new(),
                exhaust: Vec::new(),
            },
        }),
    }
}

fn edit(mut s: GameState, f: impl FnOnce(&mut CombatView)) -> GameState {
    f(s.combat.as_mut().unwrap());
    s
}

fn nibbit(hp: i32) -> Fixture {
    Fixture::new(NIBBITS_WEAK).enemy(EnemySpec::new(hp))
}

fn fields(diffs: &[sts_bridge::FieldDiff]) -> Vec<&str> {
    diffs.iter().map(|d| d.field.as_str()).collect()
}

fn consoles(steps: Vec<Step>) -> Vec<String> {
    steps
        .into_iter()
        .map(|s| match s {
            Step::Console(c) => c.line(),
            _ => unreachable!("only console steps exist"),
        })
        .collect()
}

// ---- diff ----

#[test]
fn a_state_that_is_the_fixture_has_no_diff() {
    assert_eq!(nibbit(45).diff(&fresh()), []);
}

#[test]
fn the_diff_names_every_field_that_differs() {
    let state = edit(fresh(), |c| {
        c.player.energy = 2;
        c.enemies[0].hp = 40;
        c.player.powers.push(game_power("StrengthPower", 1));
        c.piles.hand.push(in_hand(BASH, false, 0));
        c.piles.draw.push(pile_card(STRIKE));
    });
    let mut state = state;
    state.run.as_mut().unwrap().relics.push("Anchor".into());
    let diff = nibbit(45).diff(&state);
    assert_eq!(
        fields(&diff),
        [
            "run.relics",
            "player.energy",
            "player.powers[STRENGTH_POWER]",
            "enemies[0].hp",
            "piles.hand",
            "piles.draw"
        ]
    );
    assert!(diff.iter().all(|d| !d.needs_setter));
    let energy = &diff[1];
    assert_eq!(
        (energy.expected.as_str(), energy.observed.as_str()),
        ("3", "2")
    );
}

#[test]
fn the_diff_checks_pile_order_and_upgrades() {
    let fixture = nibbit(45)
        .hand([CardSpec::from(BASH), CardSpec::upgraded(STRIKE)])
        .draw([DEFEND, STRIKE]);
    let state = edit(fresh(), |c| {
        c.piles.hand = vec![in_hand(BASH, false, 0), in_hand(STRIKE, true, 1)];
        c.piles.draw = vec![pile_card(DEFEND), pile_card(STRIKE)];
    });
    assert_eq!(fixture.diff(&state), []);
    let swapped = edit(state.clone(), |c| c.piles.draw.reverse());
    assert_eq!(fields(&fixture.diff(&swapped)), ["piles.draw"]);
    let plain = edit(state, |c| c.piles.hand[1].upgraded = false);
    assert_eq!(fields(&fixture.diff(&plain)), ["piles.hand"]);
}

#[test]
fn outside_a_fight_the_diff_names_the_combat() {
    let mut state = fresh();
    state.combat = None;
    state.screen = Screen("REWARD".into());
    assert_eq!(fields(&nibbit(45).diff(&state)), ["combat"]);
}

#[test]
fn the_diff_names_the_run_and_the_enemy_count() {
    let mut other = fresh();
    other.run.as_mut().unwrap().seed = "OTHER".into();
    assert_eq!(fields(&nibbit(45).diff(&other)), ["run.seed"]);
    let two = Fixture::new(NIBBITS_WEAK)
        .enemy(EnemySpec::new(45))
        .enemy(EnemySpec::new(45));
    assert_eq!(fields(&two.diff(&fresh())), ["enemies"]);
}

#[test]
fn the_projection_ignores_undeclared_fields() {
    let fixture = nibbit(45);
    let other = edit(fresh(), |c| {
        c.enemies[0].max_hp = 46;
        c.round = 2;
    });
    assert_eq!(fixture.project(&fresh()), fixture.project(&other));
    let hurt = edit(fresh(), |c| c.enemies[0].hp = 44);
    assert_ne!(fixture.project(&fresh()), fixture.project(&hurt));
}

// ---- plan ----

#[test]
fn plan_reaches_each_field_from_a_fresh_fight_in_order() {
    let vulnerable = power("VULNERABLE_POWER");
    let strength = power("STRENGTH_POWER");
    let fixture = nibbit(20)
        .player_hp(Hp::Exact(60))
        .player_block(5)
        .energy(Energy::Exact(5))
        .player_power(strength, 2)
        .hand([CardSpec::from(BASH), CardSpec::upgraded(STRIKE)])
        .draw([DEFEND, STRIKE])
        .discard([BASH])
        .exhaust([DEFEND]);
    let mut fixture = fixture;
    fixture.enemies[0] = EnemySpec::new(20).block(4).powers([(vulnerable, 3)]);
    let steps = consoles(plan(&fixture, &fresh(), Setters::default()).unwrap());
    assert_eq!(
        steps,
        [
            "damage 20 0",
            "damage 25 1",
            "power STRENGTH_POWER 2 0",
            "power VULNERABLE_POWER 3 1",
            "block 5 0",
            "block 4 1",
            "energy 2",
            "card BASH hand",
            "card STRIKE_IRONCLAD hand",
            "upgrade 1",
            "card DEFEND_IRONCLAD draw",
            "card STRIKE_IRONCLAD draw",
            "card BASH discard",
            "card DEFEND_IRONCLAD exhaust",
        ]
    );
}

#[test]
fn plan_heals_up_to_max_and_sends_nothing_for_a_fresh_match() {
    let hurt = edit(fresh(), |c| c.player.hp = 30);
    let steps = consoles(plan(&nibbit(45), &hurt, Setters::default()).unwrap());
    assert_eq!(steps, ["heal 50"]);
    assert_eq!(plan(&nibbit(45), &fresh(), Setters::default()).unwrap(), []);
}

#[test]
fn plan_damages_through_existing_block() {
    let blocked = edit(fresh(), |c| c.enemies[0].block = 6);
    let steps = consoles(plan(&nibbit(40), &blocked, Setters::default()).unwrap());
    assert_eq!(steps, ["damage 11 1"]);
}

#[test]
fn plan_names_each_field_no_console_command_reaches() {
    let weak = power("WEAK_POWER");
    let cases: Vec<(&str, Fixture, GameState, &str)> = vec![
        (
            "enemy hp above its spawn",
            nibbit(50),
            fresh(),
            "enemies[0].hp",
        ),
        (
            "player hp above max",
            nibbit(45).player_hp(Hp::Exact(90)),
            fresh(),
            "player.hp",
        ),
        (
            "energy below the start",
            nibbit(45).energy(Energy::Exact(1)),
            fresh(),
            "player.energy",
        ),
        (
            "a spawned power lowered",
            {
                let mut f = nibbit(45);
                f.enemies[0].powers = EnemyPowers::Exact(vec![(weak, 1)]);
                f
            },
            edit(fresh(), |c| {
                c.enemies[0].powers.push(game_power("WeakPower", 2))
            }),
            "enemies[0].powers[WEAK_POWER]",
        ),
        (
            "a spawned power removed",
            nibbit(45),
            edit(fresh(), |c| {
                c.player.powers.push(game_power("WeakPower", 1))
            }),
            "player.powers[WEAK_POWER]",
        ),
        (
            "a fresh exhaust pile",
            nibbit(45),
            edit(fresh(), |c| c.piles.exhaust.push(pile_card(BASH))),
            "piles.exhaust",
        ),
        (
            "a discard pile over a hand",
            nibbit(45),
            edit(fresh(), |c| c.piles.discard = vec![pile_card(BASH); 11]),
            "piles.discard",
        ),
    ];
    for (case, fixture, state, field) in cases {
        let unreachable = plan(&fixture, &state, Setters::default())
            .err()
            .unwrap_or_else(|| panic!("{case}: planned"));
        assert_eq!(fields(&unreachable), [field], "{case}");
        assert!(unreachable[0].needs_setter, "{case}");
    }
}

#[test]
fn a_setter_makes_its_field_reachable_with_no_change_to_the_fixture() {
    let fixture = nibbit(50).energy(Energy::Exact(1));
    let none = plan(&fixture, &fresh(), Setters::default()).unwrap_err();
    assert_eq!(fields(&none), ["enemies[0].hp", "player.energy"]);
    let mut setters = Setters::default();
    setters.exact_hp = true;
    let some = plan(&fixture, &fresh(), setters).unwrap_err();
    assert_eq!(fields(&some), ["player.energy"]);
    setters.energy = true;
    assert!(plan(&fixture, &fresh(), setters).is_ok());
}

#[test]
fn plan_clears_piles_a_fight_opens_with() {
    let dealt = edit(fresh(), |c| {
        c.piles.hand = vec![in_hand(STRIKE, false, 0)];
        c.piles.draw = vec![pile_card(DEFEND), pile_card(BASH)];
        c.piles.discard = vec![pile_card(STRIKE)];
    });
    let steps = consoles(plan(&nibbit(45).hand([BASH]), &dealt, Setters::default()).unwrap());
    assert_eq!(
        steps,
        [
            "remove_card STRIKE_IRONCLAD hand",
            "draw 2",
            "remove_card DEFEND_IRONCLAD hand",
            "remove_card BASH hand",
            "draw 1",
            "remove_card STRIKE_IRONCLAD hand",
            "card BASH hand",
        ]
    );
}

#[test]
fn static_limits_are_named_without_a_game() {
    let cases: Vec<(Fixture, &str)> = vec![
        (nibbit(45).draw([CardSpec::upgraded(BASH)]), "piles.draw"),
        (
            nibbit(45).exhaust([CardSpec::upgraded(BASH)]),
            "piles.exhaust",
        ),
        (
            nibbit(45).player_power(power("STRENGTH_POWER"), -1),
            "player.powers[STRENGTH_POWER]",
        ),
        (nibbit(45).hand(vec![BASH; 11]), "piles.hand"),
        (nibbit(45).player_hp(Hp::Exact(0)), "player.hp"),
        (nibbit(0), "enemies[0].hp"),
    ];
    for (fixture, field) in cases {
        assert_eq!(fixture.unreachable(Setters::default()), [field]);
    }
    let mut setters = Setters::default();
    setters.upgraded_in_piles = true;
    assert_eq!(
        nibbit(45)
            .draw([CardSpec::upgraded(BASH)])
            .unreachable(setters),
        Vec::<String>::new()
    );
}

// ---- ensure, against FakeGame ----

fn sample() -> Fixture {
    nibbit(20)
        .player_hp(Hp::Exact(60))
        .player_power(power("STRENGTH_POWER"), 1)
        .hand([CardSpec::from(BASH), CardSpec::upgraded(STRIKE)])
        .draw([DEFEND, STRIKE])
}

#[test]
fn from_the_main_menu_ensure_resets_and_verifies() {
    let mut fx = fake_fixtures(FakeGame::new());
    let v = fx.ensure(&sample()).unwrap();
    assert_eq!(sample().diff(&v.state), []);
    assert_eq!(v.path, FixturePath::Reset);
    assert_eq!(v.timings.resets, 1);
    assert!(v.timings.reset.unwrap().to_first_fight.is_some());
    assert_eq!(v.instant, InstantMode::Unverified);
    let fake = fx.game().backend();
    assert_eq!(
        fake.run.as_ref().unwrap().deck,
        [],
        "the master deck is blank"
    );
    assert_eq!(fake.start_runs(), 1);
}

#[test]
fn fifty_dirtied_iterations_all_verify_and_are_identical() {
    let fixture = sample();
    let mut fx = fake_fixtures(FakeGame::new());
    let first = fx.ensure(&fixture).unwrap();
    let projection = fixture.project(&first.state);
    let mut paths = Vec::new();
    for i in 0..50 {
        let dirt = match i {
            24 => Dirt::MainMenu,
            49 => Dirt::GameOver,
            _ => IN_RUN_DIRT[i % IN_RUN_DIRT.len()],
        };
        dirty(&mut fx, dirt).unwrap_or_else(|e| panic!("{i}: dirty {dirt:?}: {e}"));
        let v = fx
            .ensure(&fixture)
            .unwrap_or_else(|e| panic!("{i} after {dirt:?}: {e}"));
        assert_eq!(fixture.project(&v.state), projection, "{i} after {dirt:?}");
        paths.push(v.path);
    }
    let resets = paths.iter().filter(|p| **p == FixturePath::Reset).count();
    assert_eq!(resets, 2, "only the main menu and the game over reset");
    assert_eq!(fx.resets(), 3);
}

#[test]
fn spawn_hp_and_intent_vary_but_the_fixture_does_not() {
    let fixture = sample();
    let mut fx = fake_fixtures(FakeGame::new());
    let mut spawned = std::collections::BTreeSet::new();
    let mut projections = std::collections::BTreeSet::new();
    for _ in 0..12 {
        let v = fx.ensure(&fixture).unwrap();
        projections.insert(format!("{:?}", fixture.project(&v.state)));
        spawned.insert(v.state.combat.unwrap().enemies[0].max_hp);
    }
    assert_eq!(projections.len(), 1);
    assert!(spawned.len() > 1, "spawn hp never varied: {spawned:?}");
}

#[test]
fn a_dynamic_limit_resets_once_then_names_the_fields() {
    let mut fx = fake_fixtures(FakeGame::new());
    fx.ensure(&sample()).unwrap();
    let before = fx.resets();
    let fixture = nibbit(99).energy(Energy::Exact(1));
    match fx.ensure(&fixture) {
        Err(FixtureError::Mismatch {
            fields: diffs,
            after_reset: true,
        }) => {
            assert_eq!(fields(&diffs), ["enemies[0].hp", "player.energy"]);
            assert!(diffs.iter().all(|d| d.needs_setter));
        }
        other => panic!("expected Mismatch after a reset, got {other:?}"),
    }
    assert_eq!(fx.resets(), before + 1);
}

#[test]
fn a_static_limit_fails_at_once_and_sends_nothing() {
    let mut fx = fake_fixtures(FakeGame::new());
    let fixture = nibbit(45).discard([CardSpec::upgraded(BASH)]);
    match fx.ensure(&fixture) {
        Err(FixtureError::Unreachable { fields, needs }) => {
            assert_eq!(fields, ["piles.discard"]);
            assert_eq!(needs, NEEDS_SETTERS);
        }
        other => panic!("expected Unreachable, got {other:?}"),
    }
    let fake = fx.game().backend();
    assert_eq!(fake.sent, []);
    assert_eq!(fake.reads, 0);
}

#[test]
fn a_step_the_game_ignores_falls_back_to_one_reset() {
    let mut fx = fake_fixtures(FakeGame::in_run(&spec()));
    fx.ensure(&sample()).unwrap();
    fx.game_mut().backend_mut().ignore_next = Some("card BASH hand");
    let v = fx.ensure(&sample()).unwrap();
    assert_eq!(v.path, FixturePath::Reset);
    assert_eq!(v.timings.resets, 1);
}

#[test]
fn a_run_that_is_loading_is_waited_for_and_nothing_is_sent_meanwhile() {
    let mut game = FakeGame::in_run(&spec());
    game.screen = FakeScreen::Loading(5);
    let mut fx = fake_fixtures(game);
    let v = fx.ensure(&sample()).unwrap();
    assert_eq!(v.path, FixturePath::InRun);
}

#[test]
fn a_run_with_another_seed_is_reset() {
    let mut fx = fake_fixtures(FakeGame::in_run(&RunSpec::new("Ironclad", "OTHER")));
    let v = fx.ensure(&sample()).unwrap();
    assert_eq!(v.path, FixturePath::Reset);
    let sent = &fx.game().backend().sent;
    assert!(matches!(sent[0], Command::Navigate(_)), "{sent:?}");
}

#[test]
fn instant_mode_is_never_toggled_when_the_bridge_does_not_report_it() {
    let mut fx = fake_fixtures(FakeGame::in_run(&spec()));
    let v = fx.ensure(&sample()).unwrap();
    assert_eq!(v.instant, InstantMode::Unverified);
    let toggles = |fake: &FakeGame| {
        fake.sent
            .iter()
            .filter(|c| matches!(c, Command::Console(ConsoleCommand::ToggleInstant)))
            .count()
    };
    assert_eq!(toggles(fx.game().backend()), 0);

    let mut game = FakeGame::in_run(&spec());
    game.fast_mode = Some(false);
    let mut fx = fake_fixtures(game);
    assert_eq!(fx.ensure(&sample()).unwrap().instant, InstantMode::On);
    fx.ensure(&sample()).unwrap();
    assert_eq!(toggles(fx.game().backend()), 1);
}

#[test]
fn without_deck_removal_each_fight_clears_its_piles() {
    let mut game = FakeGame::in_run(&spec());
    game.deck_removal = false;
    let mut fx = fake_fixtures(game);
    for _ in 0..3 {
        let v = fx.ensure(&sample()).unwrap();
        assert_eq!(sample().diff(&v.state), []);
        assert_eq!(v.path, FixturePath::InRun);
    }
    assert_eq!(fx.game().backend().run.as_ref().unwrap().deck.len(), 10);
}

#[test]
fn relics_are_normalised_before_the_fight() {
    let anchor = Relic::new("ANCHOR").unwrap();
    let mut fx = fake_fixtures(FakeGame::in_run(&spec()));
    let fixture = sample().relics([anchor]);
    let v = fx.ensure(&fixture).unwrap();
    assert_eq!(v.state.run.unwrap().relics, ["Anchor"]);
    let lines = fx.game().backend().console_lines();
    let fight = lines.iter().position(|l| l.starts_with("fight")).unwrap();
    let relics: Vec<usize> = (0..lines.len())
        .filter(|i| lines[*i].starts_with("relic"))
        .collect();
    assert_eq!(relics.len(), 2);
    assert!(relics.iter().all(|i| *i < fight), "{lines:?}");
}

// ---- Game::start_run and StartRun ----

#[test]
fn start_run_goes_through_the_main_menu_and_verifies_the_run() {
    let mut game = FakeGame::in_run(&RunSpec::new("Ironclad", "OTHER"));
    game.screen = FakeScreen::Combat;
    game.combat = fresh().combat;
    let mut fx = fake_fixtures(game);
    let run = fx.game_mut().start_run(&spec()).unwrap();
    assert_eq!(run.state.run.unwrap().seed, DEFAULT_SEED);
    assert!(run.accepted_after <= run.ready_after);
    let sent: Vec<String> = fx
        .game()
        .backend()
        .sent
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        sent,
        [
            "navigate(abandon_run)",
            "navigate(leave_game_over)",
            "start_run(Ironclad seed HOL108FIX ascension 0)"
        ]
    );
}

#[test]
fn start_run_fails_when_the_run_is_another_one() {
    let mut game = FakeGame::new();
    game.seed_override = Some("NOTIT".into());
    let mut fx = fake_fixtures(game);
    match fx.game_mut().start_run(&spec()) {
        Err(BridgeError::PostconditionFailed { observed, .. }) => {
            assert!(observed.contains("NOTIT"), "{observed}")
        }
        other => panic!("expected PostconditionFailed, got {other:?}"),
    }
}

#[test]
fn a_session_refuses_start_run_without_sending() {
    let mut s = Session::with_policy(FakeGame::new(), common::fast_policy());
    match s.act(&Command::StartRun(spec())) {
        Err(BridgeError::ActionRefused { reason, .. }) => {
            assert!(reason.contains("Game::start_run"), "{reason}")
        }
        other => panic!("expected ActionRefused, got {other:?}"),
    }
    assert_eq!(s.backend().sent, []);
    assert_eq!(s.backend().reads, 0, "refused before any settle wait");
}

#[test]
fn a_failure_before_the_fight_also_falls_back_to_one_reset() {
    let mut fx = fake_fixtures(FakeGame::in_run(&spec()));
    fx.ensure(&sample()).unwrap();
    fx.game_mut().backend_mut().fail_probes = 1;
    let v = fx.ensure(&sample()).unwrap();
    assert_eq!(v.path, FixturePath::Reset);
    assert_eq!(v.timings.resets, 1);
}

#[test]
fn plan_applies_the_static_limits_too() {
    let unsafe_hp = nibbit(45).player_hp(Hp::Exact(0));
    let refused = plan(&unsafe_hp, &fresh(), Setters::default()).unwrap_err();
    assert_eq!(fields(&refused), ["player.hp"]);
    assert!(refused[0].needs_setter);
}

#[test]
fn a_power_declared_at_zero_is_an_absent_power() {
    let fixture = nibbit(45).player_power(power("WEAK_POWER"), 0);
    assert_eq!(
        fixture.unreachable(Setters::default()),
        Vec::<String>::new()
    );
    assert_eq!(plan(&fixture, &fresh(), Setters::default()).unwrap(), []);
    assert_eq!(fixture.diff(&fresh()), []);
}

#[test]
fn the_mcptest_backend_sends_start_run_with_the_spec() {
    use sts_bridge::mcptest::ReplayTransport;
    let mut t = ReplayTransport::new();
    t.push(
        "start_run",
        r#"{"result":{"success":true,"seed":"S"},"id":1}"#.to_string(),
    );
    let mut backend = sts_bridge::McpTestBackend::new(t);
    backend
        .send(&Command::StartRun(
            RunSpec::new("Ironclad", "S").with_ascension(2),
        ))
        .unwrap();
    let sent = backend.transport().requests().to_vec();
    assert_eq!(sent[0].method, "start_run");
    assert_eq!(
        sent[0].params,
        Some(serde_json::json!({"character": "Ironclad", "seed": "S", "ascension": 2}))
    );
}

#[test]
fn a_backend_behind_a_reference_keeps_its_setters() {
    let mut game = FakeGame::new();
    game.setters.exact_hp = true;
    let by_ref: &mut FakeGame = &mut game;
    assert!(Backend::setters(&by_ref).exact_hp);
}
