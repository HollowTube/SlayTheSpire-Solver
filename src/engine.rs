use crate::cards::{card_data, CardType};
use crate::state::{draw_cards, CombatState};
use rand::Rng;

/// Game events that statuses can react to via `Status::reactions`. These are
/// coarser-grained than `EventType` (which drives the damage-modifier pipeline)
/// — a `GameEvent` fires once per game action and may produce `EffectOp`s
/// rather than just numeric modifiers.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum GameEvent {
    SkillPlayed,
    AttackPlayed,
}

/// A status that can be attached to a combatant. Each status participates in
/// two pipelines: `modifier_for` (damage calculation, numeric contributions)
/// and `reactions` (game events, producing `EffectOp`s). Adding a new reactive
/// status means adding arms to both methods — no new engine logic.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum Status {
    Vulnerable,
    Weak,
    // Carries its stack count — unlike Vulnerable (a binary on/off debuff),
    // Strength's contribution scales with how many stacks are held.
    Strength(i32),
    // Per the wiki, Gremlin Nob's Bellow grants Enrage(n): each time the
    // player plays a Skill card, the holder gains n Strength.
    Enrage(i32),
    // Grants 2 Block each time the holder plays an Attack for this combat.
    Rage,
    // The Shrinker Beetle's Shrink: −30% damage dealt by its holder, rounded
    // down (attacker side only) — like Weak but permanent (never decays).
    Shrink,
}

impl Status {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Status::Vulnerable => "Vulnerable",
            Status::Weak => "Weak",
            Status::Strength(_) => "Strength",
            Status::Enrage(_) => "Enrage",
            Status::Rage => "Rage",
            Status::Shrink => "Shrink",
        }
    }

    /// The reverse of `as_str`, for reconstructing a combatant's status list
    /// from a (name, amount) snapshot — e.g. from an external analysis
    /// server's JSON. For binary statuses (Vulnerable, Weak, Rage) `amount`
    /// is the stack count, expanded into that many repeated entries; for
    /// `Strength`/`Enrage`, `amount` is the carried value of the single
    /// variant. Unknown names are ignored (returns an empty vec) so a
    /// snapshot from a future, richer game state degrades gracefully rather
    /// than failing to construct.
    pub(crate) fn from_name_and_amount(name: &str, amount: i32) -> Vec<Status> {
        match name {
            "Vulnerable" => vec![Status::Vulnerable; amount.max(0) as usize],
            "Weak" => vec![Status::Weak; amount.max(0) as usize],
            "Rage" => vec![Status::Rage; amount.max(0) as usize],
            "Strength" => vec![Status::Strength(amount)],
            "Enrage" => vec![Status::Enrage(amount)],
            _ => Vec::new(),
        }
    }

    /// What this status contributes when `event` fires while sitting on
    /// `side`, if anything — the "listener registration" half of the
    /// damage-modifier pipeline. Declaring the side here (not just the status
    /// type) is what lets the same `Strength` show up on either combatant and
    /// only ever amplify *that combatant's own* outgoing damage.
    fn modifier_for(&self, side: Side, event: EventType) -> Option<Modifier> {
        match (self, side, event) {
            // Vulnerable: +50% damage taken, rounded down (target side only).
            (Status::Vulnerable, Side::Target, EventType::OnDamageDealt) => {
                Some(Modifier::MultiplyDamage(1.5))
            }
            // Weak: −25% damage dealt, rounded down (attacker side only).
            (Status::Weak, Side::Attacker, EventType::OnDamageDealt) => {
                Some(Modifier::MultiplyDamage(0.75))
            }
            // Shrink: −30% damage dealt, rounded down (attacker side only).
            (Status::Shrink, Side::Attacker, EventType::OnDamageDealt) => {
                Some(Modifier::MultiplyDamage(0.70))
            }
            // Strength: flat bonus to damage dealt (attacker side only).
            (Status::Strength(amount), Side::Attacker, EventType::OnDamageDealt) => {
                Some(Modifier::AddDamage(*amount))
            }
            _ => None,
        }
    }

    /// Whether one stack of this status is consumed at the end of its holder's
    /// turn. Debuffs with a duration counter (Vulnerable, Weak) return true;
    /// permanent buffs (Strength, Enrage) return false and are never removed
    /// by `tick_debuffs`.
    pub(crate) fn decays_per_turn(&self) -> bool {
        matches!(self, Status::Vulnerable | Status::Weak)
    }

    /// What `EffectOp`s this status fires when `event` occurs, from the
    /// perspective of the combatant holding it. An empty vec means no reaction.
    pub(crate) fn reactions(&self, event: GameEvent) -> Vec<EffectOp> {
        match (self, event) {
            (Status::Enrage(n), GameEvent::SkillPlayed) => {
                vec![EffectOp::ApplyStatusToSelf(Status::Strength(*n))]
            }
            (Status::Rage, GameEvent::AttackPlayed) => {
                vec![EffectOp::GainBlock(2)]
            }
            _ => vec![],
        }
    }
}

