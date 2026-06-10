use crate::engine::{Actor, Status};
use crate::monsters::opening_intent;
use pyo3::prelude::*;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_pcg::Pcg32;

/// Per the Slay the Spire wiki, the Ironclad opens combat with — and redraws
/// up to — a 5-card hand each turn. Hard-coded for now since the Ironclad is
/// the only character M1 models; a per-character value once others exist.
pub(crate) const HAND_SIZE: usize = 5;

/// Draws up to `count` cards from `state.draw_pile` into `state.hand`,
/// reshuffling `state.discard_pile` back into the draw pile (via the embedded
/// PRNG, so this stays replayable) whenever the draw pile runs dry mid-draw —
/// mirroring documented Slay the Spire behavior exactly. Stops early if both
/// piles are exhausted, since there's nothing left to draw.
pub(crate) fn draw_cards(state: &mut CombatState, count: usize) {
    for _ in 0..count {
        if state.draw_pile.is_empty() {
            if state.discard_pile.is_empty() {
                return;
            }
            state.draw_pile.append(&mut state.discard_pile);
            state.draw_pile.shuffle(&mut state.rng);
        }
        if let Some(card) = state.draw_pile.pop() {
            state.hand.push(card);
        }
    }
}

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
    // `player_max_hp`/`Monster::new`'s `max_hp` overrides — needed both
    // because player HP carries between fights (what an actual STS run
    // optimizes for, the common case once multi-fight runs exist) and
    // because tests sometimes construct already-terminal or partial-HP
    // states directly, where max == current == 0 would collapse the
    // fraction to NaN.
    pub(crate) max_hp: i32,
    // Absorbs incoming damage before HP, reset at the start of each
    // combatant's own turn (e.g. Jaw Worm's Thrash/Bellow grant it block that
    // lasts through the player's turn; mirrors `EndTurn`'s player_block reset).
    pub(crate) block: i32,
    pub(crate) statuses: Vec<Status>,
}

/// One enemy combatant. `CombatState` holds a `Vec<Monster>` so a fight can
/// have any number of enemies; `Actor::Monster(i)` indexes into it. Bundles
/// the symmetric `Fighter` data with the monster-only singletons that used to
/// live directly on `CombatState` as `monster_attack`/`monster_name`/
/// `monster_intent`/`monster_last_move`/`monster_move_streak` — each monster
/// now carries its own copy of these, since each enemy in a fight telegraphs
/// and tracks its own intent independently.
#[pyclass(eq)]
#[derive(Clone, PartialEq)]
pub struct Monster {
    pub(crate) fighter: Fighter,
    // The trivial flat-attacker's fixed swing damage — meaningless (left at
    // 0) for intent-driven monsters (e.g. Jaw Worm), whose move pool decides
    // what they do each turn.
    #[pyo3(get)]
    pub(crate) attack: i32,
    // The monster's species — looked up against move-pool data to drive
    // intent-based AI (e.g. "Jaw Worm"). `None` (the default) keeps the
    // original trivial fixed-`attack` behavior used by the placeholder
    // monster and every pre-HOL-11 test.
    #[pyo3(get)]
    pub(crate) name: Option<String>,
    // The move this monster has telegraphed for its next turn (e.g. "Chomp")
    // — mirrors how Slay the Spire shows enemy intent before the player acts.
    // `None` for monsters with no move pool (the trivial flat-attacker).
    pub(crate) intent: Option<String>,
    // The name of the most recently *executed* move, and how many turns in a
    // row it's now run — the minimal state `select_next_intent` needs to
    // enforce "cannot repeat X" / "cannot use Y N times in a row" constraints
    // without replaying full move history.
    pub(crate) last_move: Option<String>,
    pub(crate) move_streak: u32,
}

#[pymethods]
impl Monster {
    #[new]
    #[pyo3(signature = (hp, attack=0, max_hp=None, name=None))]
    fn new(hp: i32, attack: i32, max_hp: Option<i32>, name: Option<String>) -> Self {
        let intent = name.as_deref().and_then(opening_intent);
        Monster {
            fighter: Fighter {
                hp,
                max_hp: max_hp.unwrap_or(hp),
                block: 0,
                statuses: Vec::new(),
            },
            attack,
            name,
            intent,
            last_move: None,
            move_streak: 0,
        }
    }

    #[getter]
    fn hp(&self) -> i32 {
        self.fighter.hp
    }

    #[getter]
    fn max_hp(&self) -> i32 {
        self.fighter.max_hp
    }

    #[getter]
    fn block(&self) -> i32 {
        self.fighter.block
    }

    #[getter]
    fn statuses(&self) -> Vec<String> {
        self.fighter.statuses.iter().map(|s| s.as_str().to_string()).collect()
    }

    #[getter]
    fn strength(&self) -> i32 {
        self.fighter.statuses.iter().filter_map(|s| match s {
            Status::Strength(n) => Some(*n),
            _ => None,
        }).sum()
    }

    #[getter]
    fn intent(&self) -> Option<String> {
        self.intent.clone()
    }

