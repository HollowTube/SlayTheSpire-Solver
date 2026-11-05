//! An in-memory game for fixture tests: the dev console's effects and limits,
//! a run that loads for a while after `start_run`, Nibbits that spawn at 43 to
//! 46 hp from a seeded RNG, and an opening draw that lands a read after the
//! fight starts. It panics on any command sent while a run loads, which on
//! the real game is a crash.

use std::time::Duration;

use serde_json::json;
use sts_bridge::console::slugify;
use sts_bridge::{
    Ack, Actions, Backend, BridgeError, CardId, CardRef, CombatPile, CombatView, Command,
    ConsoleCommand, ConsoleWait, EnemyView, Entity, Fixtures, Game, GameState, HandCard, Health,
    Intent, IntentPart, Launcher, MenuNav, Piles, PlayerView, Power, Probe, RunInfo, RunSpec,
    Screen, ScreenDetail, Setters, SettlePolicy, Target, Timeouts,
};

/// Where the fake game is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeScreen {
    MainMenu,
    /// A run loading, for this many more probes.
    Loading(usize),
    Event,
    Combat,
    Reward,
    GameOver,
}

#[derive(Debug, Clone)]
pub struct FakeRun {
    pub spec: RunSpec,
    pub hp: i32,
    pub max_hp: i32,
    pub relics: Vec<String>,
    pub deck: Vec<CardRef>,
}

pub struct FakeGame {
    pub screen: FakeScreen,
    pub run: Option<FakeRun>,
    pub combat: Option<CombatView>,
    /// Cards of the opening draw still to land.
    pending_draw: usize,
    rng: u64,
    pub fast_mode: Option<bool>,
    /// When false, `remove_card X deck` reports success and does nothing.
    pub deck_removal: bool,
    /// The next console command whose line starts with this reports success
    /// and does nothing.
    pub ignore_next: Option<&'static str>,
    /// A run reports this seed instead of the one it was started with.
    pub seed_override: Option<String>,
    pub load_probes: usize,
    /// The next this many probes time out, as a bridge that stopped answering.
    pub fail_probes: usize,
    pub setters: Setters,
    /// Every command sent, in order.
    pub sent: Vec<Command>,
    pub reads: usize,
}

fn card(id: CardId) -> CardRef {
    CardRef {
        id: Entity::Known(id),
        upgraded: false,
        cost: 1,
        kind: "Attack".into(),
    }
}

/// `BURNING_BLOOD` -> `BurningBlood`: a console id as the game's class name.
fn class_name(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| {
                f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase()
            })
        })
        .collect()
}

fn starter_deck() -> Vec<CardRef> {
    let mut deck = vec![card(CardId::StrikeIronclad); 5];
    deck.extend(vec![card(CardId::DefendIronclad); 4]);
    deck.push(card(CardId::Bash));
    deck
}

fn refused(command: &Command, reason: &str) -> BridgeError {
    BridgeError::ActionRefused {
        action: command.to_string(),
        reason: reason.into(),
    }
}

impl Default for FakeGame {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeGame {
    /// On the main menu, with no run.
    pub fn new() -> Self {
        Self {
            screen: FakeScreen::MainMenu,
            run: None,
            combat: None,
            pending_draw: 0,
            rng: 0,
            fast_mode: None,
            deck_removal: true,
            ignore_next: None,
            seed_override: None,
            load_probes: 3,
            fail_probes: 0,
            setters: Setters::default(),
            sent: Vec::new(),
            reads: 0,
        }
    }

    /// In `spec`'s run on its opening event, with the starter deck.
    pub fn in_run(spec: &RunSpec) -> Self {
        let mut g = Self::new();
        g.start(spec);
        g.screen = FakeScreen::Event;
        g
    }

    fn start(&mut self, spec: &RunSpec) {
        self.run = Some(FakeRun {
            spec: spec.clone(),
            hp: 80,
            max_hp: 80,
            relics: vec!["BurningBlood".into()],
            deck: starter_deck(),
        });
        self.rng = spec.seed.bytes().fold(1469598103934665603u64, |h, b| {
            (h ^ b as u64).wrapping_mul(1099511628211)
        });
        self.combat = None;
        self.screen = FakeScreen::Loading(self.load_probes);
    }

