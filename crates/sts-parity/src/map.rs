//! Game-to-sim state mapping and the one comparable state both sides are
//! projected into.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sts_bridge::{
    BridgeId, CardId, CardRef, CombatView, Entity, GameState, MonsterId, Power, StatusId,
};
use sts_sim::state::CardInstance;
use sts_sim::{CombatParts, Counters, FighterParts, MonsterCounters, MonsterParts};

/// Fixed so a step's fresh RNG does not depend on the case.
pub const SEED: u64 = 0x5EED;

/// Why a state or a step could not be run through the sim. Never a skip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimFailure {
    /// The game state has something the sim cannot be given (an unmapped
    /// power, card or monster, no combat).
    Mapping(String),
    /// The sim refused the state or the action.
    Sim(String),
    /// The sim panicked.
    Panic(String),
    /// The sim wants a choice the harness cannot make.
    PendingChoice(String),
}

impl std::fmt::Display for SimFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimFailure::Mapping(m) => write!(f, "mapping: {m}"),
            SimFailure::Sim(m) => write!(f, "sim error: {m}"),
            SimFailure::Panic(m) => write!(f, "sim panic: {m}"),
            SimFailure::PendingChoice(m) => write!(f, "unresolved choice: {m}"),
        }
    }
}

/// The hidden sim state a game observation cannot show; a serde mirror of
/// `sts_sim::Counters`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Carry {
    pub cards_played_this_turn: i32,
    pub attacks_played_this_turn: i32,
    pub times_damaged_this_combat: i32,
    pub hp_lost_this_turn: bool,
    pub exhausted_this_turn: bool,
    pub blocks_gained_this_turn: i32,
    pub last_attacker: Option<usize>,
    pub combat_counters: BTreeMap<String, i32>,
    pub monsters: Vec<MonsterCarry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MonsterCarry {
    pub last_move: Option<String>,
    pub move_streak: u32,
    pub moves_used: Vec<String>,
}

impl Carry {
    /// A fresh combat with `monsters` enemies.
    pub fn fresh(monsters: usize) -> Self {
        Self {
            monsters: vec![MonsterCarry::default(); monsters],
            ..Self::default()
        }
    }
}

impl From<Carry> for Counters {
    fn from(c: Carry) -> Self {
        Counters {
            cards_played_this_turn: c.cards_played_this_turn,
            attacks_played_this_turn: c.attacks_played_this_turn,
            times_damaged_this_combat: c.times_damaged_this_combat,
            hp_lost_this_turn: c.hp_lost_this_turn,
            exhausted_this_turn: c.exhausted_this_turn,
            blocks_gained_this_turn: c.blocks_gained_this_turn,
            last_attacker: c.last_attacker,
            combat_counters: c.combat_counters,
            monsters: c
                .monsters
                .into_iter()
                .map(|m| MonsterCounters {
                    last_move: m.last_move,
                    move_streak: m.move_streak,
                    moves_used: m.moves_used,
                })
                .collect(),
        }
    }
}

impl From<Counters> for Carry {
    fn from(c: Counters) -> Self {
        Carry {
            cards_played_this_turn: c.cards_played_this_turn,
            attacks_played_this_turn: c.attacks_played_this_turn,
            times_damaged_this_combat: c.times_damaged_this_combat,
            hp_lost_this_turn: c.hp_lost_this_turn,
            exhausted_this_turn: c.exhausted_this_turn,
            blocks_gained_this_turn: c.blocks_gained_this_turn,
            last_attacker: c.last_attacker,
            combat_counters: c.combat_counters,
            monsters: c
                .monsters
                .into_iter()
                .map(|m| MonsterCarry {
                    last_move: m.last_move,
                    move_streak: m.move_streak,
                    moves_used: m.moves_used,
                })
                .collect(),
        }
    }
}

/// What is compared. Powers are keyed by the status's bridge name; cards are
/// `Name` or `Name+`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityState {
    pub player: PlayerState,
    pub enemies: Vec<EnemyState>,
    pub round: u32,
    /// Sorted: the hand has no order the sim and game must share.
    pub hand: Vec<String>,
    /// Top first, in order.
    pub draw: Vec<String>,
    /// Sorted.
    pub discard: Vec<String>,
    /// Sorted.
    pub exhaust: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub energy: i32,
    pub powers: BTreeMap<String, i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyState {
    pub index: usize,
    pub monster: String,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub alive: bool,
    pub powers: BTreeMap<String, i32>,
}

