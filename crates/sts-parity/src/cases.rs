//! The case table: every Ironclad registry card, base and upgraded.
//!
//! A [`Unit`] is the smallest independent thing to test (one card variant and
//! the plays that exercise it). Units that touch nothing but the state both
//! sides share are batched into one fixture; the rest get their own.

use sts_bridge::{CardSpec, EnemySpec, Energy, Fixture, Hp, PowerName, NIBBITS_WEAK};
use sts_ids::CardId;
use sts_sim::cards::card_data;

use crate::case::{Blocked, Needs, ParityCase};
use crate::step::{Mask, ParityStep, StepAction};

/// Hp the enemy is set to; it never dies (block absorbs the hits).
pub const ENEMY_HP: i32 = 40;
/// Enemy block, which keeps every hit from reaching hp.
pub const ENEMY_BLOCK: i32 = 400;
/// The most hand cards plus draws a batch may hold.
const BATCH_HAND: usize = 9;
const BATCH_PLAYS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    /// One Nibbit.
    Weak,
    /// One Nibbit with Vulnerable.
    Vuln,
    /// Two Nibbits.
    Normal,
}

#[derive(Debug, Clone)]
enum Act {
    Play(CardSpec),
    EndTurn,
}

#[derive(Debug, Clone)]
pub struct Unit {
    card: CardId,
    up: bool,
    group: Group,
    batch: bool,
    /// Hand cards besides the card under test, which goes last.
    hand: Vec<CardSpec>,
    /// Strikes appended to the draw pile.
    fill: usize,
    draw: Vec<CardSpec>,
    discard: Vec<CardSpec>,
    exhaust: Vec<CardSpec>,
    acts: Vec<Act>,
    hp: Option<i32>,
    player_block: i32,
    energy: Option<i32>,
    mask_hand: bool,
    /// Hp of the enemies when not the default.
    enemy_hp: Option<Vec<i32>>,
    enemy_block: Option<Vec<i32>>,
    target: usize,
    /// The unit deliberately kills an enemy.
    pub kills: bool,
    /// Whether the card under test starts in the hand.
    in_hand: bool,
}

fn spec(id: CardId, up: bool) -> CardSpec {
    CardSpec { id, upgraded: up }
}

impl Unit {
    fn new(card: CardId, up: bool) -> Self {
        Self {
            card,
            up,
            group: Group::Weak,
            batch: false,
            hand: Vec::new(),
            fill: 0,
            draw: Vec::new(),
            discard: Vec::new(),
            exhaust: Vec::new(),
            acts: vec![Act::Play(spec(card, up))],
            hp: None,
            player_block: 0,
            energy: None,
            mask_hand: false,
            enemy_hp: None,
            enemy_block: None,
            target: 0,
            kills: false,
            in_hand: true,
        }
    }

    fn b(mut self) -> Self {
        self.batch = true;
        self
    }
    fn group(mut self, g: Group) -> Self {
        self.group = g;
        self
    }
    fn hand(mut self, cards: &[CardId]) -> Self {
        self.hand = cards.iter().map(|c| spec(*c, false)).collect();
        self
    }
    fn fill(mut self, n: usize) -> Self {
        self.fill = n;
        self
    }
    fn draw(mut self, cards: &[CardId]) -> Self {
        self.draw = cards.iter().map(|c| spec(*c, false)).collect();
        self
    }
    fn discard(mut self, cards: &[CardId]) -> Self {
        self.discard = cards.iter().map(|c| spec(*c, false)).collect();
        self
    }
    fn exhaust(mut self, n: usize) -> Self {
        self.exhaust = vec![spec(CardId::StrikeIronclad, false); n];
        self
    }
    /// Plays `cards` (in hand) before the card under test.
    fn before(mut self, cards: &[CardId]) -> Self {
        let mut acts: Vec<Act> = cards.iter().map(|c| Act::Play(spec(*c, false))).collect();
        acts.push(Act::Play(spec(self.card, self.up)));
        self.acts = acts;
        self.hand = cards.iter().map(|c| spec(*c, false)).collect();
        self
    }
    /// Plays the card under test, then `cards`.
    fn after(mut self, cards: &[CardId]) -> Self {
        self.acts
            .extend(cards.iter().map(|c| Act::Play(spec(*c, false))));
        self.hand.extend(cards.iter().map(|c| spec(*c, false)));
        self
    }
    fn end(mut self) -> Self {
        self.acts.push(Act::EndTurn);
        self
    }
    fn hp(mut self, hp: i32) -> Self {
        self.hp = Some(hp);
        self
    }
    fn pblock(mut self, b: i32) -> Self {
        self.player_block = b;
        self
    }
    fn energy(mut self, e: i32) -> Self {
        self.energy = Some(e);
        self
    }
    fn mask(mut self) -> Self {
        self.mask_hand = true;
        self
    }