    fn next(&mut self, n: u64) -> u64 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 33) % n
    }

    /// The console commands sent, as lines.
    pub fn console_lines(&self) -> Vec<String> {
        self.sent
            .iter()
            .filter_map(|c| match c {
                Command::Console(c) => Some(c.line()),
                _ => None,
            })
            .collect()
    }

    pub fn start_runs(&self) -> usize {
        self.sent
            .iter()
            .filter(|c| matches!(c, Command::StartRun(_)))
            .count()
    }

    fn screen_name(&self) -> &'static str {
        match self.screen {
            FakeScreen::MainMenu | FakeScreen::Loading(_) => "MAIN_MENU",
            FakeScreen::Event => "EVENT",
            FakeScreen::Combat => "COMBAT_PLAYER_TURN",
            FakeScreen::Reward => "REWARD",
            FakeScreen::GameOver => "OVERLAY",
        }
    }

    fn run_info(&self) -> Option<RunInfo> {
        let run = self.run.as_ref()?;
        Some(RunInfo {
            act: 1,
            floor: 1,
            hp: run.hp,
            max_hp: run.max_hp,
            gold: 99,
            character: Some(run.spec.character.clone()),
            ascension: run.spec.ascension,
            seed: self
                .seed_override
                .clone()
                .unwrap_or_else(|| run.spec.seed.clone()),
            relics: run.relics.clone(),
            deck: run.deck.clone(),
        })
    }

    fn fight(&mut self) {
        let hp = 43 + self.next(4) as i32;
        let move_id = ["BUTT", "SLICE_AND_HISS"][self.next(2) as usize];
        let run = self.run.as_ref().expect("a run");
        let mut draw = run.deck.clone();
        for i in (1..draw.len()).rev() {
            let j = self.next(i as u64 + 1) as usize;
            draw.swap(i, j);
        }
        let run = self.run.as_ref().expect("a run");
        self.pending_draw = draw.len().min(5);
        self.combat = Some(CombatView {
            round: 1,
            is_player_turn: true,
            player: PlayerView {
                hp: run.hp,
                max_hp: run.max_hp,
                block: 0,
                energy: 3,
                max_energy: 3,
                powers: Vec::new(),
            },
            enemies: vec![EnemyView {
                index: 0,
                entity_id: "Nibbit_0".into(),
                name: Entity::parse("Nibbit".into()),
                hp,
                max_hp: hp,
                block: 0,
                is_alive: true,
                powers: Vec::new(),
                intent: Some(Intent {
                    move_id: move_id.into(),
                    parts: vec![IntentPart {
                        kind: "Attack".into(),
                        damage: Some(12),
                        hits: Some(1),
                        total_damage: Some(12),
                    }],
                }),
            }],
            piles: Piles {
                hand: Vec::new(),
                draw,
                discard: Vec::new(),
                exhaust: Vec::new(),
            },
        });
        self.screen = FakeScreen::Combat;
    }

    fn console(&mut self, command: &Command, c: &ConsoleCommand) -> Result<(), BridgeError> {
        use ConsoleCommand as C;
        if self.run.is_none() {
            return Err(refused(command, "A run is currently not in progress!"));
        }
        if let Some(prefix) = self.ignore_next {
            if c.line().starts_with(prefix) {
                self.ignore_next = None;
                return Ok(());
            }
        }
        let run = self.run.as_mut().expect("checked above");
        match c {
            C::Fight(_) => {
                self.fight();
                return Ok(());
            }
            C::Win => {
                self.combat = None;
                self.screen = FakeScreen::Reward;
                return Ok(());
            }
            C::RelicAdd(r) => {
                run.relics.push(class_name(r.as_str()));
                return Ok(());
            }
            C::RelicRemove(r) => {
                if let Some(i) = run.relics.iter().position(|n| slugify(n) == r.as_str()) {
                    run.relics.remove(i);
                }
                return Ok(());
            }
            C::RemoveCardFromDeck { card } => {
                if let Some(i) = run.deck.iter().position(|c| c.id == Entity::Known(*card)) {
                    if self.deck_removal {
                        run.deck.remove(i);
                    }
                }
                return Ok(());
            }
            C::Probe(line) => {
                let id = line
                    .strip_prefix("card ")
                    .and_then(|r| r.strip_suffix(" deck"));
                if let Some(id) = id.and_then(CardId::from_sts2) {
                    run.deck.push(card(id));
                }
                return Ok(());
            }
            C::ToggleInstant => {
                self.fast_mode = self.fast_mode.map(|on| !on);
                return Ok(());
            }
            _ => {}
        }
        let Some(combat) = self.combat.as_mut() else {
            return Err(refused(command, "not in combat"));
        };
        in_combat(combat, c);
        run.hp = combat.player.hp;
        if combat.enemies.iter().all(|e| !e.is_alive) {
            self.combat = None;
            self.screen = FakeScreen::Reward;
        }
        Ok(())
    }
}