/// Event-bus event types that statuses (and other future listeners) can hook
/// into. Only `OnDamageDealt` has a listener so far; the rest of the documented
/// set (`OnCardPlayed`, `OnTurnStart`, ...) will be added as content needs them.
#[derive(Clone, Copy, PartialEq)]
enum EventType {
    OnDamageDealt,
}

/// Which side of a damage exchange a status is sitting on — lets a status
/// declare it only contributes when it's the one *dealing* damage (Strength)
/// or only when it's the one *taking* it (Vulnerable). Without this, a status
/// that happens to exist on both combatants (e.g. the monster gaining
/// Strength via Bellow) would double-dip: amplifying its own attacks AND
/// (wrongly) the damage it takes from the player.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Attacker,
    Target,
}

/// A contribution to a calculation pipeline from a listener. The pipeline
/// (e.g. `apply_damage_modifiers`) folds these generically, so a new status
/// that multiplies or adds to damage needs no change to the pipeline itself.
#[derive(Clone, PartialEq)]
enum Modifier {
    MultiplyDamage(f64),
    AddDamage(i32),
}

/// Whether a status is a binary debuff (stacks extend duration, not effect
/// strength). Binary debuffs must only contribute one modifier per side
/// regardless of how many stacks are present.
fn is_binary_debuff(s: &Status) -> bool {
    s.decays_per_turn()
}

/// Collect modifier contributions from `statuses` acting on `side` for
/// `event`, deduplicating binary debuffs so N stacks still yields one modifier
/// (e.g. 2 Vulnerable stacks → one `MultiplyDamage(1.5)`, not two).
fn collect_modifiers(statuses: &[Status], side: Side, event: EventType) -> Vec<Modifier> {
    let mut seen_binary: std::collections::HashSet<&'static str> =
        std::collections::HashSet::new();
    statuses
        .iter()
        .filter(|s| !is_binary_debuff(s) || seen_binary.insert(s.as_str()))
        .filter_map(|s| s.modifier_for(side, event))
        .collect()
}

/// Runs `base` through every modifier that `attacker_statuses` and
/// `target_statuses` register for `event`, folding them generically in two
/// passes — flat (`AddDamage`) contributions first, then multiplicative
/// (`MultiplyDamage`) ones — matching the wiki's documented damage order
/// (e.g. Strength applies before Vulnerable). Each pass ignores modifier
/// kinds it doesn't handle, so a new status that contributes either kind from
/// either side needs no change here, only a `modifier_for` entry.
fn apply_damage_modifiers(
    base: i32,
    attacker_statuses: &[Status],
    target_statuses: &[Status],
    event: EventType,
) -> i32 {
    let modifiers: Vec<Modifier> = collect_modifiers(attacker_statuses, Side::Attacker, event)
        .into_iter()
        .chain(collect_modifiers(target_statuses, Side::Target, event))
        .collect();

    let additive = modifiers.iter().fold(base, |amount, modifier| match modifier {
        Modifier::AddDamage(delta) => amount + delta,
        Modifier::MultiplyDamage(_) => amount,
    });

    let multiplied = modifiers
        .iter()
        .fold(additive as f64, |amount, modifier| match modifier {
            Modifier::MultiplyDamage(factor) => amount * factor,
            Modifier::AddDamage(_) => amount,
        });

    multiplied.floor() as i32
}

/// Which combatant an effect pipeline is acting on behalf of — lets the same
/// `EffectOp` vocabulary and `run_effect_ops` engine drive both the player's
/// cards and a monster's moves. `Monster(i)` indexes `CombatState::monsters`,
/// so a fight can have any number of enemies. Unlike the old `Player`/
/// `Monster` pair, there's no generic "opponent" here — `run_effect_ops`
/// takes an explicit target list, since a player action might target one
/// monster (Strike), all of them (Thunderclap), or none (Defend).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Actor {
    Player,
    Monster(usize),
}

