//! Verified combat fixtures: a test declares the combat it needs as data, and
//! [`Fixtures::ensure`] returns a state proven equal to it, from any starting
//! state.
//!
//! Every field has a deterministic default, so a field a test leaves out is
//! not left to chance. [`plan`] decides which console commands reach a
//! fixture from a fresh fight, and [`Fixture::diff`] checks the result field
//! by field; both are pure.

use std::collections::BTreeMap;
use std::fmt;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::backend::{Backend, BridgeError, Setters};
use crate::command::RunSpec;
use crate::console::check::{is_power, MAX_HAND};
use crate::console::{
    slugify, CombatPile, ConsoleCommand, ConsoleWait, Encounter, PowerName, Relic, Target,
};
use crate::lifecycle::{Game, Launcher};
use crate::menu::{classify, Phase};
use crate::settle::{run_is, Session, SettlePolicy};
use crate::state::{CardId, CardRef, CombatView, Entity, GameState, Power};

/// The seed of [`Fixture::new`]'s run.
pub const DEFAULT_SEED: &str = "HOL108FIX";

/// What [`FixtureError::Unreachable`] says the unreachable fields need.
pub const NEEDS_SETTERS: &str = "exact setters (HOL-103)";

/// The player's hp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hp {
    Max,
    Exact(i32),
}

/// The player's energy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Energy {
    /// What the turn starts with (`max_energy`).
    Start,
    Exact(i32),
}

/// An enemy's powers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnemyPowers {
    /// Whatever the enemy spawns with; not checked.
    AsSpawned,
    /// Exactly these, by console id and amount.
    Exact(Vec<(PowerName, i32)>),
}

/// `CardId` as its canonical bridge name.
mod card_id {
    use serde::{Deserialize, Deserializer, Serializer};
    use sts_ids::{BridgeId, CardId};

