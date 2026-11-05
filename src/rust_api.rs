//! The sim's Rust-facing API: typed actions, a strict state constructor and
//! exporter (`CombatParts`), and read accessors. Everything here is plain
//! data in and out. The card, status, and monster tables behind it
//! (`cards`, `engine`, `monsters`) are public as read-only data.
use crate::engine::Status;
use crate::error::SimError;
use crate::ids::{CardId, MonsterId};
use crate::monsters::{monster_move, move_from_sts2};
use crate::state::{CardInstance, CombatState, Fighter, Monster, PendingDecision};
use rand::SeedableRng;
use rand_pcg::Pcg32;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// One decision the player can take. `Display` and `FromStr` use the string
/// form the search layer and the Python bindings exchange
/// (`PlayCard:Strike+`, `SelectTarget:Monster:0`, ...).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimAction {
    EndTurn,
    PlayCard { card: CardId, upgrades: u8 },
    SelectTarget(usize),
    UpgradeFromHand { card: CardId, upgrades: u8 },
    ExhaustFromHand { card: CardId, upgrades: u8 },
}

fn card_name(card: CardId, upgrades: u8) -> String {
    format!("{}{}", card.as_str(), "+".repeat(upgrades as usize))
}

impl fmt::Display for SimAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimAction::EndTurn => write!(f, "EndTurn"),
            SimAction::PlayCard { card, upgrades } => {
                write!(f, "PlayCard:{}", card_name(*card, *upgrades))
            }
            SimAction::SelectTarget(i) => write!(f, "SelectTarget:Monster:{i}"),
            SimAction::UpgradeFromHand { card, upgrades } => {
                write!(f, "UpgradeCardFromHand:{}", card_name(*card, *upgrades))
            }
            SimAction::ExhaustFromHand { card, upgrades } => {
                write!(f, "ExhaustCardFromHand:{}", card_name(*card, *upgrades))
            }
        }
    }
}

impl FromStr for SimAction {
    type Err = SimError;

    fn from_str(action: &str) -> Result<Self, SimError> {
        let unknown = || SimError::UnknownAction { action: action.to_string() };
        let card = |name: &str| {
            CardInstance::try_parse(name)
                .ok_or_else(|| SimError::UnknownCard { card: name.trim_end_matches('+').to_string() })
        };
        if action == "EndTurn" {
            Ok(SimAction::EndTurn)
        } else if let Some(name) = action.strip_prefix("PlayCard:") {
            let c = card(name)?;
            Ok(SimAction::PlayCard { card: c.id, upgrades: c.upgrade_level })
        } else if let Some(idx) = action.strip_prefix("SelectTarget:Monster:") {
            idx.parse().map(SimAction::SelectTarget).map_err(|_| unknown())
        } else if let Some(name) = action.strip_prefix("UpgradeCardFromHand:") {
            let c = card(name)?;
            Ok(SimAction::UpgradeFromHand { card: c.id, upgrades: c.upgrade_level })
        } else if let Some(name) = action.strip_prefix("ExhaustCardFromHand:") {
            let c = card(name)?;
            Ok(SimAction::ExhaustFromHand { card: c.id, upgrades: c.upgrade_level })
        } else {
            Err(unknown())
        }
    }
}

/// A combatant's visible state. Statuses are `(name, amount)` pairs as
/// `Status::as_str` / `Status::amount` give them: binary statuses stack as one
/// `(name, 1)` entry each, amount-bearing ones carry their amount, `Infested`
/// its count. `Minion` and `Infested` rebuild with the engine's only values
/// (leader Kin Priest; Wriggler, 21 HP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FighterParts {
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub statuses: Vec<(String, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonsterParts {
    /// `None` is the placeholder flat-attacker that swings for `attack`.
    pub id: Option<MonsterId>,
    pub attack: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub statuses: Vec<(String, i32)>,
    /// A move name or a raw STS2 move id. `None` means no telegraphed move
    /// (stunned or fled); the engine rolls one at the next `EndTurn`.
    pub intent: Option<String>,
}

/// Per-monster history the game does not show.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MonsterCounters {
    pub last_move: Option<String>,
    pub move_streak: u32,
    pub moves_used: Vec<String>,
}

/// The hidden state a game observation cannot show. `Default` is a fresh combat.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Counters {
    pub cards_played_this_turn: i32,
    pub attacks_played_this_turn: i32,
    pub times_damaged_this_combat: i32,
    pub hp_lost_this_turn: bool,
    pub exhausted_this_turn: bool,
    pub blocks_gained_this_turn: i32,
    pub last_attacker: Option<usize>,
    pub combat_counters: BTreeMap<String, i32>,
    /// Index-aligned with `CombatParts::monsters`.
    pub monsters: Vec<MonsterCounters>,
}