pub fn card_name(id: &Entity<CardId>, upgraded: bool) -> String {
    format!("{}{}", id.name(), if upgraded { "+" } else { "" })
}

fn sim_card_name(card: &CardInstance) -> String {
    format!(
        "{}{}",
        card.id.bridge_name(),
        "+".repeat(card.upgrade_level as usize)
    )
}

fn power_map<'a>(powers: impl Iterator<Item = (String, i32)> + 'a) -> BTreeMap<String, i32> {
    let mut out = BTreeMap::new();
    for (name, amount) in powers {
        *out.entry(name).or_insert(0) += amount;
    }
    out.retain(|_, amount| *amount != 0);
    out
}

fn sorted(mut cards: Vec<String>) -> Vec<String> {
    cards.sort();
    cards
}

impl ParityState {
    /// The game's state; an unknown class stays as its raw name, so a
    /// difference shows as a diff. Fails only outside combat.
    pub fn from_game(state: &GameState) -> Result<Self, SimFailure> {
        let c = state
            .combat
            .as_ref()
            .ok_or_else(|| SimFailure::Mapping(format!("no combat on {}", state.screen)))?;
        let powers =
            |ps: &[Power]| power_map(ps.iter().map(|p| (p.id.name().to_string(), p.amount)));
        let refs = |cards: &[CardRef]| cards.iter().map(|c| card_name(&c.id, c.upgraded)).collect();
        Ok(ParityState {
            player: PlayerState {
                hp: c.player.hp,
                max_hp: c.player.max_hp,
                block: c.player.block,
                energy: c.player.energy,
                powers: powers(&c.player.powers),
            },
            // The game drops a dead enemy from its list, the sim keeps it.
            enemies: c
                .enemies
                .iter()
                .filter(|e| e.is_alive)
                .map(|e| EnemyState {
                    index: e.index,
                    monster: e.name.name().to_string(),
                    hp: e.hp,
                    max_hp: e.max_hp,
                    block: e.block,
                    alive: e.is_alive,
                    powers: powers(&e.powers),
                })
                .collect(),
            round: c.round,
            hand: sorted(
                c.piles
                    .hand
                    .iter()
                    .map(|h| card_name(&h.id, h.upgraded))
                    .collect(),
            ),
            draw: refs(&c.piles.draw),
            discard: sorted(refs(&c.piles.discard)),
            exhaust: sorted(refs(&c.piles.exhaust)),
        })
    }