    pub fn serialize<S: Serializer>(id: &CardId, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(id.bridge_name())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<CardId, D::Error> {
        let name = String::deserialize(d)?;
        CardId::from_bridge(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown card `{name}`")))
    }
}

/// A card in a pile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardSpec {
    #[serde(with = "card_id")]
    pub id: CardId,
    pub upgraded: bool,
}

impl CardSpec {
    pub fn upgraded(id: CardId) -> Self {
        Self { id, upgraded: true }
    }
}

impl From<CardId> for CardSpec {
    fn from(id: CardId) -> Self {
        Self {
            id,
            upgraded: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSpec {
    pub hp: Hp,
    pub block: i32,
    pub energy: Energy,
    /// Exactly these powers.
    pub powers: Vec<(PowerName, i32)>,
}

impl Default for PlayerSpec {
    fn default() -> Self {
        Self {
            hp: Hp::Max,
            block: 0,
            energy: Energy::Start,
            powers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemySpec {
    pub hp: i32,
    pub block: i32,
    pub powers: EnemyPowers,
}

impl EnemySpec {
    /// At `hp`, no block, powers as spawned.
    pub fn new(hp: i32) -> Self {
        Self {
            hp,
            block: 0,
            powers: EnemyPowers::AsSpawned,
        }
    }

    pub fn block(mut self, block: i32) -> Self {
        self.block = block;
        self
    }

    pub fn powers(mut self, powers: impl IntoIterator<Item = (PowerName, i32)>) -> Self {
        self.powers = EnemyPowers::Exact(powers.into_iter().collect());
        self
    }
}

/// A combat a test needs, declared field by field. Piles are exact and
/// ordered (`draw` is top first); relics are an exact multiset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fixture {
    pub run: RunSpec,
    pub encounter: Encounter,
    pub relics: Vec<Relic>,
    pub player: PlayerSpec,
    /// One per enemy the encounter spawns, in order.
    pub enemies: Vec<EnemySpec>,
    pub hand: Vec<CardSpec>,
    pub draw: Vec<CardSpec>,
    pub discard: Vec<CardSpec>,
    pub exhaust: Vec<CardSpec>,
}

fn starting_relics(character: &str) -> Vec<Relic> {
    let id = match character.to_ascii_lowercase().as_str() {
        "ironclad" => "BURNING_BLOOD",
        _ => return Vec::new(),
    };
    Relic::new(id).into_iter().collect()
}

impl Fixture {
    /// `encounter` in an Ironclad run seeded [`DEFAULT_SEED`], ascension 0,
    /// with the starting relic, the player at max hp with the turn's
    /// energy, and every pile empty. Declare each enemy with [`Self::enemy`].
    pub fn new(encounter: Encounter) -> Self {
        Self {
            run: RunSpec::new("Ironclad", DEFAULT_SEED),
            encounter,
            relics: starting_relics("Ironclad"),
            player: PlayerSpec::default(),
            enemies: Vec::new(),
            hand: Vec::new(),
            draw: Vec::new(),
            discard: Vec::new(),
            exhaust: Vec::new(),
        }
    }

    /// Another run; the relics become that character's starting relics
    /// where known, none otherwise.
    pub fn run(mut self, run: RunSpec) -> Self {
        self.relics = starting_relics(&run.character);
        self.run = run;
        self
    }

    pub fn relics(mut self, relics: impl IntoIterator<Item = Relic>) -> Self {
        self.relics = relics.into_iter().collect();
        self
    }

    pub fn player_hp(mut self, hp: Hp) -> Self {
        self.player.hp = hp;
        self
    }

    pub fn player_block(mut self, block: i32) -> Self {
        self.player.block = block;
        self
    }

    pub fn energy(mut self, energy: Energy) -> Self {
        self.player.energy = energy;
        self
    }

    pub fn player_power(mut self, power: PowerName, amount: i32) -> Self {
        self.player.powers.push((power, amount));
        self
    }

    pub fn enemy(mut self, enemy: EnemySpec) -> Self {
        self.enemies.push(enemy);
        self
    }

    pub fn hand<C: Into<CardSpec>>(mut self, cards: impl IntoIterator<Item = C>) -> Self {
        self.hand = cards.into_iter().map(Into::into).collect();
        self
    }

    pub fn draw<C: Into<CardSpec>>(mut self, cards: impl IntoIterator<Item = C>) -> Self {
        self.draw = cards.into_iter().map(Into::into).collect();
        self
    }

    pub fn discard<C: Into<CardSpec>>(mut self, cards: impl IntoIterator<Item = C>) -> Self {
        self.discard = cards.into_iter().map(Into::into).collect();
        self
    }

    pub fn exhaust<C: Into<CardSpec>>(mut self, cards: impl IntoIterator<Item = C>) -> Self {
        self.exhaust = cards.into_iter().map(Into::into).collect();
        self
    }

    /// The fields no backend with `setters` can reach, whatever the game
    /// shows. Sends nothing.
    pub fn unreachable(&self, setters: Setters) -> Vec<String> {
        let mut out = Vec::new();
        for (name, pile) in [
            ("piles.draw", &self.draw),
            ("piles.discard", &self.discard),
            ("piles.exhaust", &self.exhaust),
        ] {
            if !setters.upgraded_in_piles && pile.iter().any(|c| c.upgraded) {
                out.push(name.to_string());
            }
        }
        if self.hand.len() > MAX_HAND {
            out.push("piles.hand".to_string());
        }
        if matches!(self.player.hp, Hp::Exact(hp) if hp <= 0) {
            out.push("player.hp".to_string());
        }
        if self.player.block < 0 {
            out.push("player.block".to_string());
        }
        if matches!(self.player.energy, Energy::Exact(e) if e < 0) {
            out.push("player.energy".to_string());
        }
        let mut powers = |who: &str, powers: &[(PowerName, i32)]| {
            for (p, n) in powers {
                if *n < 0 && !setters.remove_power {
                    out.push(format!("{who}.powers[{p}]"));
                }
            }
        };
        powers("player", &self.player.powers);
        for (i, e) in self.enemies.iter().enumerate() {
            if let EnemyPowers::Exact(ps) = &e.powers {
                powers(&format!("enemies[{i}]"), ps);
            }
        }
        for (i, e) in self.enemies.iter().enumerate() {
            if e.hp <= 0 {
                out.push(format!("enemies[{i}].hp"));
            }
            if e.block < 0 {
                out.push(format!("enemies[{i}].block"));
            }
        }
        out
    }

    /// Every declared field that `state` does not show, by name: `player.hp`,
    /// `enemies[0].powers[VULNERABLE_POWER]`, `piles.draw`, `run.relics`, ….
    /// Empty when `state` is this fixture.
    pub fn diff(&self, state: &GameState) -> Vec<FieldDiff> {
        self.fields(state)
            .into_iter()
            .filter(|f| f.expected != f.observed)
            .map(|f| FieldDiff {
                field: f.name,
                expected: f.expected,
                observed: f.observed,
                needs_setter: false,
            })
            .collect()
    }

    /// The values `state` shows for every declared field: equal for two
    /// states that are the same fixture.
    pub fn project(&self, state: &GameState) -> Projection {
        Projection(
            self.fields(state)
                .into_iter()
                .map(|f| (f.name, f.observed))
                .collect(),
        )
    }

    fn fields(&self, state: &GameState) -> Vec<Field> {
        let mut out = Vec::new();
        let mut field = |name: String, expected: String, observed: String| {
            out.push(Field {
                name,
                expected,
                observed,
            })
        };
        match &state.run {
            None => field("run".into(), self.run.to_string(), "no run".into()),
            Some(run) => {
                field("run.seed".into(), self.run.seed.clone(), run.seed.clone());
                field(
                    "run.character".into(),
                    self.run.character.to_ascii_lowercase(),
                    run.character
                        .clone()
                        .unwrap_or_default()
                        .to_ascii_lowercase(),
                );
                field(
                    "run.ascension".into(),
                    self.run.ascension.to_string(),
                    run.ascension.to_string(),
                );
                let mut want: Vec<&str> = self.relics.iter().map(Relic::as_str).collect();
                want.sort_unstable();
                let mut have: Vec<String> = run.relics.iter().map(|r| slugify(r)).collect();
                have.sort_unstable();
                field("run.relics".into(), list(&want), list(&have));
            }
        }
        let Some(c) = state
            .combat
            .as_ref()
            .filter(|_| state.screen.as_str() == "COMBAT_PLAYER_TURN")
        else {
            field(
                "combat".into(),
                "a fight on COMBAT_PLAYER_TURN".into(),
                state.screen.to_string(),
            );
            return out;
        };
        let p = &c.player;
        field(
            "player.hp".into(),
            self.player.hp.resolve(p.max_hp).to_string(),
            p.hp.to_string(),
        );
        field(
            "player.block".into(),
            self.player.block.to_string(),
            p.block.to_string(),
        );
        field(
            "player.energy".into(),
            self.player.energy.resolve(p.max_energy).to_string(),
            p.energy.to_string(),
        );
        for (key, want, have) in power_pairs(&self.player.powers, &p.powers) {
            field(
                format!("player.powers[{key}]"),
                want.to_string(),
                have.to_string(),
            );
        }
        field(
            "enemies".into(),
            self.enemies.len().to_string(),
            c.enemies.len().to_string(),
        );
        for (i, (spec, e)) in self.enemies.iter().zip(&c.enemies).enumerate() {
            field(
                format!("enemies[{i}].hp"),
                spec.hp.to_string(),
                e.hp.to_string(),
            );
            field(
                format!("enemies[{i}].block"),
                spec.block.to_string(),
                e.block.to_string(),
            );
            if let EnemyPowers::Exact(ps) = &spec.powers {
                for (key, want, have) in power_pairs(ps, &e.powers) {
                    field(
                        format!("enemies[{i}].powers[{key}]"),
                        want.to_string(),
                        have.to_string(),
                    );
                }
            }
        }
        let piles = &c.piles;
        let hand: Vec<String> = piles
            .hand
            .iter()
            .map(|h| card_name(&h.id, h.upgraded))
            .collect();
        field("piles.hand".into(), specs(&self.hand), list(&hand));
        for (name, want, have) in [
            ("piles.draw", &self.draw, &piles.draw),
            ("piles.discard", &self.discard, &piles.discard),
            ("piles.exhaust", &self.exhaust, &piles.exhaust),
        ] {
            field(name.into(), specs(want), refs(have));
        }
        out
    }
}

impl Hp {
    fn resolve(self, max: i32) -> i32 {
        match self {
            Hp::Max => max,
            Hp::Exact(hp) => hp,
        }
    }
}

impl Energy {
    fn resolve(self, start: i32) -> i32 {
        match self {
            Energy::Start => start,
            Energy::Exact(e) => e,
        }
    }
}

struct Field {
    name: String,
    expected: String,
    observed: String,
}

/// One declared field the game does not show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDiff {
    /// e.g. `player.energy`, `enemies[0].hp`, `piles.draw`, `run.relics`.
    pub field: String,
    pub expected: String,
    pub observed: String,
    /// No console command can reach it: it waits for [`Setters`].
    pub needs_setter: bool,
}

impl fmt::Display for FieldDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: expected {}, observed {}",
            self.field, self.expected, self.observed
        )?;
        if self.needs_setter {
            f.write_str(" (needs a setter)")?;
        }
        Ok(())
    }
}

/// The declared fields' values in one state, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection(pub Vec<(String, String)>);

fn list<S: AsRef<str>>(items: &[S]) -> String {
    let items: Vec<&str> = items.iter().map(AsRef::as_ref).collect();
    format!("[{}]", items.join(", "))
}

fn card_name(id: &Entity<CardId>, upgraded: bool) -> String {
    format!("{}{}", id.name(), if upgraded { "+" } else { "" })
}

fn specs(cards: &[CardSpec]) -> String {
    let names: Vec<String> = cards
        .iter()
        .map(|c| card_name(&Entity::Known(c.id), c.upgraded))
        .collect();
    list(&names)
}

fn refs(cards: &[CardRef]) -> String {
    let names: Vec<String> = cards.iter().map(|c| card_name(&c.id, c.upgraded)).collect();
    list(&names)
}

/// The console id of a power the game reports, e.g. `VULNERABLE_POWER`.
fn power_id(power: &Power) -> String {
    if let Entity::Known(status) = &power.id {
        if let Some(id) = status
            .bridge_names()
            .iter()
            .map(|n| slugify(n))
            .find(|n| PowerName::new(n).is_ok())
        {
            return id;
        }
    }
    slugify(power.id.name())
}

/// Declared and observed amounts for every power in either, by console id.
fn power_pairs(want: &[(PowerName, i32)], have: &[Power]) -> Vec<(String, i32, i32)> {
    let mut pairs: BTreeMap<String, (i32, i32)> = BTreeMap::new();
    for (p, n) in want {
        pairs.entry(p.as_str().to_string()).or_default().0 += n;
    }
    for p in have {
        let key = pairs
            .keys()
            .find(|k| is_power(&p.id, k))
            .cloned()
            .unwrap_or_else(|| power_id(p));
        pairs.entry(key).or_default().1 += p.amount;
    }
    pairs
        .into_iter()
        .map(|(k, (want, have))| (k, want, have))
        .collect()
}

/// One step toward a fixture.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Step {
    /// A console command, sent verified by [`Session::console`].
    Console(ConsoleCommand),
}

fn amount(n: i32) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

/// The steps that take `fresh`, a fight just started on its player turn, to
/// `fixture`; or the fields no step a backend with `setters` has can reach,
/// each with `needs_setter`.
///
/// The order is player hp (while block is still 0), enemy hp, powers, block,
/// energy, then the piles: what is not wanted is drawn into the hand and
/// removed, then the declared cards are added in order and the upgraded hand
/// cards upgraded. Relics are a run's, not a fight's: [`Fixtures::ensure`]
/// sets them before the fight.
pub fn plan(
    fixture: &Fixture,
    fresh: &GameState,
    setters: Setters,
) -> Result<Vec<Step>, Vec<FieldDiff>> {
    let statics = fixture.unreachable(setters);
    if !statics.is_empty() {
        return Err(statics
            .into_iter()
            .map(|field| FieldDiff {
                field,
                expected: "a value within the game's limits".into(),
                observed: "not checked".into(),
                needs_setter: true,
            })
            .collect());
    }
    let Some(c) = &fresh.combat else {
        return Err(vec![FieldDiff {
            field: "combat".into(),
            expected: "a fight on COMBAT_PLAYER_TURN".into(),
            observed: fresh.screen.to_string(),
            needs_setter: false,
        }]);
    };
    if fixture.enemies.len() != c.enemies.len() {
        return Err(vec![FieldDiff {
            field: "enemies".into(),
            expected: fixture.enemies.len().to_string(),
            observed: c.enemies.len().to_string(),
            needs_setter: false,
        }]);
    }
    let mut p = Planner {
        steps: Vec::new(),
        unreachable: Vec::new(),
    };
    let player = &c.player;

    let mut player_block = player.block;
    let hp = fixture.player.hp.resolve(player.max_hp);
    if hp < player.hp {
        p.console(ConsoleCommand::Damage {
            target: Target::Player,
            amount: amount(player.hp - hp + player_block),
        });
        player_block = 0;
    } else if hp > player.max_hp {
        p.needs("player.hp", hp, player.hp, setters.exact_hp);
    } else if hp > player.hp {
        p.console(ConsoleCommand::HealPlayer {
            amount: amount(hp - player.hp),
        });
    }

    let mut enemy_blocks = Vec::new();
    for (i, (spec, e)) in fixture.enemies.iter().zip(&c.enemies).enumerate() {
        let mut block = e.block;
        if spec.hp < e.hp {
            p.console(ConsoleCommand::Damage {
                target: Target::Enemy(i),
                amount: amount(e.hp - spec.hp + block),
            });
            block = 0;
        } else if spec.hp > e.hp {
            p.needs(&format!("enemies[{i}].hp"), spec.hp, e.hp, setters.exact_hp);
        }
        enemy_blocks.push(block);
    }

    p.powers(
        "player",
        Target::Player,
        &fixture.player.powers,
        &player.powers,
        setters,
    );
    for (i, (spec, e)) in fixture.enemies.iter().zip(&c.enemies).enumerate() {
        if let EnemyPowers::Exact(ps) = &spec.powers {
            p.powers(
                &format!("enemies[{i}]"),
                Target::Enemy(i),
                ps,
                &e.powers,
                setters,
            );
        }
    }

    p.block(
        "player.block",
        Target::Player,
        fixture.player.block,
        player_block,
    );
    for (i, (spec, block)) in fixture.enemies.iter().zip(enemy_blocks).enumerate() {
        p.block(
            &format!("enemies[{i}].block"),
            Target::Enemy(i),
            spec.block,
            block,
        );
    }

    let energy = fixture.player.energy.resolve(player.max_energy);
    if energy > player.energy {
        p.console(ConsoleCommand::Energy {
            amount: amount(energy - player.energy),
        });
    } else if energy < player.energy {
        p.needs("player.energy", energy, player.energy, setters.energy);
    }

    p.piles(fixture, c, setters);

    if p.unreachable.is_empty() {
        Ok(p.steps)
    } else {
        Err(p.unreachable)
    }
}

struct Planner {
    steps: Vec<Step>,
    unreachable: Vec<FieldDiff>,
}

impl Planner {
    fn console(&mut self, command: ConsoleCommand) {
        self.steps.push(Step::Console(command));
    }

