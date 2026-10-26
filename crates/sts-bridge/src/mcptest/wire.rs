//! MCPTest response shapes and their conversion into [`crate::state`] types.
//!
//! Fields consumers depend on are required: a missing one is a
//! [`BridgeError::Schema`], never a default. Fields the bridge omits when null
//! (it serializes with `WhenWritingNull`) are `Option`.

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::backend::BridgeError;
use crate::state::{
    AvailableAction, CardRef, CombatView, EnemyView, HandCard, Intent, IntentPart, Piles,
    PlayerView, Power, RunInfo, Screen,
};

pub(crate) fn parse<T: DeserializeOwned>(method: &str, value: Value) -> Result<T, BridgeError> {
    serde_path_to_error::deserialize(value).map_err(|e| BridgeError::Schema {
        method: method.to_string(),
        path: e.path().to_string(),
        message: e.into_inner().to_string(),
    })
}

fn first_player<P>(method: &str, players: Vec<P>) -> Result<P, BridgeError> {
    players
        .into_iter()
        .next()
        .ok_or_else(|| BridgeError::Schema {
            method: method.to_string(),
            path: "players".to_string(),
            message: "expected at least one player".to_string(),
        })
}

#[derive(Deserialize)]
pub(crate) struct WireActions {
    pub screen: String,
    pub screen_context_type: Option<String>,
    pub actions: Vec<AvailableAction>,
}

#[derive(Deserialize)]
pub(crate) struct WireRunFlag {
    pub in_progress: bool,
}

#[derive(Deserialize)]
pub(crate) struct WireRun {
    act: u32,
    floor: u32,
    ascension: u32,
    seed: String,
    players: Vec<WireRunPlayer>,
}

#[derive(Deserialize)]
struct WireRunPlayer {
    character: Option<String>,
    hp: i32,
    max_hp: i32,
    gold: i32,
}

impl WireRun {
    pub fn into_run_info(self, method: &str) -> Result<RunInfo, BridgeError> {
        let p = first_player(method, self.players)?;
        Ok(RunInfo {
            act: self.act,
            floor: self.floor,
            hp: p.hp,
            max_hp: p.max_hp,
            gold: p.gold,
            character: p.character,
            ascension: self.ascension,
            seed: self.seed,
        })
    }
}

#[derive(Deserialize)]
pub(crate) struct WireCombatFlag {
    pub in_combat: bool,
}

#[derive(Deserialize)]
pub(crate) struct WireCombat {
    round: u32,
    is_player_turn: bool,
    enemies: Vec<WireEnemy>,
    players: Vec<WireCombatPlayer>,
}

#[derive(Deserialize)]
struct WireEnemy {
    index: usize,
    entity_id: String,
    name: String,
    hp: i32,
    max_hp: i32,
    block: i32,
    is_alive: bool,
    intent: Option<WireIntent>,
    powers: Vec<WirePower>,
}

#[derive(Deserialize)]
struct WireIntent {
    move_id: String,
    intents: Vec<WireIntentPart>,
}

#[derive(Deserialize)]
struct WireIntentPart {
    #[serde(rename = "type")]
    kind: String,
    damage: Option<i32>,
    hits: Option<i32>,
    total_damage: Option<i32>,
}

#[derive(Deserialize)]
struct WirePower {
    name: String,
    amount: i32,
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct WireCombatPlayer {
    hp: i32,
    max_hp: i32,
    block: i32,
    energy: i32,
    max_energy: i32,
    hand: Vec<WireHandCard>,
    powers: Vec<WirePower>,
}

#[derive(Deserialize)]
struct WireHandCard {
    index: usize,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    energy_cost: i32,
    can_play: bool,
    unplayable_reason: Option<String>,
    target_type: String,
    valid_targets: Option<Vec<usize>>,
    upgraded: bool,
}

#[derive(Deserialize)]
pub(crate) struct WireCardPiles {
    draw_pile: WirePile,
    discard_pile: WirePile,
    exhaust_pile: WirePile,
}

#[derive(Deserialize)]
struct WirePile {
    cards: Vec<WireCard>,
}

#[derive(Deserialize)]
struct WireCard {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    energy_cost: i32,
    upgraded: bool,
}

fn powers(raw: Vec<WirePower>) -> Vec<Power> {
    raw.into_iter()
        .map(|p| Power {
            id: p.name,
            amount: p.amount,
            kind: p.kind,
        })
        .collect()
}

fn cards(pile: WirePile) -> Vec<CardRef> {
    pile.cards
        .into_iter()
        .map(|c| CardRef {
            id: c.name,
            upgraded: c.upgraded,
            cost: c.energy_cost,
            kind: c.kind,
        })
        .collect()
}

impl WireCombat {
    pub fn into_combat_view(
        self,
        method: &str,
        piles: WireCardPiles,
    ) -> Result<CombatView, BridgeError> {
        let p = first_player(method, self.players)?;
        let hand = p
            .hand
            .into_iter()
            .map(|c| HandCard {
                index: c.index,
                id: c.name,
                upgraded: c.upgraded,
                cost: c.energy_cost,
                kind: c.kind,
                playable: c.can_play,
                unplayable_reason: c.unplayable_reason.filter(|r| !r.is_empty()),
                target_type: c.target_type,
                valid_targets: c.valid_targets,
            })
            .collect();
        let enemies = self
            .enemies
            .into_iter()
            .map(|e| EnemyView {
                index: e.index,
                entity_id: e.entity_id,
                name: e.name,
                hp: e.hp,
                max_hp: e.max_hp,
                block: e.block,
                is_alive: e.is_alive,
                powers: powers(e.powers),
                intent: e.intent.map(|i| Intent {
                    move_id: i.move_id,
                    parts: i
                        .intents
                        .into_iter()
                        .map(|part| IntentPart {
                            kind: part.kind,
                            damage: part.damage,
                            hits: part.hits,
                            total_damage: part.total_damage,
                        })
                        .collect(),
                }),
            })
            .collect();
        Ok(CombatView {
            round: self.round,
            is_player_turn: self.is_player_turn,
            player: PlayerView {
                hp: p.hp,
                max_hp: p.max_hp,
                block: p.block,
                energy: p.energy,
                max_energy: p.max_energy,
                powers: powers(p.powers),
            },
            enemies,
            piles: Piles {
                hand,
                draw: cards(piles.draw_pile),
                discard: cards(piles.discard_pile),
                exhaust: cards(piles.exhaust_pile),
            },
        })
    }
}

impl WireActions {
    pub fn screen(&self) -> Screen {
        Screen(self.screen.clone())
    }
}
