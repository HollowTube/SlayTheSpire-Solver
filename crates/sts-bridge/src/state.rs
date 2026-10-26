//! Backend-neutral game state: what the live game reports, and what the sim
//! will produce for parity comparison.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
pub use sts_ids::{BridgeId, CardId, MonsterId, StatusId};

/// A game entity named by its bridge class name: a registry id, or the raw
/// name when the registry doesn't know it (a game patch, an unported card).
/// On the wire and in the schema it is a plain string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Entity<T> {
    Known(T),
    Unknown(String),
}

impl<T: BridgeId> Entity<T> {
    pub fn parse(name: String) -> Self {
        match T::from_bridge(&name) {
            Some(id) => Entity::Known(id),
            None => Entity::Unknown(name),
        }
    }

    /// The canonical bridge name for `Known`, the raw name for `Unknown`.
    pub fn name(&self) -> &str {
        match self {
            Entity::Known(id) => id.bridge_name(),
            Entity::Unknown(name) => name,
        }
    }
}

impl<T: BridgeId> Serialize for Entity<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de, T: BridgeId> Deserialize<'de> for Entity<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Entity::parse)
    }
}

impl<T> JsonSchema for Entity<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        String::schema_name()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        String::json_schema(generator)
    }
}

/// Everything a consumer can observe about the game at one instant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GameState {
    pub screen: Screen,
    /// `None` when no run is in progress (main menu, character select).
    pub run: Option<RunInfo>,
    /// `None` outside combat, including while a combat room is loading. Set on
    /// in-combat overlay screens (`HAND_SELECT`, `CARD_SELECTION`) too.
    pub combat: Option<CombatView>,
}

/// The game's screen name (e.g. `MAIN_MENU`, `COMBAT_PLAYER_TURN`). A string
/// rather than an enum so a screen added by a game patch is not a parse error.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Screen(pub String);

impl Screen {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A combat screen whose state can be read (not a loading transition).
    pub fn is_combat(&self) -> bool {
        self.0.starts_with("COMBAT") && !self.is_loading()
    }

    pub fn is_loading(&self) -> bool {
        self.0.contains("LOADING")
    }
}

impl std::fmt::Display for Screen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunInfo {
    pub act: u32,
    pub floor: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub gold: i32,
    pub character: Option<String>,
    pub ascension: u32,
    pub seed: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CombatView {
    pub round: u32,
    pub is_player_turn: bool,
    pub player: PlayerView,
    pub enemies: Vec<EnemyView>,
    pub piles: Piles,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerView {
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub energy: i32,
    pub max_energy: i32,
    pub powers: Vec<Power>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EnemyView {
    /// Position in the enemy list; the target index for card plays.
    pub index: usize,
    /// Unique per combat, e.g. `Nibbit_0`, `Nibbit_1`.
    pub entity_id: String,
    pub name: Entity<MonsterId>,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub is_alive: bool,
    pub powers: Vec<Power>,
    /// `None` when the enemy has no next move (e.g. dead).
    pub intent: Option<Intent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Intent {
    pub move_id: String,
    pub parts: Vec<IntentPart>,
}

/// One component of an enemy move. Damage numbers are carried exactly as the
/// game reports them; they are `None` for non-attack parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IntentPart {
    /// e.g. `Attack`, `Defend`, `Buff`, `Debuff`.
    pub kind: String,
    pub damage: Option<i32>,
    pub hits: Option<i32>,
    pub total_damage: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Power {
    /// Parsed from the game's power class name, e.g. `VulnerablePower`.
    pub id: Entity<StatusId>,
    pub amount: i32,
    /// `Buff`, `Debuff`, … when the backend reports it.
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Piles {
    pub hand: Vec<HandCard>,
    pub draw: Vec<CardRef>,
    pub discard: Vec<CardRef>,
    pub exhaust: Vec<CardRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HandCard {
    pub index: usize,
    /// Parsed from the game's card class name, e.g. `StrikeIronclad`.
    pub id: Entity<CardId>,
    pub upgraded: bool,
    pub cost: i32,
    /// `Attack`, `Skill`, `Power`, `Status`, `Curse`.
    pub kind: String,
    pub playable: bool,
    pub unplayable_reason: Option<String>,
    /// e.g. `AnyEnemy`, `Self`, `None`.
    pub target_type: String,
    /// Enemy (or ally) indices the card may target; `None` for untargeted or
    /// unplayable cards.
    pub valid_targets: Option<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CardRef {
    pub id: Entity<CardId>,
    pub upgraded: bool,
    pub cost: i32,
    pub kind: String,
}

/// One action the game currently accepts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AvailableAction {
    /// e.g. `play_card`, `end_turn`, `map_travel`, `event_option`.
    pub action: String,
    pub card_index: Option<usize>,
    pub target_index: Option<usize>,
    pub card_name: Option<String>,
    pub target_name: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    /// Action-specific parameters (e.g. `row`/`col` for `map_travel`).
    #[serde(flatten)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

/// The actions the game accepts on its current screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Actions {
    pub screen: Screen,
    /// The game's active overlay type, which disambiguates generic screens
    /// such as `CARD_SELECTION`.
    pub screen_context: Option<String>,
    pub actions: Vec<AvailableAction>,
}

/// Result of a ping round-trip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Health {
    pub ok: bool,
    pub latency_ms: f64,
    pub backend: String,
    pub version: Option<String>,
    pub screen: Option<Screen>,
    pub run_in_progress: Option<bool>,
    pub in_combat: Option<bool>,
}