    fn needs(
        &mut self,
        field: &str,
        expected: impl fmt::Display,
        observed: impl fmt::Display,
        have: bool,
    ) {
        if !have {
            self.unreachable.push(FieldDiff {
                field: field.to_string(),
                expected: expected.to_string(),
                observed: observed.to_string(),
                needs_setter: true,
            });
        }
    }

    fn powers(
        &mut self,
        who: &str,
        target: Target,
        want: &[(PowerName, i32)],
        have: &[Power],
        setters: Setters,
    ) {
        for (key, w, h) in power_pairs(want, have) {
            if w > h {
                if let Ok(power) = PowerName::new(&key) {
                    self.console(ConsoleCommand::Power {
                        power,
                        amount: amount(w - h),
                        target,
                    });
                }
            } else if w < h {
                self.needs(&format!("{who}.powers[{key}]"), w, h, setters.remove_power);
            }
        }
    }

    fn block(&mut self, field: &str, target: Target, want: i32, now: i32) {
        if want > now {
            self.console(ConsoleCommand::Block {
                target,
                amount: amount(want - now),
            });
        } else if want < now {
            self.needs(field, want, now, false);
        }
    }

    fn piles(&mut self, fixture: &Fixture, c: &CombatView, setters: Setters) {
        let piles = &c.piles;
        let same = |want: &[CardSpec], have: &[(Entity<CardId>, bool)]| {
            want.len() == have.len()
                && want
                    .iter()
                    .zip(have)
                    .all(|(w, (id, up))| *id == Entity::Known(w.id) && w.upgraded == *up)
        };
        let pairs = |cards: &[CardRef]| -> Vec<(Entity<CardId>, bool)> {
            cards.iter().map(|c| (c.id.clone(), c.upgraded)).collect()
        };
        let hand: Vec<(Entity<CardId>, bool)> = piles
            .hand
            .iter()
            .map(|h| (h.id.clone(), h.upgraded))
            .collect();
        if same(&fixture.hand, &hand)
            && same(&fixture.draw, &pairs(&piles.draw))
            && same(&fixture.discard, &pairs(&piles.discard))
            && same(&fixture.exhaust, &pairs(&piles.exhaust))
        {
            return;
        }

        let remove = |planner: &mut Self, field: &str, cards: &[(Entity<CardId>, bool)]| {
            for (id, _) in cards {
                match id {
                    Entity::Known(card) => {
                        planner.console(ConsoleCommand::RemoveCardFromHand { card: *card })
                    }
                    Entity::Unknown(name) => planner.needs(
                        field,
                        "a card the registry knows",
                        name,
                        setters.exact_piles,
                    ),
                }
            }
        };
        remove(self, "piles.hand", &hand);
        let draw = pairs(&piles.draw);
        for batch in draw.chunks(MAX_HAND) {
            self.console(ConsoleCommand::Draw {
                count: batch.len() as u32,
            });
            remove(self, "piles.draw", batch);
        }
        let discard = pairs(&piles.discard);
        if discard.len() > MAX_HAND {
            self.needs(
                "piles.discard",
                "[]",
                refs(&piles.discard),
                setters.exact_piles,
            );
        } else if !discard.is_empty() {
            self.console(ConsoleCommand::Draw {
                count: discard.len() as u32,
            });
            remove(self, "piles.discard", &discard);
        }
        if !piles.exhaust.is_empty() {
            self.needs(
                "piles.exhaust",
                "[]",
                refs(&piles.exhaust),
                setters.exact_piles,
            );
        }

        for card in &fixture.hand {
            self.console(ConsoleCommand::AddCard {
                card: card.id,
                pile: CombatPile::Hand,
            });
        }
        for (hand_index, card) in fixture.hand.iter().enumerate() {
            if card.upgraded {
                self.console(ConsoleCommand::Upgrade { hand_index });
            }
        }
        for (pile, cards) in [
            (CombatPile::Draw, &fixture.draw),
            (CombatPile::Discard, &fixture.discard),
            (CombatPile::Exhaust, &fixture.exhaust),
        ] {
            for card in cards {
                self.console(ConsoleCommand::AddCard {
                    card: card.id,
                    pile,
                });
            }
        }
    }
}

/// The relic commands that make the run's relics `fixture.relics`.
fn relic_steps(fixture: &Fixture, state: &GameState) -> Vec<ConsoleCommand> {
    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for r in &fixture.relics {
        counts.entry(r.as_str()).or_default().0 += 1;
    }
    let owned: Vec<String> = state
        .run
        .iter()
        .flat_map(|run| &run.relics)
        .map(|r| slugify(r))
        .collect();
    let mut steps = Vec::new();
    for id in &owned {
        match Relic::new(id) {
            Ok(relic) => counts.entry(relic.as_str()).or_default().1 += 1,
            // An id the catalog does not know cannot be removed; the diff
            // reports it.
            Err(_) => continue,
        }
    }
    for (id, (want, have)) in counts {
        let Ok(relic) = Relic::new(id) else { continue };
        for _ in have..want {
            steps.push(ConsoleCommand::RelicAdd(relic));
        }
        for _ in want..have {
            steps.push(ConsoleCommand::RelicRemove(relic));
        }
    }
    steps
}

/// How a verified state was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixturePath {
    /// A fight in the run already in progress.
    InRun,
    /// After a full seeded reset.
    Reset,
}

