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
    TurnStart,
    // Fires whenever an `EffectOp::LoseHp` reduces a combatant's HP (e.g.
    // Inferno's own turn-start self-damage, or other self-damage cards).
    HpLost,
    // Fires whenever a card with the Exhaust keyword (`exhausts: true`) is
    // moved to the exhaust pile, or an `ExhaustRandomFromHand`/
    // `ExhaustAllFromHand` op exhausts a card from hand. Does NOT fire for
    // Power/Status cards leaving play — those aren't "Exhausted" in the
    // keyword sense, just removed from the game.
    CardExhausted,
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
    // Demon Form: at the start of each turn, the holder gains 2 Strength.
    DemonForm,
    // Crimson Mantle: at the start of each turn, gain 8 Block and lose HP
    // equal to the carried amount (unblockable), which then increases by 1
    // for next turn.
    CrimsonMantle(i32),
    // Inferno: at the start of each turn, lose 1 HP (unblockable). Whenever
    // the holder loses HP on their turn (including from this trigger),
    // deal 6 damage to all enemies.
    Inferno,
    // Aggression: at the start of each turn, return a random Attack card
    // from the discard pile to hand.
    Aggression,
    // Dark Embrace: whenever a card is Exhausted, draw 1 card.
    DarkEmbrace,
    // Feel No Pain: whenever a card is Exhausted, gain 3 Block.
    FeelNoPain,
    // Barricade: Block is no longer removed at the start of your turn.
    Barricade,
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
            Status::DemonForm => "DemonForm",
            Status::CrimsonMantle(_) => "CrimsonMantle",
            Status::Inferno => "Inferno",
            Status::Aggression => "Aggression",
            Status::DarkEmbrace => "DarkEmbrace",
            Status::FeelNoPain => "FeelNoPain",
            Status::Barricade => "Barricade",
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
            "DemonForm" => vec![Status::DemonForm; amount.max(0) as usize],
            "CrimsonMantle" => vec![Status::CrimsonMantle(amount)],
            "Inferno" => vec![Status::Inferno; amount.max(0) as usize],
            "Aggression" => vec![Status::Aggression; amount.max(0) as usize],
            "DarkEmbrace" => vec![Status::DarkEmbrace; amount.max(0) as usize],
            "FeelNoPain" => vec![Status::FeelNoPain; amount.max(0) as usize],
            "Barricade" => vec![Status::Barricade; amount.max(0) as usize],
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
            (Status::DemonForm, GameEvent::TurnStart) => {
                vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))]
            }
            (Status::CrimsonMantle(n), GameEvent::TurnStart) => vec![
                EffectOp::GainBlock(8),
                EffectOp::LoseHp(*n),
                EffectOp::EscalateSelfStatus(Status::CrimsonMantle(n + 1)),
            ],
            (Status::Inferno, GameEvent::TurnStart) => vec![EffectOp::LoseHp(1)],
            (Status::Inferno, GameEvent::HpLost) => {
                vec![EffectOp::DealDamageToAllEnemies(6)]
            }
            (Status::Aggression, GameEvent::TurnStart) => {
                vec![EffectOp::ReturnRandomDiscardToHand(HandFilter::Attack)]
            }
            (Status::DarkEmbrace, GameEvent::CardExhausted) => {
                vec![EffectOp::DrawCards(1)]
            }
            (Status::FeelNoPain, GameEvent::CardExhausted) => {
                vec![EffectOp::GainBlock(3)]
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
    // Removes the first status on `actor` whose `as_str()` matches `new`'s,
    // then pushes `new` — used by self-mutating turn counters (e.g.
    // CrimsonMantle's increasing self-damage) to replace their carried
    // amount each turn.
    EscalateSelfStatus(Status),
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
    // Removes a random card matching `filter` from `actor`'s discard pile
    // and adds it directly to their hand (e.g. Aggression's turn-start
    // "return a random discarded Attack to hand"). No-op if no card in the
    // discard pile matches `filter`. Only meaningful for `Actor::Player`.
    ReturnRandomDiscardToHand(HandFilter),
    // Deals `base + per_unit * source` damage to each target, where `source`
    // is read from `state` at the moment this op runs (e.g. FiendFire: 0 base
    // + 7 per card in hand, evaluated before the hand is exhausted by a later
    // op in the same pipeline).
    DealDamageScaled {
        base: i32,
        per_unit: i32,
        source: ScaleSource,
    },
    // Deals `amount` damage to each target, repeated `hits_base +
    // hits_per_unit * hits_source` times — each repetition is a separate hit
    // for damage-modifier/block purposes (e.g. TearAsunder, Spite,
    // Dismantle).
    DealDamageRepeated {
        amount: i32,
        hits_base: i32,
        hits_per_unit: i32,
        hits_source: ScaleSource,
    },
    // Adds a random card from `pool` to `actor`'s hand (e.g. InfernalBlade
    // generating a random Attack). Only meaningful for `Actor::Player` —
    // monsters have no hand. No-op if `pool` is empty.
    AddRandomCardToHand(Vec<String>),
    // Doubles the number of Vulnerable stacks currently on each target —
    // a special-cased push applied *before* normal damage application (and
    // its Vulnerable-stack consumption) in the same effects list (e.g.
    // MoltenFist).
    DoubleVulnerableOnTarget,
    // Grants `actor` Strength equal to each target's *current* number of
    // Vulnerable stacks — meant to run after an `ApplyStatusToTarget(
    // Vulnerable)` earlier in the same effects list, so it reads the
    // resulting stack count (e.g. Dominate).
    GainStrengthEqualToTargetVulnerable,
    // Deals `amount` damage to every living enemy, independent of `targets`
    // (e.g. Inferno's HpLost-triggered retaliation, which fires from a status
    // reaction rather than a targeted card play).
    DealDamageToAllEnemies(i32),
}