/// A single step in a card's declarative effect pipeline. A generic engine
/// interprets these against a `CombatState`, so that adding an ordinary card
/// means adding data to `card_data` rather than new engine logic.
#[derive(Clone, PartialEq)]
pub(crate) enum EffectOp {
    DealDamage(i32),
    GainBlock(i32),
    // Applies to whichever combatant(s) the action resolved as targets
    // (e.g. Bash's Vulnerable lands on the monster it struck).
    ApplyStatusToTarget(Status),
    // Applies to the player directly, regardless of targeting — for
    // self-buffs like Inflame's Strength, which never go through SelectTarget.
    ApplyStatusToSelf(Status),
    DrawCards(usize),
    // Unblockable, unpowered self-damage to `actor` — bypasses block and the
    // damage-modifier pipeline entirely (e.g. Bloodletting's HP cost is not
    // reduced by the player's own Block or amplified by their own Vulnerable).
    LoseHp(i32),
    // Grants `actor` (always the player in practice — monsters have no
    // energy pool) extra Energy this turn, on top of whatever's left after
    // paying the card's cost.
    GainEnergy(i32),
    // Heals `actor` for `amount`, capped at their `max_hp`.
    Heal(i32),
    // Pushes a named card (e.g. "Slimed") into a target's discard pile — used
    // by slime monster moves (Goop/StickyShot) to stick junk cards into the
    // player's deck. Only `Actor::Player` has card piles, so this is a no-op
    // for any other target in `targets`.
    ApplyCardToTarget(String),
    // Removes one random card matching `filter` from `actor`'s hand and moves
    // it to the exhaust pile (e.g. Cinder, TrueGrit). No-op if no card in hand
    // matches `filter`. Only meaningful for `Actor::Player` — monsters have no
    // hand.
    ExhaustRandomFromHand(HandFilter),
    // Exhausts every card in `actor`'s hand matching `filter`, granting
    // `gain_block_per_card` block to `actor` for each one exhausted (e.g.
    // SecondWind: all non-Attacks, 5 block each).
    ExhaustAllFromHand {
        filter: HandFilter,
        gain_block_per_card: i32,
    },
    // Removes a random card from `actor`'s discard pile and places it on top
    // of their draw pile (e.g. Headbutt). No-op if the discard pile is empty.
    // Only meaningful for `Actor::Player` — monsters have no card piles.
    PutRandomDiscardOnTopOfDraw,
    // Deals `base + per_unit * source` damage to each target, where `source`
    // is read from `state` at the moment this op runs (e.g. FiendFire: 0 base
    // + 7 per card in hand, evaluated before the hand is exhausted by a later
    // op in the same pipeline).
    DealDamageScaled {
        base: i32,
        per_unit: i32,
        source: ScaleSource,
    },
    // Adds a random card from `pool` to `actor`'s hand (e.g. InfernalBlade
    // generating a random Attack). Only meaningful for `Actor::Player` —
    // monsters have no hand. No-op if `pool` is empty.
    AddRandomCardToHand(Vec<String>),
}

/// What a `DealDamageScaled` op reads its multiplier from. New scaling
/// sources (e.g. exhaust pile size, current Block) extend this enum without
/// touching the `DealDamageScaled` arm itself.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ScaleSource {
    HandSize,
}

impl ScaleSource {
    fn read(&self, state: &CombatState) -> i32 {
        match self {
            ScaleSource::HandSize => state.hand.len() as i32,
        }
    }
}

/// A predicate over hand-card names, used by `EffectOp::ExhaustRandomFromHand`
/// (and future hand-manipulation ops) to restrict which cards in hand are
/// eligible — e.g. Thrash only exhausts Attacks, SecondWind only exhausts
/// non-Attacks.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum HandFilter {
    Any,
    Attack,
    NonAttack,
}

impl HandFilter {
    fn matches(&self, card_name: &str) -> bool {
        match self {
            HandFilter::Any => true,
            HandFilter::Attack | HandFilter::NonAttack => {
                let is_attack = card_data(card_name)
                    .map(|data| matches!(data.card_type, CardType::Attack))
                    .unwrap_or(false);
                if matches!(self, HandFilter::Attack) {
                    is_attack
                } else {
                    !is_attack
                }
            }
        }
    }
}

/// Deals damage from `attacker` to `target`, running it through both
/// combatants' damage-modifier pipelines first and `target`'s block second —
/// the same absorb-then-spill arithmetic regardless of which side is hitting
/// which (mirrors Slay the Spire: block reduces incoming damage from anyone).
fn deal_damage(state: &mut CombatState, attacker: Actor, target: Actor, amount: i32) {
    let modified = apply_damage_modifiers(
        amount,
        &state.fighter(attacker).statuses,
        &state.fighter(target).statuses,
        EventType::OnDamageDealt,
    );

    let target = state.fighter_mut(target);
    let absorbed = modified.min(target.block);
    target.block -= absorbed;
    target.hp -= modified - absorbed;
}