/// Whether instant mode is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstantMode {
    On,
    /// The bridge does not report it, so it was never toggled.
    Unverified,
}

/// How long one full seeded reset took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetTimings {
    /// To the bridge accepting `start_run`.
    pub to_accepted: Duration,
    /// To the run ready for a room change.
    pub to_ready: Duration,
    /// To the fixture's fight settled.
    pub to_first_fight: Option<Duration>,
}

/// How long one [`Fixtures::ensure`] took, and what it sent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Timings {
    pub total: Duration,
    /// The last reset, when there was one.
    pub reset: Option<ResetTimings>,
    /// Console commands sent.
    pub commands: usize,
    pub resets: usize,
}

/// A state proven equal to a fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedState {
    pub state: GameState,
    pub path: FixturePath,
    pub timings: Timings,
    pub instant: InstantMode,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FixtureError {
    /// The fixture asks for something no available step can do; nothing was
    /// sent.
    #[error("unreachable fixture fields {fields:?}: they need {needs}")]
    Unreachable {
        fields: Vec<String>,
        needs: &'static str,
    },

    /// The game did not show the fixture, after a full reset when
    /// `after_reset`.
    #[error("the fixture was not met{}: {}", if *after_reset { " after a full reset" } else { "" }, describe(fields))]
    Mismatch {
        fields: Vec<FieldDiff>,
        after_reset: bool,
    },

    #[error("setting up the fixture failed{}: {source}", if *after_reset { " after a full reset" } else { "" })]
    Bridge {
        #[source]
        source: BridgeError,
        after_reset: bool,
    },
}