    pub fn tag(&self) -> String {
        format!(
            "{}{}{}",
            self.card.as_str(),
            if self.up { "+" } else { "" },
            if self.in_hand { "" } else { "/trigger" }
        )
    }

    fn draws(&self) -> usize {
        self.fill + self.draw.len()
    }

    fn plays(&self) -> usize {
        self.acts
            .iter()
            .filter(|a| matches!(a, Act::Play(_)))
            .count()
    }

    fn hand_len(&self) -> usize {
        self.hand.len() + 1
    }

    fn cost(&self) -> i32 {
        self.acts
            .iter()
            .map(|a| match a {
                Act::Play(c) => card_data(c.id, u8::from(c.upgraded)).cost.max(0),
                Act::EndTurn => 0,
            })
            .sum()
    }
}

fn base_units(id: CardId, up: bool) -> Vec<Unit> {
    use CardId as C;
    let u = || Unit::new(id, up);
    let strike = C::StrikeIronclad;
    let defend = C::DefendIronclad;
    let unit = match id {
        C::StrikeIronclad
        | C::DefendIronclad
        | C::Bash
        | C::IronWave
        | C::Bloodletting
        | C::BloodWall
        | C::Hemokinesis
        | C::Tremble
        | C::Impervious
        | C::Bludgeon
        | C::TwinStrike
        | C::Break
        | C::Taunt
        | C::Uppercut
        | C::MoltenFist
        | C::Dominate
        | C::Anger
        | C::FightMe
        | C::SwordBoomerang
        | C::Feed => u().b(),
        C::PommelStrike => u().b().fill(2),
        C::Offering => u().b().fill(if up { 5 } else { 3 }),
        C::ShrugItOff => u().b().fill(1),
        C::Havoc => u().b().fill(1),
        C::BodySlam => u().b().pblock(12),
        C::PerfectedStrike | C::Bully | C::Dismantle => u().b().group(Group::Vuln),
        C::InfernalBlade => u().mask(),
        C::Mangle | C::SetupStrike | C::Rampage => u(),
        C::NotYet => u().hp(60),
        C::Whirlwind => u().energy(3),
        C::Cascade => u().energy(3).fill(4),
        C::BattleTrance => u().fill(if up { 5 } else { 3 }),
        C::Headbutt => u().discard(&[defend]),
        C::Armaments if up => u().hand(&[strike, defend]),
        C::Cinder | C::TrueGrit | C::Thrash | C::Armaments | C::Brand => u().hand(&[strike]),
        C::BurningPact => u().hand(&[strike]).fill(if up { 4 } else { 2 }),
        C::FiendFire => u().hand(&[strike, defend]),
        C::SecondWind => u().hand(&[defend, C::Tremble]),
        C::AshenStrike => u().b().exhaust(2),
        C::ExpectAFight => u().hand(&[strike, strike]),
        C::DrumOfBattle => u().before(&[C::Corruption]).fill(2).energy(4),
        C::Thunderclap | C::Conflagration | C::Breakthrough => u().b().group(Group::Normal),
        C::HowlFromBeyond => u().b().group(Group::Normal),
        C::PactsEnd => u().b().group(Group::Normal).exhaust(3),
        C::Spite | C::TearAsunder => u().before(&[C::Bloodletting]),
        C::EvilEye | C::ForgottenRitual => u().before(&[C::Tremble]),
        C::Stomp => u().before(&[strike, strike]),
        C::Unrelenting | C::Rage | C::OneTwoPunch => u().after(&[strike]),
        C::Inflame => u(),
        C::StoneArmor
        | C::Pyre
        | C::DemonForm
        | C::CrimsonMantle
        | C::Barricade
        | C::FlameBarrier => u().end(),
        C::Inferno => u().group(Group::Normal).end(),
        C::Aggression => u().discard(&[strike]).end(),
        C::DarkEmbrace => u().after(&[C::Tremble]).fill(2),
        C::FeelNoPain => u().after(&[C::Tremble]),
        C::Juggernaut | C::Unmovable => u().after(&[defend, defend]),
        C::Corruption => u().after(&[defend]),
        C::Cruelty => u().after(&[C::Bash, strike]),
        C::Vicious => u().after(&[C::Bash]).fill(2),
        C::Juggling => u().after(&[strike, strike, strike]).energy(4),
        C::Colossus => u().group(Group::Vuln).end(),
        C::Pillage => u().draw(&[strike, defend]),
        C::Hellraiser => u(),
        C::Rupture | C::Stampede => u(),
        C::PrimalForce => u().hand(&[strike]),
        C::Stoke => u().hand(&[strike, defend]).mask(),
        other => panic!("no unit for {other:?}"),
    };
    vec![unit]
}