fn hit(hp: &mut i32, block: &mut i32, amount: i32) {
    let through = (amount - *block).max(0);
    *block = (*block - amount).max(0);
    *hp = (*hp - through).max(0);
}

fn add_power(powers: &mut Vec<Power>, id: &str, amount: i32) {
    let name = class_name(id);
    match powers
        .iter_mut()
        .find(|p| slugify(p.id.name()) == slugify(&name) || p.id == Entity::parse(name.clone()))
    {
        Some(p) => p.amount += amount,
        None => powers.push(Power {
            id: Entity::parse(name),
            amount,
            kind: None,
        }),
    }
}

fn hand_card(c: &CardRef) -> HandCard {
    HandCard {
        index: 0,
        id: c.id.clone(),
        upgraded: c.upgraded,
        cost: c.cost,
        kind: c.kind.clone(),
        playable: true,
        unplayable_reason: None,
        target_type: "AnyEnemy".into(),
        valid_targets: Some(vec![0]),
    }
}

fn in_combat(c: &mut CombatView, command: &ConsoleCommand) {
    use ConsoleCommand as C;
    let n = |amount: &u32| *amount as i32;
    match command {
        C::Damage { target, amount } => match target {
            Target::Player => hit(&mut c.player.hp, &mut c.player.block, n(amount)),
            Target::Enemy(i) => {
                let e = &mut c.enemies[*i];
                hit(&mut e.hp, &mut e.block, n(amount));
                e.is_alive = e.hp > 0;
            }
        },
        C::DamageAllEnemies { amount } => {
            for e in &mut c.enemies {
                hit(&mut e.hp, &mut e.block, n(amount));
                e.is_alive = e.hp > 0;
            }
        }
        C::HealPlayer { amount } => c.player.hp = (c.player.hp + n(amount)).min(c.player.max_hp),
        C::Block { target, amount } => match target {
            Target::Player => c.player.block += n(amount),
            Target::Enemy(i) => c.enemies[*i].block += n(amount),
        },
        C::Energy { amount } => c.player.energy += n(amount),
        C::Power {
            power,
            amount,
            target,
        } => match target {
            Target::Player => add_power(&mut c.player.powers, power.as_str(), n(amount)),
            Target::Enemy(i) => add_power(&mut c.enemies[*i].powers, power.as_str(), n(amount)),
        },
        C::AddCard { card: id, pile } => {
            let added = card(*id);
            match pile {
                CombatPile::Hand => c.piles.hand.push(hand_card(&added)),
                CombatPile::Draw => c.piles.draw.push(added),
                CombatPile::Discard => c.piles.discard.push(added),
                CombatPile::Exhaust => c.piles.exhaust.push(added),
            }
        }
        C::RemoveCardFromHand { card: id } => {
            if let Some(i) = c.piles.hand.iter().position(|h| h.id == Entity::Known(*id)) {
                c.piles.hand.remove(i);
            }
        }
        C::Draw { count } => {
            for _ in 0..*count {
                if c.piles.hand.len() >= 10 {
                    break;
                }
                if c.piles.draw.is_empty() {
                    c.piles.draw = std::mem::take(&mut c.piles.discard);
                }
                if c.piles.draw.is_empty() {
                    break;
                }
                let drawn = c.piles.draw.remove(0);
                c.piles.hand.push(hand_card(&drawn));
            }
        }
        C::Upgrade { hand_index } => c.piles.hand[*hand_index].upgraded = true,
        _ => {}
    }
}

impl Backend for FakeGame {
    fn send(&mut self, command: &Command) -> Result<Ack, BridgeError> {
        if let FakeScreen::Loading(_) = self.screen {
            panic!("`{command}` was sent while a run loads: the real game crashes");
        }
        self.sent.push(command.clone());
        match command {
            Command::StartRun(spec) => {
                if self.run.is_some() {
                    return Err(refused(command, "A run is already in progress"));
                }
                self.start(spec);
            }
            Command::Navigate(MenuNav::AbandonRun) if self.run.is_some() => {
                self.combat = None;
                self.screen = FakeScreen::GameOver;
            }
            Command::Navigate(MenuNav::LeaveGameOver) if self.screen == FakeScreen::GameOver => {
                self.run = None;
                self.screen = FakeScreen::MainMenu;
            }
            Command::EndTurn if self.combat.is_some() => {
                let c = self.combat.as_mut().expect("checked above");
                c.round += 1;
                c.player.energy = c.player.max_energy;
                c.player.block = 0;
                c.player.hp = (c.player.hp - 3).max(1);
                if let Some(run) = self.run.as_mut() {
                    run.hp = c.player.hp;
                }
            }
            Command::Console(c) => self.console(command, c)?,
            _ => return Err(refused(command, "not modelled by FakeGame")),
        }
        Ok(Ack::new(json!({"success": true})))
    }