    /// The sim's state, in the game's terms.
    pub fn from_sim(p: &CombatParts) -> Self {
        let status = |name: &str| match StatusId::from_str(name) {
            Some(id) => id.bridge_name().to_string(),
            None => name.to_string(),
        };
        let powers = |ps: &[(String, i32)]| {
            let mut map = power_map(ps.iter().map(|(n, a)| (status(n), *a * unit(n))));
            // The game applies a temporary Strength as a matching Strength.
            let temporary = map.get(TEMPORARY_STRENGTH).copied().unwrap_or(0);
            if temporary != 0 {
                *map.entry("Strength".to_string()).or_insert(0) += temporary;
                map.retain(|_, a| *a != 0);
            }
            map
        };
        let cards = |cs: &[CardInstance]| cs.iter().map(sim_card_name).collect::<Vec<_>>();
        ParityState {
            player: PlayerState {
                hp: p.player.hp,
                max_hp: p.player.max_hp,
                block: p.player.block,
                energy: p.energy,
                powers: powers(&p.player.statuses),
            },
            enemies: p
                .monsters
                .iter()
                .enumerate()
                .filter(|(_, m)| m.hp > 0)
                .map(|(i, m)| EnemyState {
                    index: i,
                    monster: m
                        .id
                        .map_or_else(|| "none".to_string(), |id| id.bridge_name().to_string()),
                    hp: m.hp,
                    max_hp: m.max_hp,
                    block: m.block,
                    alive: m.hp > 0,
                    powers: powers(&m.statuses),
                })
                .collect(),
            round: p.turn + 1,
            hand: sorted(cards(&p.hand)),
            draw: cards(&p.draw_pile),
            discard: sorted(cards(&p.discard_pile)),
            // The sim parks a played Power in the exhaust pile; the game removes
            // it from combat.
            exhaust: sorted(cards(
                &p.exhaust_pile
                    .iter()
                    .filter(|c| {
                        !matches!(
                            sts_sim::cards::card_data(c.id, c.upgrade_level).card_type,
                            sts_sim::cards::CardType::Power
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
            )),
        }
    }
}

/// The game amount one sim copy of a binary status stands for. A sim copy
/// already applies the full effect, so the game's amount is copies x unit.
const UNITS: &[(&str, i32)] = &[
    ("Rage", 3),
    ("DemonForm", 2),
    ("FeelNoPain", 3),
    ("FlameBarrier", 4),
    ("Inferno", 6),
    ("Cruelty", 25),
];

/// The status the game reports as `SetupStrikePower`: a Strength it applied
/// beside this power.
const TEMPORARY_STRENGTH: &str = "SetupStrikePower";

fn unit(sim_name: &str) -> i32 {
    UNITS
        .iter()
        .find(|(n, _)| *n == sim_name)
        .map_or(1, |(_, u)| *u)
}

/// An amount that is not a whole number of copies is rounded down, so the
/// remainder shows as a diff against the game's amount.
fn statuses(powers: &[Power]) -> Result<Vec<(String, i32)>, SimFailure> {
    let mut sums: BTreeMap<String, i32> = BTreeMap::new();
    for p in powers {
        match &p.id {
            Entity::Known(id) => *sums.entry(id.as_str().to_string()).or_insert(0) += p.amount,
            Entity::Unknown(class) => {
                return Err(SimFailure::Mapping(format!("unmapped power `{class}`")))
            }
        }
    }
    if let Some(&n) = sums.get("StrengthThisTurn") {
        *sums.entry("Strength".to_string()).or_insert(0) -= n;
    }
    Ok(sums
        .into_iter()
        .filter_map(|(name, amount)| {
            let amount = if unit(&name) > 1 {
                amount / unit(&name)
            } else {
                amount
            };
            (amount != 0).then_some((name, amount))
        })
        .collect())
}

fn card(id: &Entity<CardId>, upgraded: bool) -> Result<CardInstance, SimFailure> {
    match id {
        Entity::Known(id) => Ok(CardInstance {
            id: *id,
            upgrade_level: u8::from(upgraded),
        }),
        Entity::Unknown(class) => Err(SimFailure::Mapping(format!("unknown card `{class}`"))),
    }
}

fn cards(piles: &[CardRef]) -> Result<Vec<CardInstance>, SimFailure> {
    piles.iter().map(|c| card(&c.id, c.upgraded)).collect()
}

/// The sim's input for the game's current combat plus the carried counters.
/// The draw pile is handed over top first, as the bridge lists it.
pub fn to_parts(combat: &CombatView, carry: &Carry) -> Result<CombatParts, SimFailure> {
    let mut monsters = Vec::new();
    for e in &combat.enemies {
        let Entity::Known(id): &Entity<MonsterId> = &e.name else {
            return Err(SimFailure::Mapping(format!(
                "unknown monster `{}`",
                e.name.name()
            )));
        };
        monsters.push(MonsterParts {
            id: Some(*id),
            attack: 0,
            hp: e.hp,
            max_hp: e.max_hp,
            block: e.block,
            statuses: statuses(&e.powers)?,
            intent: if e.is_alive {
                e.intent.as_ref().map(|i| i.move_id.clone())
            } else {
                None
            },
        });
    }
    let mut counters: Counters = carry.clone().into();
    counters
        .monsters
        .resize(monsters.len(), MonsterCounters::default());
    let hand = combat
        .piles
        .hand
        .iter()
        .map(|h| card(&h.id, h.upgraded))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CombatParts {
        player: FighterParts {
            hp: combat.player.hp,
            max_hp: combat.player.max_hp,
            block: combat.player.block,
            statuses: statuses(&combat.player.powers)?,
        },
        energy: combat.player.energy,
        max_energy: combat.player.max_energy,
        monsters,
        hand,
        draw_pile: cards(&combat.piles.draw)?,
        discard_pile: cards(&combat.piles.discard)?,
        exhaust_pile: cards(&combat.piles.exhaust)?,
        turn: combat.round.saturating_sub(1),
        seed: SEED,
        counters,
    })
}