fn describe(fields: &[FieldDiff]) -> String {
    let items: Vec<String> = fields.iter().map(ToString::to_string).collect();
    items.join("; ")
}

/// What a failed attempt left to do.
enum Miss {
    Fields(Vec<FieldDiff>),
    Bridge(BridgeError),
}

impl From<BridgeError> for Miss {
    fn from(e: BridgeError) -> Self {
        Miss::Bridge(e)
    }
}

/// Errors that a full reset may cure; the rest are bugs or a missing mod.
fn resettable(e: &BridgeError) -> bool {
    !matches!(
        e,
        BridgeError::Schema { .. }
            | BridgeError::Protocol { .. }
            | BridgeError::Lifecycle { .. }
            | BridgeError::ExplorerUnavailable { .. }
    )
}

/// The session-level setup, done once per run.
struct Prepared {
    run: RunSpec,
    instant: InstantMode,
    deck_blank: bool,
}

/// Sets up verified combat fixtures on one game.
///
/// Per session (and again after every reset): the game started, the
/// fixture's seeded run in progress, instant mode on where the bridge reports
/// it, and the master deck emptied, so every fight opens with empty piles.
/// Per [`Fixtures::ensure`]: a fight, normalised to the fixture and verified
/// field by field, with one full seeded reset if that fails. Teardown is the
/// next setup.
pub struct Fixtures<B: Backend, L: Launcher> {
    game: Game<B, L>,
    wait: ConsoleWait,
    prepared: Option<Prepared>,
    resets: usize,
}