    #[getter]
    fn is_alive(&self) -> bool {
        self.fighter.hp > 0
    }
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
    #[pyo3(get)]
    pub(crate) monsters: Vec<Monster>,
    #[pyo3(get)]
    pub(crate) turn: u32,
    #[pyo3(get)]
    pub(crate) hand: Vec<String>,
    #[pyo3(get)]
    pub(crate) draw_pile: Vec<String>,
    #[pyo3(get)]
    pub(crate) discard_pile: Vec<String>,
    #[pyo3(get)]
    pub(crate) exhaust_pile: Vec<String>,
    pub(crate) pending: Option<PendingDecision>,
    pub(crate) rng: Pcg32,
}

#[pymethods]
impl CombatState {
    #[new]
    #[pyo3(signature = (player_hp, player_energy, monsters, seed, hand=Vec::new(), deck=None, player_max_hp=None, player_max_energy=None))]
    fn new(
        player_hp: i32,
        player_energy: i32,
        monsters: Vec<Monster>,
        seed: u64,
        hand: Vec<String>,
        deck: Option<Vec<String>>,
        player_max_hp: Option<i32>,
        player_max_energy: Option<i32>,
    ) -> Self {
        let mut rng = Pcg32::seed_from_u64(seed);
        // A `deck` shuffles into the draw pile and deals an opening hand —
        // the real-play path. Plain `hand` (no `deck`) is test scaffolding:
        // it sets up a scripted hand directly, with empty piles, exactly as
        // before this card-pile model existed.
        let (hand, draw_pile, deal_opening_hand) = match deck {
            Some(mut deck) => {
                deck.shuffle(&mut rng);
                (Vec::new(), deck, true)
            }
            None => (hand, Vec::new(), false),
        };
        let mut state = CombatState {
            player: Fighter {
                hp: player_hp,
                max_hp: player_max_hp.unwrap_or(player_hp),
                block: 0,
                statuses: Vec::new(),
            },
            player_energy,
            player_max_energy: player_max_energy.unwrap_or(player_energy),
            monsters,
            turn: 0,
            hand,
            draw_pile,
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
            pending: None,
            rng,
        };
        if deal_opening_hand {
            draw_cards(&mut state, HAND_SIZE);
        }
        state
    }

    // `Fighter`'s fields aren't `#[pyo3(get)]` themselves (the struct isn't a
    // pyclass), so each previously-direct `player_hp`/etc. getter becomes an
    // explicit `#[getter]` delegating to `self.player` — preserving every
    // Python-visible attribute name exactly.
    #[getter]
    fn player_hp(&self) -> i32 {
        self.player.hp
    }

    #[getter]
    fn player_block(&self) -> i32 {
        self.player.block
    }

    #[getter]
    fn pending(&self) -> Option<String> {
        self.pending.as_ref().map(|p| p.as_str().to_string())
    }

    #[getter]
    fn player_statuses(&self) -> Vec<String> {
        self.player.statuses.iter().map(|s| s.as_str().to_string()).collect()
    }

    #[getter]
    fn player_strength(&self) -> i32 {
        self.player.statuses.iter().filter_map(|s| match s {
            Status::Strength(n) => Some(*n),
            _ => None,
        }).sum()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    /// Return a copy of this state with the embedded PRNG re-seeded from
    /// `seed`. Used by MCTS to give each rollout a fresh draw sequence so
    /// the search averages over possible futures rather than optimizing for
    /// the one fixed shuffle the original state would produce.
    fn with_rng_seed(&self, seed: u64) -> Self {
        self.reseeded(seed)
    }
}

impl CombatState {
    pub(crate) fn reseeded(&self, seed: u64) -> Self {
        let mut copy = self.clone();
        copy.rng = Pcg32::seed_from_u64(seed);
        copy
    }

    /// Resolves an `Actor` identity to its `Fighter` — the single chokepoint
    /// that lets the generic, `Actor`-aware engine (`run_effect_ops`,
    /// `deal_damage`, ...) reach either combatant's HP/block/statuses
    /// symmetrically, without ever branching on Player-vs-Monster itself.
    pub(crate) fn fighter(&self, who: Actor) -> &Fighter {
        match who {
            Actor::Player => &self.player,
            Actor::Monster(i) => &self.monsters[i].fighter,
        }
    }

    pub(crate) fn fighter_mut(&mut self, who: Actor) -> &mut Fighter {
        match who {
            Actor::Player => &mut self.player,
            Actor::Monster(i) => &mut self.monsters[i].fighter,
        }
    }

    /// Indices of monsters still alive (`hp > 0`) — the default target set
    /// for non-targeted attacks (AOE) and the set of legal `SelectTarget`
    /// indices.
    pub(crate) fn living_monster_indices(&self) -> Vec<usize> {
        self.monsters
            .iter()
            .enumerate()
            .filter(|(_, m)| m.fighter.hp > 0)
            .map(|(i, _)| i)
            .collect()
    }
}
