use crate::state::CombatState;

/// Game events that statuses can react to via `Status::reactions`. These are
/// coarser-grained than `EventType` (which drives the damage-modifier pipeline)
/// — a `GameEvent` fires once per game action and may produce `EffectOp`s
/// rather than just numeric modifiers.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum GameEvent {
    SkillPlayed,
}

/// A status that can be attached to a combatant. Each status participates in
/// two pipelines: `modifier_for` (damage calculation, numeric contributions)
/// and `reactions` (game events, producing `EffectOp`s). Adding a new reactive
/// status means adding arms to both methods — no new engine logic.
#[derive(Clone, PartialEq)]
pub(crate) enum Status {
    Vulnerable,
    // Carries its stack count — unlike Vulnerable (a binary on/off debuff),
    // Strength's contribution scales with how many stacks are held.
    Strength(i32),
    // Per the wiki, Gremlin Nob's Bellow grants Enrage(n): each time the
    // player plays a Skill card, the holder gains n Strength.
    Enrage(i32),
}

impl Status {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Status::Vulnerable => "Vulnerable",
            Status::Strength(_) => "Strength",
            Status::Enrage(_) => "Enrage",
        }
    }

    /// What this status contributes when `event` fires while sitting on
    /// `side`, if anything — the "listener registration" half of the
    /// damage-modifier pipeline. Declaring the side here (not just the status
    /// type) is what lets the same `Strength` show up on either combatant and
    /// only ever amplify *that combatant's own* outgoing damage.
    fn modifier_for(&self, side: Side, event: EventType) -> Option<Modifier> {
        match (self, side, event) {
            // Per the Slay the Spire wiki, Vulnerable increases damage taken
            // from attacks by 50%, rounded down — it amplifies what its
            // holder *receives*, so it only contributes from the target side.
            (Status::Vulnerable, Side::Target, EventType::OnDamageDealt) => {
                Some(Modifier::MultiplyDamage(1.5))
            }
            // Per the Slay the Spire wiki, each stack of Strength adds its
            // count directly to attack damage *dealt* — it only contributes
            // from the attacker side, regardless of which combatant holds it.
            (Status::Strength(amount), Side::Attacker, EventType::OnDamageDealt) => {
                Some(Modifier::AddDamage(*amount))
            }
            _ => None,
        }
    }

    /// What `EffectOp`s this status fires when `event` occurs, from the
    /// perspective of the combatant holding it. An empty vec means no reaction.
    pub(crate) fn reactions(&self, event: GameEvent) -> Vec<EffectOp> {
        match (self, event) {
            (Status::Enrage(n), GameEvent::SkillPlayed) => {
                vec![EffectOp::ApplyStatusToSelf(Status::Strength(*n))]
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

/// Whether a status is a binary debuff (any number of stacks = same effect;
/// stacks extend duration, not strength). Binary debuffs must only contribute
/// one modifier per side regardless of how many stacks are present.
fn is_binary_debuff(s: &Status) -> bool {
    matches!(s, Status::Vulnerable)
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
/// cards and the monster's moves, with "self"/"target" resolved generically
/// to the right side rather than hardcoded to the player.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Actor {
    Player,
    Monster,
}

impl Actor {
    /// The combatant on the other side of whatever `self` is doing — lets
    /// damage/status resolution stay generic ("self" vs "target") without
    /// the engine ever needing to special-case which enum variant that means.
    fn opponent(self) -> Actor {
        match self {
            Actor::Player => Actor::Monster,
            Actor::Monster => Actor::Player,
        }
    }
}

/// A single step in a card's declarative effect pipeline. A generic engine
/// interprets these against a `CombatState`, so that adding an ordinary card
/// means adding data to `card_data` rather than new engine logic.
#[derive(Clone, PartialEq)]
pub(crate) enum EffectOp {
    DealDamage(i32),
    GainBlock(i32),
    // Applies to whichever combatant the card resolved a target against
    // (e.g. Bash's Vulnerable lands on the monster it struck).
    ApplyStatusToTarget(Status),
    // Applies to the player directly, regardless of targeting — for
    // self-buffs like Inflame's Strength, which never go through SelectTarget.
    ApplyStatusToSelf(Status),
}

/// Deals damage from `actor` to the opposing combatant, running it through
/// that combatant's damage-modifier pipeline first and its block second —
/// the same absorb-then-spill arithmetic regardless of which side is hitting
/// which (mirrors Slay the Spire: block reduces incoming damage from anyone).
fn deal_damage(state: &mut CombatState, actor: Actor, amount: i32) {
    let opponent = actor.opponent();
    let modified = apply_damage_modifiers(
        amount,
        &state.fighter(actor).statuses,
        &state.fighter(opponent).statuses,
        EventType::OnDamageDealt,
    );

    let target = state.fighter_mut(opponent);
    let absorbed = modified.min(target.block);
    target.block -= absorbed;
    target.hp -= modified - absorbed;
}

/// Fires `event` against every status on both combatants, collecting their
/// `reactions` and running the resulting `EffectOp`s on behalf of the holder.
/// Adding a new reactive status only requires a `Status::reactions` arm — no
/// changes here.
pub(crate) fn fire_event(state: &mut CombatState, event: GameEvent) {
    for actor in [Actor::Monster, Actor::Player] {
        let ops: Vec<EffectOp> = state
            .fighter(actor)
            .statuses
            .iter()
            .flat_map(|s| s.reactions(event))
            .collect();
        run_effect_ops(state, &ops, actor);
    }
}

/// Removes one stack of each per-turn debuff (currently only Vulnerable) from
/// `statuses` — mirrors Slay the Spire's end-of-turn debuff countdown. Called
/// once per combatant per turn: player debuffs tick when the player ends their
/// turn, monster debuffs tick after the monster has acted.
pub(crate) fn tick_debuffs(statuses: &mut Vec<Status>) {
    if let Some(pos) = statuses.iter().position(|s| matches!(s, Status::Vulnerable)) {
        statuses.remove(pos);
    }
}

pub(crate) fn run_effect_ops(state: &mut CombatState, ops: &[EffectOp], actor: Actor) {
    for op in ops {
        match op {
            EffectOp::DealDamage(amount) => deal_damage(state, actor, *amount),
            EffectOp::GainBlock(amount) => state.fighter_mut(actor).block += amount,
            EffectOp::ApplyStatusToTarget(status) => {
                state.fighter_mut(actor.opponent()).statuses.push(status.clone())
            }
            EffectOp::ApplyStatusToSelf(status) => {
                state.fighter_mut(actor).statuses.push(status.clone())
            }
        }
    }
}
