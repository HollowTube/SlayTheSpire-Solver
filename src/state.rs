use crate::engine::{Actor, Status};
use crate::monsters::opening_intent;
use pyo3::prelude::*;
use rand::SeedableRng;
use rand_pcg::Pcg32;

#[derive(Clone, PartialEq)]
pub(crate) enum PendingDecision {
    SelectTarget { card: String },
}

impl PendingDecision {
    fn as_str(&self) -> &'static str {
        match self {
            PendingDecision::SelectTarget { .. } => "SelectTarget",
        }
    }
}

/// One combatant's symmetric battle state. HP, max HP, block, and statuses
/// were previously tracked as six separate `player_*`/`monster_*` field pairs
/// directly on `CombatState`; every piece of `Actor`-aware engine code touched
/// them in mirrored pairs anyway (that symmetry is the whole point of `Actor`/
/// `Side`), so this struct removes the duplication at its root rather than
/// just hiding it behind accessors. `CombatState::fighter`/`fighter_mut`
/// resolve an `Actor` to the right one.
#[derive(Clone, PartialEq)]
pub(crate) struct Fighter {
    pub(crate) hp: i32,
    // The HP fraction `evaluate`/`reward` shape against. Defaults to the
    // starting `hp` (combat begins at full) via `CombatState::new`'s
    // `player_max_hp`/`monster_max_hp` overrides — needed both because player
    // HP carries between fights (what an actual STS run optimizes for, the
    // common case once multi-fight runs exist) and because tests sometimes
    // construct already-terminal or partial-HP states directly, where
    // max == current == 0 would collapse the fraction to NaN.
    pub(crate) max_hp: i32,
    // Absorbs incoming damage before HP, reset at the start of each
    // combatant's own turn (e.g. Jaw Worm's Thrash/Bellow grant it block that
    // lasts through the player's turn; mirrors `EndTurn`'s player_block reset).
    pub(crate) block: i32,
    pub(crate) statuses: Vec<Status>,
}

#[pyclass(eq)]
#[derive(Clone, PartialEq)]
pub struct CombatState {
    pub(crate) player: Fighter,
    #[pyo3(get)]
    pub(crate) player_energy: i32,
    // What `EndTurn` refreshes `player_energy` back up to each turn — Slay
    // the Spire characters draw a fixed energy amount every turn rather than
    // carrying it over. Defaults to the starting `player_energy` (a fresh
    // combat begins at full); the override exists for the same reason as
    // `Fighter::max_hp` — relics/potions that raise max energy mid-run are a
    // real mechanic, and tests may want to start mid-turn with partially-spent
    // energy without that looking like the per-turn maximum.
    pub(crate) player_max_energy: i32,
    pub(crate) monster: Fighter,
    #[pyo3(get)]
    pub(crate) monster_attack: i32,
    // The monster's species — looked up against move-pool data to drive
    // intent-based AI (e.g. "Jaw Worm"). `None` (the default) keeps the
    // original trivial fixed-`monster_attack` behavior used by the
    // placeholder monster and every pre-HOL-11 test.
    #[pyo3(get)]
    pub(crate) monster_name: Option<String>,
    // The move the monster has telegraphed for its next turn (e.g. "Chomp")
    // — mirrors how Slay the Spire shows enemy intent before the player acts.
    // `None` for monsters with no move pool (the trivial flat-attacker).
    pub(crate) monster_intent: Option<String>,
    // The name of the most recently *executed* move, and how many turns in a
    // row it's now run — the minimal state `select_next_intent` needs to
    // enforce "cannot repeat X" / "cannot use Y N times in a row" constraints
    // without replaying full move history.
    pub(crate) monster_last_move: Option<String>,
    pub(crate) monster_move_streak: u32,
    #[pyo3(get)]
    pub(crate) turn: u32,
    #[pyo3(get)]
    pub(crate) hand: Vec<String>,
    pub(crate) pending: Option<PendingDecision>,
    pub(crate) rng: Pcg32,
}

#[pymethods]
impl CombatState {
    #[new]
    #[pyo3(signature = (player_hp, player_energy, monster_hp, monster_attack, seed, hand=Vec::new(), player_max_hp=None, monster_max_hp=None, player_max_energy=None, monster_name=None))]
    fn new(
        player_hp: i32,
        player_energy: i32,
        monster_hp: i32,
        monster_attack: i32,
        seed: u64,
        hand: Vec<String>,
        player_max_hp: Option<i32>,
        monster_max_hp: Option<i32>,
        player_max_energy: Option<i32>,
        monster_name: Option<String>,
    ) -> Self {
        let monster_intent = monster_name.as_deref().and_then(opening_intent);
        CombatState {
            player: Fighter {
                hp: player_hp,
                max_hp: player_max_hp.unwrap_or(player_hp),
                block: 0,
                statuses: Vec::new(),
            },
            player_energy,
            player_max_energy: player_max_energy.unwrap_or(player_energy),
            monster: Fighter {
                hp: monster_hp,
                max_hp: monster_max_hp.unwrap_or(monster_hp),
                block: 0,
                statuses: Vec::new(),
            },
            monster_attack,
            monster_name,
            monster_intent,
            monster_last_move: None,
            monster_move_streak: 0,
            turn: 0,
            hand,
            pending: None,
            rng: Pcg32::seed_from_u64(seed),
        }
    }

    // `Fighter`'s fields aren't `#[pyo3(get)]` themselves (the struct isn't a
    // pyclass), so each previously-direct `player_hp`/`monster_block`/etc.
    // getter becomes an explicit `#[getter]` delegating to `self.player`/
    // `self.monster` — preserving every Python-visible attribute name exactly.
    #[getter]
    fn player_hp(&self) -> i32 {
        self.player.hp
    }

    #[getter]
    fn player_block(&self) -> i32 {
        self.player.block
    }

    #[getter]
    fn monster_hp(&self) -> i32 {
        self.monster.hp
    }

    #[getter]
    fn monster_block(&self) -> i32 {
        self.monster.block
    }

    #[getter]
    fn pending(&self) -> Option<String> {
        self.pending.as_ref().map(|p| p.as_str().to_string())
    }

    #[getter]
    fn monster_intent(&self) -> Option<String> {
        self.monster_intent.clone()
    }

    #[getter]
    fn monster_statuses(&self) -> Vec<String> {
        self.monster.statuses.iter().map(|s| s.as_str().to_string()).collect()
    }

    #[getter]
    fn player_statuses(&self) -> Vec<String> {
        self.player.statuses.iter().map(|s| s.as_str().to_string()).collect()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}

impl CombatState {
    /// Resolves an `Actor` identity to its `Fighter` — the single chokepoint
    /// that lets the generic, `Actor`-aware engine (`run_effect_ops`,
    /// `deal_damage`, ...) reach either combatant's HP/block/statuses
    /// symmetrically, without ever branching on Player-vs-Monster itself.
    pub(crate) fn fighter(&self, who: Actor) -> &Fighter {
        match who {
            Actor::Player => &self.player,
            Actor::Monster => &self.monster,
        }
    }

    pub(crate) fn fighter_mut(&mut self, who: Actor) -> &mut Fighter {
        match who {
            Actor::Player => &mut self.player,
            Actor::Monster => &mut self.monster,
        }
    }
}