/// A combat as plain data. `parts()` leaves out the transient `pending`
/// decision, X-cost energy and the card being resolved, and the live RNG
/// position: `from_parts` seeds a fresh RNG from `seed`. So
/// `from_parts(s.parts()).parts() == s.parts()`, but state equality holds only
/// for pending-free states whose RNG was just seeded (`reseeded(seed)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatParts {
    pub player: FighterParts,
    pub energy: i32,
    pub max_energy: i32,
    pub monsters: Vec<MonsterParts>,
    pub hand: Vec<CardInstance>,
    /// Top of the pile first. The sim draws from the end, so `from_parts` and
    /// `parts` reverse.
    pub draw_pile: Vec<CardInstance>,
    pub discard_pile: Vec<CardInstance>,
    pub exhaust_pile: Vec<CardInstance>,
    pub turn: u32,
    pub seed: u64,
    pub counters: Counters,
}

/// Combat counter keys the engine reads (`DealDamageWithCombatBonus`).
const COUNTER_KEYS: &[&str] = &["Rampage"];

fn fighter_statuses(parts: &[(String, i32)]) -> Result<Vec<Status>, SimError> {
    let mut out = Vec::new();
    for (name, amount) in parts {
        let probe = Status::from_name_and_amount(name, 1);
        if probe.is_empty() {
            return Err(SimError::UnknownStatus { name: name.clone() });
        }
        if *amount <= 0 && probe.iter().all(|s| s.amount().is_none()) {
            return Err(bad(format!("status {name} needs a positive amount, got {amount}")));
        }
        out.extend(Status::from_name_and_amount(name, *amount));
    }
    Ok(out)
}

fn status_parts(statuses: &[Status]) -> Vec<(String, i32)> {
    statuses
        .iter()
        .map(|s| {
            let amount = match s {
                Status::Infested { count, .. } => *count,
                _ => s.amount().unwrap_or(1),
            };
            (s.as_str().to_string(), amount)
        })
        .collect()
}

fn bad(reason: impl Into<String>) -> SimError {
    SimError::BadParts { reason: reason.into() }
}

impl CombatState {
    /// Strict constructor. Unlike `CombatState::new`, an unknown status,
    /// counter key or intent is an error rather than silently dropped.
    pub fn from_parts(p: CombatParts) -> Result<CombatState, SimError> {
        if p.counters.monsters.len() != p.monsters.len() {
            return Err(bad(format!(
                "{} monster counters for {} monsters",
                p.counters.monsters.len(),
                p.monsters.len()
            )));
        }
        if let Some(i) = p.counters.last_attacker {
            if i >= p.monsters.len() {
                return Err(bad(format!("last_attacker {i} is out of range")));
            }
        }
        let mut combat_counters = BTreeMap::new();
        for (key, value) in &p.counters.combat_counters {
            let known = COUNTER_KEYS
                .iter()
                .find(|k| **k == key.as_str())
                .ok_or_else(|| bad(format!("unknown combat counter: {key}")))?;
            combat_counters.insert(*known, *value);
        }
        let mut monsters = Vec::with_capacity(p.monsters.len());
        for (m, c) in p.monsters.iter().zip(&p.counters.monsters) {
            let intent = match (&m.intent, m.id) {
                (Some(raw), Some(id)) => {
                    let name = move_from_sts2(id, raw).map(str::to_string).unwrap_or_else(|| raw.clone());
                    if monster_move(id, &name).is_none() {
                        return Err(SimError::UnknownMove {
                            monster: id.as_str().to_string(),
                            intent: raw.clone(),
                        });
                    }
                    Some(name)
                }
                (Some(raw), None) => {
                    return Err(SimError::UnknownMove { monster: String::new(), intent: raw.clone() })
                }
                // The engine rolls an intent at the next `EndTurn` when a
                // monster has none (stunned, fled), so `None` stays `None`.
                (None, _) => None,
            };
            monsters.push(Monster {
                fighter: Fighter {
                    hp: m.hp,
                    max_hp: m.max_hp,
                    block: m.block,
                    statuses: fighter_statuses(&m.statuses)?,
                },
                attack: m.attack,
                name: m.id,
                intent,
                last_move: c.last_move.clone(),
                move_streak: c.move_streak,
                moves_used: c.moves_used.clone(),
            });
        }
        let reversed = |pile: &[CardInstance]| pile.iter().rev().cloned().collect::<Vec<_>>();
        Ok(CombatState {
            player: Fighter {
                hp: p.player.hp,
                max_hp: p.player.max_hp,
                block: p.player.block,
                statuses: fighter_statuses(&p.player.statuses)?,
            },
            player_energy: p.energy,
            player_energy_x: 0,
            player_max_energy: p.max_energy,
            monsters,
            turn: p.turn,
            hand: p.hand,
            draw_pile: reversed(&p.draw_pile),
            discard_pile: p.discard_pile,
            exhaust_pile: p.exhaust_pile,
            cards_played_this_turn: p.counters.cards_played_this_turn,
            attacks_played_this_turn: p.counters.attacks_played_this_turn,
            player_times_damaged_this_combat: p.counters.times_damaged_this_combat,
            player_hp_lost_this_turn: p.counters.hp_lost_this_turn,
            player_exhausted_card_this_turn: p.counters.exhausted_this_turn,
            blocks_gained_this_turn: p.counters.blocks_gained_this_turn,
            last_attacker: p.counters.last_attacker,
            pending: None,
            rng: Pcg32::seed_from_u64(p.seed),
            seed: p.seed,
            resolving_card: None,
            combat_counters,
        })
    }