/// Fires `event` against every status on every combatant, collecting their
/// `reactions` and running the resulting `EffectOp`s on behalf of the holder.
/// Adding a new reactive status only requires a `Status::reactions` arm — no
/// changes here. None of today's reactions (Enrage, Rage) target another
/// combatant, so an empty target list is passed.
pub(crate) fn fire_event(state: &mut CombatState, event: GameEvent) {
    let actors: Vec<Actor> = std::iter::once(Actor::Player)
        .chain((0..state.monsters.len()).map(Actor::Monster))
        .collect();
    for actor in actors {
        let ops: Vec<EffectOp> = state
            .fighter(actor)
            .statuses
            .iter()
            .flat_map(|s| s.reactions(event))
            .collect();
        run_effect_ops(state, &ops, actor, &[]);
    }
}

/// Removes one stack of each duration-based debuff from `statuses` — mirrors
/// Slay the Spire's end-of-turn debuff countdown. Only statuses where
/// `decays_per_turn()` is true are touched; permanent statuses (Strength,
/// Enrage) are never affected.
pub(crate) fn tick_debuffs(statuses: &mut Vec<Status>) {
    if let Some(pos) = statuses.iter().position(|s| s.decays_per_turn()) {
        statuses.remove(pos);
    }
}

/// Interprets `ops` on behalf of `actor`. `DealDamage` and
/// `ApplyStatusToTarget` fan out over every entry in `targets` (e.g.
/// Thunderclap hits all enemies, Sword Boomerang hits its selected target
/// three times); `GainBlock`, `ApplyStatusToSelf`, and `DrawCards` affect
/// `actor` once regardless of `targets`.
pub(crate) fn run_effect_ops(state: &mut CombatState, ops: &[EffectOp], actor: Actor, targets: &[Actor]) {
    for op in ops {
        match op {
            EffectOp::DealDamage(amount) => {
                for &target in targets {
                    deal_damage(state, actor, target, *amount);
                }
            }
            EffectOp::GainBlock(amount) => state.fighter_mut(actor).block += amount,
            EffectOp::ApplyStatusToTarget(status) => {
                for &target in targets {
                    state.fighter_mut(target).statuses.push(status.clone());
                }
            }
            EffectOp::ApplyStatusToSelf(status) => {
                state.fighter_mut(actor).statuses.push(status.clone())
            }
            EffectOp::DrawCards(n) => draw_cards(state, *n),
            EffectOp::LoseHp(amount) => state.fighter_mut(actor).hp -= amount,
            EffectOp::GainEnergy(amount) => {
                if actor == Actor::Player {
                    state.player_energy += amount;
                }
            }
            EffectOp::Heal(amount) => {
                let fighter = state.fighter_mut(actor);
                fighter.hp = (fighter.hp + amount).min(fighter.max_hp);
            }
            EffectOp::ApplyCardToTarget(card_name) => {
                for &target in targets {
                    if target == Actor::Player {
                        state.discard_pile.push(card_name.clone());
                    }
                }
            }
            EffectOp::ExhaustRandomFromHand(filter) => {
                let candidates: Vec<usize> = state
                    .hand
                    .iter()
                    .enumerate()
                    .filter(|(_, name)| filter.matches(name))
                    .map(|(i, _)| i)
                    .collect();
                if !candidates.is_empty() {
                    let pick = candidates[state.rng.gen_range(0..candidates.len())];
                    let card = state.hand.remove(pick);
                    state.exhaust_pile.push(card);
                }
            }
            EffectOp::ExhaustAllFromHand {
                filter,
                gain_block_per_card,
            } => {
                let (matching, remaining): (Vec<String>, Vec<String>) =
                    state.hand.drain(..).partition(|name| filter.matches(name));
                let count = matching.len() as i32;
                state.hand = remaining;
                state.exhaust_pile.extend(matching);
                state.fighter_mut(actor).block += gain_block_per_card * count;
            }
            EffectOp::PutRandomDiscardOnTopOfDraw => {
                if !state.discard_pile.is_empty() {
                    let pick = state.rng.gen_range(0..state.discard_pile.len());
                    let card = state.discard_pile.remove(pick);
                    // `draw_cards` pops from the end, so the end is the top.
                    state.draw_pile.push(card);
                }
            }
            EffectOp::DealDamageScaled {
                base,
                per_unit,
                source,
            } => {
                let amount = base + per_unit * source.read(state);
                for &target in targets {
                    deal_damage(state, actor, target, amount);
                }
            }
            EffectOp::AddRandomCardToHand(pool) => {
                if !pool.is_empty() {
                    let pick = state.rng.gen_range(0..pool.len());
                    state.hand.push(pool[pick].clone());
                }
            }
        }
    }
}