/// One console command at a time, counted.
struct Driver<'a, B: Backend> {
    session: Session<&'a mut B>,
    wait: &'a ConsoleWait,
    commands: &'a mut usize,
}

impl<B: Backend> Driver<'_, B> {
    fn run(&mut self, command: &ConsoleCommand) -> Result<GameState, BridgeError> {
        *self.commands += 1;
        self.session.console_with(command, self.wait)
    }
}

impl<B: Backend, L: Launcher> Fixtures<B, L> {
    pub fn new(game: Game<B, L>) -> Self {
        Self {
            game,
            wait: ConsoleWait::for_session(&SettlePolicy::default()),
            prepared: None,
            resets: 0,
        }
    }

    /// How console commands wait; the default is [`ConsoleWait::for_session`]
    /// over the default policy.
    pub fn with_wait(mut self, wait: ConsoleWait) -> Self {
        self.wait = wait;
        self
    }

    pub fn game(&self) -> &Game<B, L> {
        &self.game
    }

    pub fn game_mut(&mut self) -> &mut Game<B, L> {
        &mut self.game
    }

    /// A session on the game under this fixture set's wait, e.g. to dirty
    /// the state in a test.
    pub fn session(&mut self) -> Session<&mut B> {
        Session::with_policy(self.game.backend_mut(), self.wait.settle.clone())
    }