/// The units of `id`, base then upgraded. Variants that need a different
/// shape than the default (a kill, a trigger from the exhaust pile) are added
/// or replaced here.
pub fn units_for(id: CardId, up: bool) -> Vec<Unit> {
    use CardId as C;
    let mut units = base_units(id, up);
    if id == C::Feed {
        let mut kill = Unit::new(id, up).group(Group::Normal);
        kill.enemy_hp = Some(vec![ENEMY_HP, 10]);
        kill.enemy_block = Some(vec![ENEMY_BLOCK, 0]);
        kill.target = 1;
        kill.kills = true;
        units.push(kill);
    }
    if id == C::HowlFromBeyond && !up {
        let mut trigger = Unit::new(id, up)
            .group(Group::Normal)
            .hand(&[C::StrikeIronclad]);
        trigger.acts = vec![Act::EndTurn];
        trigger.in_hand = false;
        trigger.exhaust = vec![spec(id, false)];
        units.push(trigger);
    }
    units
}

fn power(id: &str) -> PowerName {
    PowerName::new(id).expect("catalog power")
}

fn encounter(g: Group) -> sts_bridge::Encounter {
    match g {
        Group::Weak | Group::Vuln => NIBBITS_WEAK,
        Group::Normal => sts_bridge::Encounter::new("NIBBITS_NORMAL").expect("catalog encounter"),
    }
}

fn build(name: String, units: &[Unit]) -> ParityCase {
    let g = units[0].group;
    let mut fixture = Fixture::new(encounter(g));
    let n = if g == Group::Normal { 2 } else { 1 };
    for i in 0..n {
        let hp = units
            .iter()
            .find_map(|u| u.enemy_hp.as_ref().map(|v| v[i]))
            .unwrap_or(ENEMY_HP);
        let block = units
            .iter()
            .find_map(|u| u.enemy_block.as_ref().map(|v| v[i]))
            .unwrap_or(ENEMY_BLOCK);
        let mut e = EnemySpec::new(hp).block(block);
        if g == Group::Vuln {
            e = e.powers([(power("VULNERABLE_POWER"), 2)]);
        }
        fixture = fixture.enemy(e);
    }
    let mut hand = Vec::new();
    let mut draw = Vec::new();
    let mut discard = Vec::new();
    let mut exhaust = Vec::new();
    let mut fill = 0;
    let mut cost = 0;
    let mut steps = Vec::new();
    let mut cards = Vec::new();
    let mut masks = Vec::new();
    for u in units {
        hand.extend(u.hand.iter().copied());
        if u.in_hand {
            hand.push(spec(u.card, u.up));
        }
        draw.extend(u.draw.iter().copied());
        discard.extend(u.discard.iter().copied());
        exhaust.extend(u.exhaust.iter().copied());
        fill += u.fill;
        cost += u.cost();
        cards.push(u.tag());
        if u.mask_hand && masks.is_empty() {
            masks.push(Mask {
                field: "hand".into(),
                reason: "the game picks the generated cards at random".into(),
            });
        }
        for a in &u.acts {
            let action = match a {
                Act::Play(c) => {
                    let targeted = card_data(c.id, u8::from(c.upgraded)).targeted;
                    StepAction::PlayCard {
                        card: *c,
                        target: targeted.then_some(u.target),
                    }
                }
                Act::EndTurn => StepAction::EndTurn,
            };
            steps.push(ParityStep {
                action,
                card: u.tag(),
            });
        }
    }
    draw.extend(std::iter::repeat(spec(CardId::StrikeIronclad, false)).take(fill));
    let energy = units
        .iter()
        .filter_map(|u| u.energy)
        .max()
        .unwrap_or_else(|| cost.max(3));
    let hp = units.iter().find_map(|u| u.hp);
    fixture = fixture
        .energy(if energy == 3 {
            Energy::Start
        } else {
            Energy::Exact(energy)
        })
        .player_block(units.iter().map(|u| u.player_block).max().unwrap_or(0))
        .hand(hand)
        .draw(draw)
        .discard(discard)
        .exhaust(exhaust);
    if let Some(hp) = hp {
        fixture = fixture.player_hp(Hp::Exact(hp));
    }
    ParityCase {
        name,
        cards,
        fixture,
        steps,
        masks,
        blocked: None,
    }
}