    fn state(&mut self) -> Result<GameState, BridgeError> {
        self.reads += 1;
        let mut combat = self.combat.clone();
        if let Some(c) = combat.as_mut() {
            for (i, h) in c.piles.hand.iter_mut().enumerate() {
                h.index = i;
            }
        }
        let state = GameState {
            screen: Screen(self.screen_name().into()),
            run: self.run_info(),
            combat: if self.screen == FakeScreen::Combat {
                combat
            } else {
                None
            },
        };
        if self.pending_draw > 0 {
            let c = self.combat.as_mut().expect("a fight");
            for _ in 0..self.pending_draw {
                let drawn = c.piles.draw.remove(0);
                c.piles.hand.push(hand_card(&drawn));
            }
            self.pending_draw = 0;
        }
        Ok(state)
    }

    fn actions(&mut self) -> Result<Actions, BridgeError> {
        Ok(Actions {
            screen: Screen(self.screen_name().into()),
            screen_context: None,
            actions: Vec::new(),
        })
    }

    fn health(&mut self) -> Result<Health, BridgeError> {
        Ok(Health {
            ok: true,
            latency_ms: 0.0,
            backend: "FakeGame".into(),
            version: None,
            screen: Some(Screen(self.screen_name().into())),
            run_in_progress: Some(self.run.is_some()),
            in_combat: Some(self.screen == FakeScreen::Combat),
            fast_mode: self.fast_mode,
        })
    }

    fn probe(&mut self) -> Result<Probe, BridgeError> {
        if self.fail_probes > 0 {
            self.fail_probes -= 1;
            return Err(BridgeError::Timeout {
                method: "ping".into(),
                after: Duration::from_millis(1),
            });
        }
        if let FakeScreen::Loading(n) = self.screen {
            self.screen = if n == 0 {
                FakeScreen::Event
            } else {
                FakeScreen::Loading(n - 1)
            };
        }
        let screen = Screen(self.screen_name().into());
        let main_menu = self.screen == FakeScreen::MainMenu;
        Ok(Probe {
            ok: true,
            screen: Some(screen.clone()),
            run_in_progress: Some(self.run.is_some()),
            in_combat: Some(self.screen == FakeScreen::Combat),
            detail: Some(ScreenDetail {
                screen: Some(screen),
                source: Some("active_screen_context".into()),
                room_type: None,
                context: (self.screen == FakeScreen::GameOver).then(|| "NGameOverScreen".into()),
            }),
            main_menu_node: main_menu.then_some(true),
        })
    }

    fn setters(&self) -> Setters {
        self.setters
    }
}

/// A game process that is always up.
pub struct Up;

impl Launcher for Up {
    fn launch(&mut self) -> Result<(), BridgeError> {
        Ok(())
    }

    fn pid(&mut self) -> Option<u32> {
        Some(1)
    }

    fn kill(&mut self, _pid: u32, _grace: Duration) -> Result<(), BridgeError> {
        panic!("FakeGame is never killed");
    }
}

/// Fast polling, short deadlines: the fake answers at once.
pub fn fast_policy() -> SettlePolicy {
    SettlePolicy::default()
        .with_timeout(Duration::from_millis(150))
        .with_poll(Duration::from_millis(1), Duration::from_millis(2))
}

pub fn fast_timeouts() -> Timeouts {
    Timeouts::uniform(Duration::from_millis(200), Duration::from_millis(1))
}

pub fn fake_fixtures(game: FakeGame) -> Fixtures<FakeGame, Up> {
    let game = Game::new(game, Up).with_timeouts(fast_timeouts());
    Fixtures::new(game).with_wait(
        ConsoleWait::for_session(&fast_policy())
            .with_settle(fast_policy())
            .with_room_ready(fast_timeouts()),
    )
}