    /// The state as plain data; see `CombatParts` for what it leaves out.
    pub fn parts(&self) -> CombatParts {
        CombatParts {
            player: FighterParts {
                hp: self.player.hp,
                max_hp: self.player.max_hp,
                block: self.player.block,
                statuses: status_parts(&self.player.statuses),
            },
            energy: self.player_energy,
            max_energy: self.player_max_energy,
            monsters: self
                .monsters
                .iter()
                .map(|m| MonsterParts {
                    id: m.name,
                    attack: m.attack,
                    hp: m.fighter.hp,
                    max_hp: m.fighter.max_hp,
                    block: m.fighter.block,
                    statuses: status_parts(&m.fighter.statuses),
                    intent: m.intent.clone(),
                })
                .collect(),
            hand: self.hand.clone(),
            draw_pile: self.draw_pile.iter().rev().cloned().collect(),
            discard_pile: self.discard_pile.clone(),
            exhaust_pile: self.exhaust_pile.clone(),
            turn: self.turn,
            seed: self.seed,
            counters: Counters {
                cards_played_this_turn: self.cards_played_this_turn,
                attacks_played_this_turn: self.attacks_played_this_turn,
                times_damaged_this_combat: self.player_times_damaged_this_combat,
                hp_lost_this_turn: self.player_hp_lost_this_turn,
                exhausted_this_turn: self.player_exhausted_card_this_turn,
                blocks_gained_this_turn: self.blocks_gained_this_turn,
                last_attacker: self.last_attacker,
                combat_counters: self
                    .combat_counters
                    .iter()
                    .map(|(k, v)| (k.to_string(), *v))
                    .collect(),
                monsters: self
                    .monsters
                    .iter()
                    .map(|m| MonsterCounters {
                        last_move: m.last_move.clone(),
                        move_streak: m.move_streak,
                        moves_used: m.moves_used.clone(),
                    })
                    .collect(),
            },
        }
    }

    pub fn apply(&self, action: &SimAction) -> Result<CombatState, SimError> {
        crate::apply(self, action)
    }

    /// Applies `action` in place. A failed action leaves the state unchanged.
    pub fn step(&mut self, action: &SimAction) -> Result<(), SimError> {
        *self = crate::apply(self, action)?;
        Ok(())
    }

    pub fn legal_actions(&self) -> Vec<SimAction> {
        crate::legal_actions(self)
    }

    pub fn player(&self) -> &Fighter {
        &self.player
    }

    pub fn monsters(&self) -> &[Monster] {
        &self.monsters
    }

    pub fn energy(&self) -> i32 {
        self.player_energy
    }

    pub fn max_energy(&self) -> i32 {
        self.player_max_energy
    }

    pub fn turn(&self) -> u32 {
        self.turn
    }

    pub fn hand(&self) -> &[CardInstance] {
        &self.hand
    }

    /// The sim draws from the END of this slice.
    pub fn draw_pile(&self) -> &[CardInstance] {
        &self.draw_pile
    }

    pub fn discard_pile(&self) -> &[CardInstance] {
        &self.discard_pile
    }

    pub fn exhaust_pile(&self) -> &[CardInstance] {
        &self.exhaust_pile
    }

    pub fn pending(&self) -> Option<&PendingDecision> {
        self.pending.as_ref()
    }
}