/// What a `DealDamageScaled` op reads its multiplier from. New scaling
/// sources (e.g. exhaust pile size, current Block) extend this enum without
/// touching the `DealDamageScaled` arm itself.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ScaleSource {
    HandSize,
    // `actor`'s current Block (e.g. BodySlam: damage = current block).
    CurrentBlock,
    // Number of cards named "Strike" anywhere in `actor`'s deck — i.e. across
    // hand, draw pile, discard pile, and exhaust pile, which together always
    // total the full deck for the duration of a fight (e.g. PerfectedStrike).
    StrikeCardsInDeck,
    // Number of cards currently in `actor`'s exhaust pile (e.g. AshenStrike).
    ExhaustPileSize,
    // Number of Vulnerable stacks currently on `target` (e.g. Bully).
    VulnerableStacksOnTarget,
    // Number of Attack cards `actor` has played so far this turn, not
    // counting the card currently resolving (e.g. Conflagration).
    AttacksPlayedThisTurn,
    // Number of times the player has lost HP from an attack so far this
    // combat (e.g. TearAsunder).
    DamageTakenThisCombat,
    // 1 if the player has lost HP this turn, else 0 (e.g. Spite).
    HpLostThisTurn,
    // 1 if `target` currently has any Vulnerable stacks, else 0 (e.g.
    // Dismantle).
    TargetHasVulnerable,
}