/// `--card` filters: `Name` selects both variants, `Name+` the upgraded one;
/// spaces and case are ignored.
pub fn selected(tag: &str, filter: &[String]) -> bool {
    let norm = |s: &str| s.replace(' ', "").to_ascii_lowercase();
    let t = norm(tag.split('/').next().unwrap_or(tag));
    filter.is_empty()
        || filter.iter().any(|f| {
            let f = norm(f);
            f == t || (!f.ends_with('+') && f == t.trim_end_matches('+'))
        })
}

fn blocked(mut case: ParityCase, reason: &str, needs: Needs) -> ParityCase {
    case.name = format!("blocked-{}", case.name);
    case.blocked = Some(Blocked {
        by: "HOL-103".into(),
        reason: reason.into(),
        needs,
    });
    case
}

/// Cases that wait on HOL-103. A `Needs::Setters` case runs by itself once
/// its fixture is reachable.
fn blocked_cases(filter: &[String]) -> Vec<ParityCase> {
    use CardId as C;
    let strike = C::StrikeIronclad;
    let defend = C::DefendIronclad;
    let mut out = Vec::new();
    let mut howl = Unit::new(C::HowlFromBeyond, true)
        .group(Group::Normal)
        .hand(&[strike]);
    howl.acts = vec![Act::EndTurn];
    howl.in_hand = false;
    howl.exhaust = vec![spec(C::HowlFromBeyond, true)];
    let multi = [
        Unit::new(C::Armaments, false).hand(&[strike, defend]),
        Unit::new(C::Brand, false).hand(&[strike, defend]),
        Unit::new(C::Brand, true).hand(&[strike, defend]),
        Unit::new(C::BurningPact, false)
            .hand(&[strike, defend])
            .fill(2),
        Unit::new(C::BurningPact, true)
            .hand(&[strike, defend])
            .fill(2),
        Unit::new(C::TrueGrit, true).hand(&[strike, defend]),
    ];
    if selected(&howl.tag(), filter) {
        out.push(blocked(
            build("howl-trigger".into(), &[howl]),
            "the end-of-turn trigger from the exhaust pile needs an upgraded card outside the hand (Setters::upgraded_in_piles)",
            Needs::Setters,
        ));
    }
    for unit in multi {
        if selected(&unit.tag(), filter) {
            let reason = format!(
                "{} with two or more eligible cards needs a hand-selection action",
                unit.tag()
            );
            let name = format!("{}-two-eligible", unit.tag().to_ascii_lowercase());
            out.push(blocked(build(name, &[unit]), &reason, Needs::HandSelection));
        }
    }
    out
}

/// The cases of the full list that hold a card selected by `filter`. Unlike
/// [`cases`] it never regroups a batch, so a case keeps its name.
pub fn cases_holding(filter: &[String]) -> Vec<ParityCase> {
    cases(&[])
        .into_iter()
        .filter(|c| c.cards.iter().any(|t| selected(t, filter)))
        .collect()
}