    pub fn wait(&self) -> &ConsoleWait {
        &self.wait
    }

    /// Full seeded resets so far.
    pub fn resets(&self) -> usize {
        self.resets
    }

    /// Returns a state proven equal to `fixture`, from any starting state.
    ///
    /// A fixture no available step can reach fails at once, sending nothing
    /// ([`FixtureError::Unreachable`]). Otherwise: a fight in the run in
    /// progress (or after a reset, when there is no such run), normalised and
    /// verified field by field. On a mismatch or a failed step, one full
    /// seeded reset and a second attempt; what still differs fails as
    /// [`FixtureError::Mismatch`] naming each field.
    pub fn ensure(&mut self, fixture: &Fixture) -> Result<VerifiedState, FixtureError> {
        let started = Instant::now();
        let setters = self.game.backend().setters();
        let fields = fixture.unreachable(setters);
        if !fields.is_empty() {
            return Err(FixtureError::Unreachable {
                fields,
                needs: NEEDS_SETTERS,
            });
        }
        let mut t = Tally::default();
        let bridge = |after_reset| {
            move |source| FixtureError::Bridge {
                source,
                after_reset,
            }
        };
        let first = self
            .begin(&fixture.run, &mut t)
            .map_err(Miss::Bridge)
            .and_then(|()| self.attempt(fixture, setters, &mut t));
        let state = match first {
            Ok(state) => state,
            Err(miss) => {
                let after_reset = t.resets > 0;
                match miss {
                    Miss::Bridge(e) if !resettable(&e) || after_reset => {
                        return Err(bridge(after_reset)(e))
                    }
                    Miss::Fields(fields) if after_reset => {
                        return Err(FixtureError::Mismatch {
                            fields,
                            after_reset,
                        })
                    }
                    _ => {}
                }
                self.reset(&fixture.run, &mut t).map_err(bridge(true))?;
                match self.attempt(fixture, setters, &mut t) {
                    Ok(state) => state,
                    Err(Miss::Fields(fields)) => {
                        return Err(FixtureError::Mismatch {
                            fields,
                            after_reset: true,
                        })
                    }
                    Err(Miss::Bridge(e)) => return Err(bridge(true)(e)),
                }
            }
        };
        let instant = self
            .prepared
            .as_ref()
            .map_or(InstantMode::Unverified, |p| p.instant);
        Ok(VerifiedState {
            state,
            path: if t.resets > 0 {
                FixturePath::Reset
            } else {
                FixturePath::InRun
            },
            timings: Timings {
                total: started.elapsed(),
                reset: t.reset,
                commands: t.commands,
                resets: t.resets,
            },
            instant,
        })
    }