impl ScaleSource {
    fn read(&self, state: &CombatState, actor: Actor, target: Actor) -> i32 {
        match self {
            ScaleSource::HandSize => state.hand.len() as i32,
            ScaleSource::CurrentBlock => state.fighter(actor).block,
            ScaleSource::StrikeCardsInDeck => {
                let count_strikes = |pile: &[String]| pile.iter().filter(|c| *c == "Strike").count();
                (count_strikes(&state.hand)
                    + count_strikes(&state.draw_pile)
                    + count_strikes(&state.discard_pile)
                    + count_strikes(&state.exhaust_pile)) as i32
            }
            ScaleSource::ExhaustPileSize => state.exhaust_pile.len() as i32,
            ScaleSource::VulnerableStacksOnTarget => state
                .fighter(target)
                .statuses
                .iter()
                .filter(|s| **s == Status::Vulnerable)
                .count() as i32,
            ScaleSource::AttacksPlayedThisTurn => state.attacks_played_this_turn,
            ScaleSource::DamageTakenThisCombat => state.player_times_damaged_this_combat,
            ScaleSource::HpLostThisTurn => state.player_hp_lost_this_turn as i32,
            ScaleSource::TargetHasVulnerable => state
                .fighter(target)
                .statuses
                .iter()
                .any(|s| *s == Status::Vulnerable) as i32,
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

    let hp_loss;
    {
        let target_fighter = state.fighter_mut(target);
        let absorbed = modified.min(target_fighter.block);
        target_fighter.block -= absorbed;
        hp_loss = modified - absorbed;
        target_fighter.hp -= hp_loss;
    }

    // Track how often/how much the player has been hit, for cards that scale
    // off it (e.g. TearAsunder, Spite).
    if target == Actor::Player && hp_loss > 0 {
        state.player_times_damaged_this_combat += 1;
        state.player_hp_lost_this_turn = true;
    }
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
            EffectOp::EscalateSelfStatus(new_status) => {
                let statuses = &mut state.fighter_mut(actor).statuses;
                if let Some(pos) = statuses.iter().position(|s| s.as_str() == new_status.as_str()) {
                    statuses.remove(pos);
                }
                statuses.push(new_status.clone());
            }
            EffectOp::LoseHp(amount) => {
                state.fighter_mut(actor).hp -= amount;
                if *amount > 0 {
                    fire_event(state, GameEvent::HpLost);
                }
            }
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
                    fire_event(state, GameEvent::CardExhausted);
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
                for _ in 0..count {
                    fire_event(state, GameEvent::CardExhausted);
                }
            }
            EffectOp::PutRandomDiscardOnTopOfDraw => {
                if !state.discard_pile.is_empty() {
                    let pick = state.rng.gen_range(0..state.discard_pile.len());
                    let card = state.discard_pile.remove(pick);
                    // `draw_cards` pops from the end, so the end is the top.
                    state.draw_pile.push(card);
                }
            }
            EffectOp::ReturnRandomDiscardToHand(filter) => {
                let candidates: Vec<usize> = state
                    .discard_pile
                    .iter()
                    .enumerate()
                    .filter(|(_, name)| filter.matches(name))
                    .map(|(i, _)| i)
                    .collect();
                if !candidates.is_empty() {
                    let pick = candidates[state.rng.gen_range(0..candidates.len())];
                    let card = state.discard_pile.remove(pick);
                    state.hand.push(card);
                }
            }
            EffectOp::DealDamageScaled {
                base,
                per_unit,
                source,
            } => {
                for &target in targets {
                    let amount = base + per_unit * source.read(state, actor, target);
                    deal_damage(state, actor, target, amount);
                }
            }
            EffectOp::DealDamageRepeated {
                amount,
                hits_base,
                hits_per_unit,
                hits_source,
            } => {
                for &target in targets {
                    let hits = hits_base + hits_per_unit * hits_source.read(state, actor, target);
                    for _ in 0..hits {
                        deal_damage(state, actor, target, *amount);
                    }
                }
            }
            EffectOp::AddRandomCardToHand(pool) => {
                if !pool.is_empty() {
                    let pick = state.rng.gen_range(0..pool.len());
                    state.hand.push(pool[pick].clone());
                }
            }
            EffectOp::DoubleVulnerableOnTarget => {
                for &target in targets {
                    let count = state
                        .fighter(target)
                        .statuses
                        .iter()
                        .filter(|s| **s == Status::Vulnerable)
                        .count();
                    for _ in 0..count {
                        state.fighter_mut(target).statuses.push(Status::Vulnerable);
                    }
                }
            }
            EffectOp::GainStrengthEqualToTargetVulnerable => {
                for &target in targets {
                    let count = state
                        .fighter(target)
                        .statuses
                        .iter()
                        .filter(|s| **s == Status::Vulnerable)
                        .count() as i32;
                    state.fighter_mut(actor).statuses.push(Status::Strength(count));
                }
            }
            EffectOp::DealDamageToAllEnemies(amount) => {
                for i in state.living_monster_indices() {
                    deal_damage(state, actor, Actor::Monster(i), *amount);
                }
            }
        }
    }
}