fn slug(tag: &str) -> String {
    tag.to_ascii_lowercase().replace([' ', '/'], "-")
}

/// Every case, filtered by `filter` (empty means all).
pub fn cases(filter: &[String]) -> Vec<ParityCase> {
    let mut batches: Vec<(Group, Vec<Unit>)> = Vec::new();
    let mut alone: Vec<Unit> = Vec::new();
    for &id in IRONCLAD {
        for up in [false, true] {
            for unit in units_for(id, up) {
                if !selected(&unit.tag(), filter) {
                    continue;
                }
                if unit.batch && !unit.kills {
                    let open = batches.iter_mut().find(|(g, us)| {
                        *g == unit.group
                            && us.iter().map(Unit::plays).sum::<usize>() + unit.plays()
                                <= BATCH_PLAYS
                            && us.iter().map(|u| u.hand_len() + u.draws()).sum::<usize>()
                                + unit.hand_len()
                                + unit.draws()
                                <= BATCH_HAND
                    });
                    match open {
                        Some((_, us)) => us.push(unit),
                        None => batches.push((unit.group, vec![unit])),
                    }
                } else {
                    alone.push(unit);
                }
            }
        }
    }
    let mut out = Vec::new();
    for (g, us) in &batches {
        let name = format!("batch-{g:?}-{}", slug(&us[0].tag())).to_ascii_lowercase();
        out.push(build(name, us));
    }
    for u in alone {
        out.push(build(slug(&u.tag()), &[u]));
    }
    out.extend(blocked_cases(filter));
    out
}

/// The 85 Ironclad cards that are registry rows.
pub const IRONCLAD: &[CardId] = &[
    CardId::Aggression,
    CardId::Anger,
    CardId::Armaments,
    CardId::AshenStrike,
    CardId::Barricade,
    CardId::Bash,
    CardId::BattleTrance,
    CardId::Bloodletting,
    CardId::BloodWall,
    CardId::Bludgeon,
    CardId::BodySlam,
    CardId::Brand,
    CardId::Break,
    CardId::Breakthrough,
    CardId::Bully,
    CardId::BurningPact,
    CardId::Cascade,
    CardId::Cinder,
    CardId::Colossus,
    CardId::Conflagration,
    CardId::Corruption,
    CardId::CrimsonMantle,
    CardId::Cruelty,
    CardId::DarkEmbrace,
    CardId::DefendIronclad,
    CardId::DemonForm,
    CardId::Dismantle,
    CardId::Dominate,
    CardId::DrumOfBattle,
    CardId::EvilEye,
    CardId::ExpectAFight,
    CardId::Feed,
    CardId::FeelNoPain,
    CardId::FiendFire,
    CardId::FightMe,
    CardId::FlameBarrier,
    CardId::ForgottenRitual,
    CardId::Havoc,
    CardId::Headbutt,
    CardId::Hellraiser,
    CardId::Hemokinesis,
    CardId::HowlFromBeyond,
    CardId::Impervious,
    CardId::InfernalBlade,
    CardId::Inferno,
    CardId::Inflame,
    CardId::IronWave,
    CardId::Juggernaut,
    CardId::Juggling,
    CardId::Mangle,
    CardId::MoltenFist,
    CardId::NotYet,
    CardId::Offering,
    CardId::OneTwoPunch,
    CardId::PactsEnd,
    CardId::PerfectedStrike,
    CardId::Pillage,
    CardId::PommelStrike,
    CardId::PrimalForce,
    CardId::Pyre,
    CardId::Rage,
    CardId::Rampage,
    CardId::Rupture,
    CardId::SecondWind,
    CardId::SetupStrike,
    CardId::ShrugItOff,
    CardId::Spite,
    CardId::Stampede,
    CardId::Stoke,
    CardId::Stomp,
    CardId::StoneArmor,
    CardId::StrikeIronclad,
    CardId::SwordBoomerang,
    CardId::Taunt,
    CardId::TearAsunder,
    CardId::Thrash,
    CardId::Thunderclap,
    CardId::Tremble,
    CardId::TrueGrit,
    CardId::TwinStrike,
    CardId::Unmovable,
    CardId::Unrelenting,
    CardId::Uppercut,
    CardId::Vicious,
    CardId::Whirlwind,
];
