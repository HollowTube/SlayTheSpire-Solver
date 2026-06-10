use crate::state::{draw_cards, CombatState};

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
#[derive(Clone, PartialEq)]
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
}

impl Status {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Status::Vulnerable => "Vulnerable",
            Status::Weak => "Weak",
            Status::Strength(_) => "Strength",
            Status::Enrage(_) => "Enrage",
            Status::Rage => "Rage",
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
        }
    }
}