    /// Brings the game up and into `run`, resetting when it is anywhere
    /// else.
    fn begin(&mut self, run: &RunSpec, t: &mut Tally) -> Result<(), BridgeError> {
        if self.prepared.is_none() {
            self.game.start()?;
        }
        let mut probe = self.game.backend_mut().probe()?;
        if classify(&probe) == Phase::Loading {
            probe = self.game.wait_run_ready()?;
        }
        let in_run = matches!(classify(&probe), Phase::InRun | Phase::Transition)
            && run_is(run, &self.game.backend_mut().state()?).is_ok();
        if !in_run {
            return self.reset(run, t);
        }
        let stale = match &self.prepared {
            Some(p) if &p.run == run => {
                // The game may have been reset outside these fixtures.
                p.instant == InstantMode::On
                    && self.game.backend_mut().health()?.fast_mode != Some(true)
            }
            _ => true,
        };
        if stale {
            self.prepare(run, t)?;
        }
        Ok(())
    }

    /// A full seeded reset: the main menu, a new seeded run, and its
    /// session setup.
    fn reset(&mut self, run: &RunSpec, t: &mut Tally) -> Result<(), BridgeError> {
        self.resets += 1;
        t.resets += 1;
        let started = Instant::now();
        self.prepared = None;
        let begun = self.game.start_run(run)?;
        t.reset = Some(ResetTimings {
            to_accepted: begun.accepted_after,
            to_ready: begun.ready_after,
            to_first_fight: None,
        });
        t.reset_started = Some(started);
        self.prepare(run, t)
    }

    /// Instant mode, then an empty master deck.
    fn prepare(&mut self, run: &RunSpec, t: &mut Tally) -> Result<(), BridgeError> {
        let Self { game, wait, .. } = self;
        let mut d = Driver {
            session: Session::with_policy(game.backend_mut(), wait.settle.clone()),
            wait,
            commands: &mut t.commands,
        };
        // Settled first: the toggle is a console command, and one sent
        // mid-transition can wedge the game.
        let state = d.session.wait_settled()?;
        let instant = match d.session.set_instant_with(true, d.wait) {
            Ok(_) => InstantMode::On,
            Err(BridgeError::ActionRefused { .. }) => InstantMode::Unverified,
            Err(e) => return Err(e),
        };
        let deck_blank = blank_deck(&mut d, &state)?;
        self.prepared = Some(Prepared {
            run: run.clone(),
            instant,
            deck_blank,
        });
        Ok(())
    }

    fn attempt(
        &mut self,
        fixture: &Fixture,
        setters: Setters,
        t: &mut Tally,
    ) -> Result<GameState, Miss> {
        let deck_blank = self.prepared.as_ref().is_some_and(|p| p.deck_blank);
        let Self { game, wait, .. } = self;
        let mut d = Driver {
            session: Session::with_policy(game.backend_mut(), wait.settle.clone()),
            wait,
            commands: &mut t.commands,
        };
        let state = d.session.wait_settled()?;
        for command in relic_steps(fixture, &state) {
            d.run(&command)?;
        }
        if deck_blank {
            blank_deck(&mut d, &state)?;
        }
        let mut state = d.run(&ConsoleCommand::Fight(fixture.encounter))?;
        if let (Some(started), Some(reset)) = (t.reset_started.take(), t.reset.as_mut()) {
            reset.to_first_fight = Some(started.elapsed());
        }
        let steps = plan(fixture, &state, setters).map_err(Miss::Fields)?;
        for step in steps {
            match step {
                Step::Console(command) => state = d.run(&command)?,
            }
        }
        let diff = fixture.diff(&state);
        if diff.is_empty() {
            Ok(state)
        } else {
            Err(Miss::Fields(diff))
        }
    }
}

#[derive(Default)]
struct Tally {
    commands: usize,
    resets: usize,
    reset: Option<ResetTimings>,
    reset_started: Option<Instant>,
}

/// Removes every master-deck card; `false` when that cannot be done (a card
/// the registry does not know, or a removal the game did not take), in which
/// case [`plan`] clears each fight's piles instead.
fn blank_deck<B: Backend>(d: &mut Driver<'_, B>, state: &GameState) -> Result<bool, BridgeError> {
    let deck = state.run.iter().flat_map(|r| &r.deck);
    for card in deck {
        let Entity::Known(card) = card.id else {
            return Ok(false);
        };
        match d.run(&ConsoleCommand::RemoveCardFromDeck { card }) {
            Ok(_) => {}
            Err(BridgeError::ActionRefused { .. } | BridgeError::PostconditionFailed { .. }) => {
                return Ok(false)
            }
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}
