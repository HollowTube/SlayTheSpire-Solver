use crate::cards::{card_data, CardType};
use crate::state::{draw_cards, CardInstance, CombatState, Fighter, Monster};
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
    // Fires whenever a card with the Exhaust keyword (`CardKeyword::Exhaust`) is
    // moved to the exhaust pile, or an `ExhaustRandomFromHand`/
    // `ExhaustAllFromHand` op exhausts a card from hand. Does NOT fire for
    // Power/Status cards leaving play — those aren't "Exhausted" in the
    // keyword sense, just removed from the game.
    CardExhausted,
    // Fires whenever `EffectOp::GainBlock` increases the player's Block
    // (e.g. Juggernaut's retaliation).
    BlockGained,
    // Fires whenever the player loses HP from an attack (hp_loss > 0 in
    // `deal_damage`), after `player_times_damaged_this_combat`/
    // `player_hp_lost_this_turn` are updated (e.g. FlameBarrier's
    // retaliation).
    DamageReceived,
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
    // Juggernaut: whenever the holder gains Block, deal `n` damage to a
    // random enemy.
    Juggernaut(i32),
    // Flame Barrier: lasts until the end of the holder's next turn. Whenever
    // the holder takes attack damage, deal 4 damage back to the attacker.
    FlameBarrier,
    // Colossus: incoming damage from an attacker that has Vulnerable is
    // halved per stack of this power.
    Colossus(i32),
    // Corruption: Skills cost 0, and whenever you play a Skill, it is
    // Exhausted instead of discarded.
    Corruption,
    // Cruelty: damage dealt to a target with Vulnerable is amplified by
    // 1.75x instead of the usual 1.5x.
    Cruelty,
    // One Two Punch: the next Attack card played this turn is played a
    // second time. Consumed on use; if unused, decays at the start of the
    // next turn (does not persist).
    OneTwoPunch,
    // Slippery: the next time the holder would lose HP, that loss is capped
    // to 1 instead - consuming one stack. Carries its stack count; stacks
    // persist across turns until consumed by a hit (handled directly in
    // `deal_damage`, not via the Modifier pipeline, since it depends on the
    // post-block HP loss and has a stateful side effect).
    Slippery(i32),
    // Setup Strike: like `Strength`, but expires at the end of the turn it
    // was gained (decays_per_turn) rather than persisting permanently.
    StrengthThisTurn(i32),
    // Unrelenting: the next Attack card the holder plays costs 0 Energy.
    // Consumed when that Attack is played (in `effective_cost`/`apply`'s
    // PlayCard branch); if unused, decays at the start of the next turn —
    // same one-shot lifecycle as `OneTwoPunch`.
    FreeAttack,
    // Pyre: at the start of each turn, gain 1 Energy.
    Pyre,
    // DrumOfBattle: at the start of each turn, Exhaust the top card of the
    // draw pile.
    BattleDrum,
    // Constrict: at the end of the player's turn, the holder takes `n`
    // unblockable damage (bypasses block, unaffected by damage modifiers).
    // Stacks add when reapplied (counter-style, like Strength); does NOT
    // decay (permanent for the combat).
    Constrict(i32),
    // Artifact: negates the next `n` debuff-type status applications to this
    // combatant. Decrements on each blocked application; non-debuff statuses
    // (Strength, block, etc.) pass through unaffected.
    Artifact(i32),
    // Frail: −25% Block gained (rounds down). Decays per turn like Weak
    // (binary debuff — multiple stacks extend duration, not effect).
    Frail(i32),
    // Minion: the combatant skips its turn entirely if the monster named
    // `leader` has hp <= 0. The `leader` field is not part of the string key
    // so all Minion variants collapse to "Minion" for binary dedup.
    Minion { leader: String },
    // Stun: the combatant skips its entire next turn (no intent resolution,
    // no damage/effects). Consumed after the skip — designed for reuse by
    // Ceremonial Beast's self-stun at the Plow threshold.
    Stun,
    // Infested: when holder's HP reaches ≤ 0, spawns `count` monsters with
    // `minion_name` and `minion_hp`, each starting with Stun(1). Designed for
    // reuse by future summon-on-death enemies.
    Infested {
        minion_name: String,
        minion_hp: i32,
        count: i32,
    },
    // Tangled: while active on the player, all Attack cards cost +n energy
    // to play. Removed at end of the player's turn (decays_per_turn).
    Tangled(i32),
    // Slow: Attack-card damage dealt to the holder is multiplied by
    // (1 + 0.1 * cards_played_this_turn), floor-rounded. Inherent to the
    // holder (e.g. Bygone Effigy), not a conventional stack-based debuff —
    // never decays, not blocked by Artifact.
    Slow(i32),
    // Plow: when the holder's HP is reduced to ≤ n by an unblocked hit,
    // strip all Strength from the holder, override its intent to "Stun",
    // and remove this status. Fires once (Ceremonial Beast).
    Plow(i32),
    // Ringing: player debuff — limits the player to one PlayCard action per
    // turn. Decays at end of the player's turn. Applied by Ceremonial
    // Beast's Beast Cry.
    Ringing,
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
            Status::Juggernaut(_) => "Juggernaut",
            Status::FlameBarrier => "FlameBarrier",
            Status::Colossus(_) => "Colossus",
            Status::Corruption => "Corruption",
            Status::Cruelty => "Cruelty",
            Status::OneTwoPunch => "OneTwoPunch",
            Status::Slippery(_) => "Slippery",
            Status::StrengthThisTurn(_) => "StrengthThisTurn",
            Status::FreeAttack => "FreeAttack",
            Status::Pyre => "Pyre",
            Status::BattleDrum => "BattleDrum",
            Status::Constrict(_) => "Constrict",
            Status::Artifact(_) => "Artifact",
            Status::Frail(_) => "Frail",
            Status::Minion { .. } => "Minion",
            Status::Stun => "Stun",
            Status::Infested { .. } => "Infested",
            Status::Tangled(_) => "Tangled",
            Status::Slow(_) => "Slow",
            Status::Plow(_) => "Plow",
            Status::Ringing => "Ringing",
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
            "Juggernaut" => vec![Status::Juggernaut(amount)],
            "FlameBarrier" => vec![Status::FlameBarrier; amount.max(0) as usize],
            "Colossus" => vec![Status::Colossus(amount)],
            "Corruption" => vec![Status::Corruption; amount.max(0) as usize],
            "Cruelty" => vec![Status::Cruelty; amount.max(0) as usize],
            "OneTwoPunch" => vec![Status::OneTwoPunch; amount.max(0) as usize],
            "Slippery" => vec![Status::Slippery(amount)],
            "Strength" => vec![Status::Strength(amount)],
            "Enrage" => vec![Status::Enrage(amount)],
            "StrengthThisTurn" => vec![Status::StrengthThisTurn(amount)],
            "FreeAttack" => vec![Status::FreeAttack; amount.max(0) as usize],
            "Pyre" => vec![Status::Pyre; amount.max(0) as usize],
            "BattleDrum" => vec![Status::BattleDrum; amount.max(0) as usize],
            "Constrict" => vec![Status::Constrict(amount)],
            "Artifact" => vec![Status::Artifact(amount)],
            "Frail" => vec![Status::Frail(amount)],
            "Minion" => vec![Status::Minion {
                leader: "Kin Priest".to_string(),
            }],
            "Stun" => vec![Status::Stun; amount.max(0) as usize],
            "Infested" => vec![Status::Infested {
                minion_name: "Wriggler".to_string(),
                minion_hp: 21,
                count: amount,
            }],
            "Tangled" => vec![Status::Tangled(amount)],
            "Slow" => vec![Status::Slow(amount)],
            "Plow" => vec![Status::Plow(amount)],
            "Ringing" => vec![Status::Ringing; amount.max(0) as usize],
            _ => Vec::new(),
        }
    }

    /// What this status contributes when `event` fires while sitting on
    /// `side`, if anything — the "listener registration" half of the
    /// damage-modifier pipeline. Declaring the side here (not just the status
    /// type) is what lets the same `Strength` show up on either combatant and
    /// only ever amplify *that combatant's own* outgoing damage.
    fn modifier_for(
        &self,
        side: Side,
        event: EventType,
        other_side_statuses: &[Status],
    ) -> Option<Modifier> {
        match (self, side, event) {
            // Vulnerable: +50% damage taken, rounded down (target side only).
            // Cruelty on the attacker amplifies this to +75% instead.
            (Status::Vulnerable, Side::Target, EventType::OnDamageDealt) => {
                let multiplier = if other_side_statuses.contains(&Status::Cruelty) {
                    1.75
                } else {
                    1.5
                };
                Some(Modifier::MultiplyDamage(multiplier))
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
            // Setup Strike's Strength-this-turn: same flat bonus as Strength,
            // but expires at end of turn via `decays_per_turn`.
            (Status::StrengthThisTurn(amount), Side::Attacker, EventType::OnDamageDealt) => {
                Some(Modifier::AddDamage(*amount))
            }
            // Colossus: incoming damage is halved per stack, but only from an
            // attacker who currently has Vulnerable (target side only).
            (Status::Colossus(n), Side::Target, EventType::OnDamageDealt)
                if other_side_statuses.contains(&Status::Vulnerable) =>
            {
                Some(Modifier::MultiplyDamage(0.5_f64.powi(*n)))
            }
            // Frail: −25% Block gained, rounded down (target side only).
            (Status::Frail(_), Side::Target, EventType::OnBlockGained) => {
                Some(Modifier::MultiplyBlock(0.75))
            }
            _ => None,
        }
    }

    /// Whether one stack of this status is consumed at the end of its holder's
    /// turn. Debuffs with a duration counter (Vulnerable, Weak) return true;
    /// permanent buffs (Strength, Enrage) return false and are never removed
    /// by `tick_debuffs`.
    pub(crate) fn decays_per_turn(&self) -> bool {
        matches!(
            self,
            Status::Vulnerable
                | Status::Weak
                | Status::FlameBarrier
                | Status::OneTwoPunch
                | Status::Rage
                | Status::Colossus(_)
                | Status::StrengthThisTurn(_)
                | Status::FreeAttack
                | Status::Frail(_)
        )
    }

    /// Whether this status is a debuff (negative effect on the holder).
    /// Artifact blocks only debuff-type status applications; buffs and other
    /// neutral statuses pass through.
    pub(crate) fn is_debuff(&self) -> bool {
        matches!(
            self,
            Status::Vulnerable | Status::Weak | Status::Shrink | Status::Constrict(_) | Status::Frail(_) | Status::Tangled(_) | Status::Ringing
        )
    }

    /// What `EffectOp`s this status fires when `event` occurs, from the
    /// perspective of the combatant holding it. An empty vec means no reaction.
    pub(crate) fn reactions(&self, event: GameEvent) -> Vec<EffectOp> {
        match (self, event) {
            (Status::Enrage(n), GameEvent::SkillPlayed) => {
                vec![EffectOp::ApplyStatusToSelf(Status::Strength(*n))]
            }
            (Status::Rage, GameEvent::AttackPlayed) => {
                vec![EffectOp::GainBlock(3)]
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
            (Status::Juggernaut(n), GameEvent::BlockGained) => {
                vec![EffectOp::DealDamageToRandomEnemy(*n)]
            }
            (Status::FlameBarrier, GameEvent::DamageReceived) => {
                vec![EffectOp::DealDamageToLastAttacker(4)]
            }
            (Status::Pyre, GameEvent::TurnStart) => vec![EffectOp::GainEnergy(1)],
            (Status::BattleDrum, GameEvent::TurnStart) => vec![EffectOp::ExhaustTopOfDrawPile],
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
    OnBlockGained,
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
    MultiplyBlock(f64),
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
fn collect_modifiers(
    statuses: &[Status],
    side: Side,
    event: EventType,
    other_side_statuses: &[Status],
) -> Vec<Modifier> {
    let mut seen_binary: std::collections::HashSet<&'static str> =
        std::collections::HashSet::new();
    statuses
        .iter()
        .filter(|s| !is_binary_debuff(s) || seen_binary.insert(s.as_str()))
        .filter_map(|s| s.modifier_for(side, event, other_side_statuses))
        .collect()
}

/// Runs `base` through every modifier that `attacker_statuses` and
/// `target_statuses` register for `event`, folding them generically in two
/// passes — flat (`AddDamage`) contributions first, then multiplicative
/// (`MultiplyDamage`) ones — matching the wiki's documented damage order
/// (e.g. Strength applies before Vulnerable). Each pass ignores modifier
/// kinds it doesn't handle, so a new status that contributes either kind from
/// either side needs no change here, only a `modifier_for` entry.
///
/// If `is_attack_card` is true and `target_statuses` contains `Status::Slow`,
/// the result is additionally multiplied by `(1 + 0.1 * cards_played_this_turn)`,
/// floor-rounded — the Bygone Effigy's inherent Slow damage scaling.
fn apply_damage_modifiers(
    base: i32,
    attacker_statuses: &[Status],
    target_statuses: &[Status],
    event: EventType,
    is_attack_card: bool,
    cards_played_this_turn: i32,
) -> i32 {
    let modifiers: Vec<Modifier> = collect_modifiers(attacker_statuses, Side::Attacker, event, target_statuses)
        .into_iter()
        .chain(collect_modifiers(target_statuses, Side::Target, event, attacker_statuses))
        .collect();

    let additive = modifiers.iter().fold(base, |amount, modifier| match modifier {
        Modifier::AddDamage(delta) => amount + delta,
        _ => amount,
    });

    let multiplied = modifiers
        .iter()
        .fold(additive as f64, |amount, modifier| match modifier {
            Modifier::MultiplyDamage(factor) => amount * factor,
            _ => amount,
        });

    let result = multiplied.floor() as i32;

    if is_attack_card && target_statuses.iter().any(|s| matches!(s, Status::Slow(_))) {
        let factor = 1.0 + 0.1 * cards_played_this_turn as f64;
        (result as f64 * factor).floor() as i32
    } else {
        result
    }
}

/// Runs `base` through every block modifier that `holder_statuses` registers
/// for `EventType::OnBlockGained` — only `Modifier::MultiplyBlock` variants
/// are applied, so damage-only modifiers (AddDamage, MultiplyDamage) pass
/// through unchanged. Right now only Frail contributes here, but the pipeline
/// is generic enough for future block-modifying statuses.
fn apply_block_modifiers(base: i32, holder_statuses: &[Status]) -> i32 {
    let modifiers: Vec<Modifier> = collect_modifiers(holder_statuses, Side::Target, EventType::OnBlockGained, &[]);
    modifiers.iter().fold(base as f64, |amount, modifier| match modifier {
        Modifier::MultiplyBlock(factor) => amount * factor,
        _ => amount,
    }).floor() as i32
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
    // Deals `amount` damage to a randomly chosen living enemy, independent of
    // `targets` (e.g. Juggernaut's BlockGained-triggered retaliation).
    DealDamageToRandomEnemy(i32),
    // Deals `amount` damage to the monster that most recently dealt damage to
    // the player this combat (`CombatState::last_attacker`), independent of
    // `targets` (e.g. FlameBarrier's DamageReceived-triggered retaliation).
    // No-op if no monster has attacked yet.
    DealDamageToLastAttacker(i32),
    // Grants `actor` `base + per_unit * source` Block (e.g. Evil Eye: 8 base
    // + 8 more if a card was Exhausted this turn).
    GainBlockScaled {
        base: i32,
        per_unit: i32,
        source: ScaleSource,
    },
    // Grants `actor` `base + per_unit * source` Energy (e.g. Forgotten
    // Ritual: 0 base + 3 if a card was Exhausted this turn).
    GainEnergyScaled {
        base: i32,
        per_unit: i32,
        source: ScaleSource,
    },
    // Exhaust the top card of the player's draw pile (e.g. DrumOfBattle's
    // turn-start trigger).
    ExhaustTopOfDrawPile,
    // Add a copy of the named card to the player's discard pile (e.g. Anger).
    AddCardToDiscard(String),
    // Spawns a new monster with the given name and HP onto the field
    // (e.g. Fogmog's Illusion spawns Eye With Teeth). The new monster
    // is added at the end of the monsters list.
    SpawnMonster(String, i32),
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
    // 1 if the player has Exhausted a card this turn, else 0 (e.g. Evil Eye,
    // Forgotten Ritual).
    ExhaustedCardThisTurn,
}

impl ScaleSource {
    fn read(&self, state: &CombatState, actor: Actor, target: Actor) -> i32 {
        match self {
            ScaleSource::HandSize => state.hand.len() as i32,
            ScaleSource::CurrentBlock => state.fighter(actor).block,
            ScaleSource::StrikeCardsInDeck => {
                let count_strikes = |pile: &[CardInstance]| pile.iter().filter(|c| c.name.contains("Strike")).count();
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
            ScaleSource::ExhaustedCardThisTurn => state.player_exhausted_card_this_turn as i32,
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
                let is_attack = card_data(card_name, 0)
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
///
/// `is_attack_card` tells the damage pipeline whether the source is an Attack
/// card (used by `Status::Slow`), and `cards_played_this_turn` drives the
/// Slow multiplier. These are only meaningful for player-Attack-card damage;
/// all other callers pass `false` / `0` respectively.
fn deal_damage(state: &mut CombatState, attacker: Actor, target: Actor, amount: i32, is_attack_card: bool, cards_played_this_turn: i32) {
    let modified = apply_damage_modifiers(
        amount,
        &state.fighter(attacker).statuses,
        &state.fighter(target).statuses,
        EventType::OnDamageDealt,
        is_attack_card,
        cards_played_this_turn,
    );

    let hp_loss;
    {
        let target_fighter = state.fighter_mut(target);
        let absorbed = modified.min(target_fighter.block);
        target_fighter.block -= absorbed;
        let mut loss = modified - absorbed;

        // Slippery: the next time the holder would lose HP, cap that loss to
        // 1 and consume one stack.
        if loss > 1 {
            if let Some(pos) = target_fighter
                .statuses
                .iter()
                .position(|s| matches!(s, Status::Slippery(n) if *n > 0))
            {
                loss = 1;
                match &mut target_fighter.statuses[pos] {
                    Status::Slippery(n) => *n -= 1,
                    _ => unreachable!(),
                }
                if matches!(target_fighter.statuses[pos], Status::Slippery(0)) {
                    target_fighter.statuses.remove(pos);
                }
            }
        }

        hp_loss = loss;
        target_fighter.hp -= hp_loss;
    }

    // Track how often/how much the player has been hit, for cards that scale
    // off it (e.g. TearAsunder, Spite).
    if target == Actor::Player && hp_loss > 0 {
        state.player_times_damaged_this_combat += 1;
        state.player_hp_lost_this_turn = true;
        state.last_attacker = match attacker {
            Actor::Monster(i) => Some(i),
            Actor::Player => None,
        };
        fire_event(state, GameEvent::DamageReceived);
    }
}

/// Fires `event` against every status on every combatant, collecting their
/// `reactions` and running the resulting `EffectOp`s on behalf of the holder.
/// Adding a new reactive status only requires a `Status::reactions` arm — no
/// changes here. None of today's reactions (Enrage, Rage) target another
/// combatant, so an empty target list is passed.
pub(crate) fn fire_event(state: &mut CombatState, event: GameEvent) {
    // Tracks "has the player Exhausted a card this turn" for Evil Eye /
    // Forgotten Ritual — set here, the single chokepoint every
    // CardExhausted-firing path runs through, rather than at each call site.
    if event == GameEvent::CardExhausted {
        state.player_exhausted_card_this_turn = true;
    }
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
        run_effect_ops(state, &ops, actor, &[], false);
    }
}

/// Removes one stack of each duration-based debuff from `statuses` — mirrors
/// Slay the Spire's end-of-turn debuff countdown. Only statuses where
/// `decays_per_turn()` is true are touched; permanent statuses (Strength,
/// Enrage) are never affected. Each distinct decaying status (by `as_str()`)
/// loses one stack per call, so e.g. Vulnerable and a one-turn power like
/// Colossus can expire independently in the same turn.
pub(crate) fn tick_debuffs(statuses: &mut Vec<Status>) {
    let mut decremented = std::collections::HashSet::new();
    let mut i = 0;
    while i < statuses.len() {
        if statuses[i].decays_per_turn() && decremented.insert(statuses[i].as_str()) {
            statuses.remove(i);
        } else {
            i += 1;
        }
    }
}

/// Interprets `ops` on behalf of `actor`. `DealDamage` and
/// `ApplyStatusToTarget` fan out over every entry in `targets` (e.g.
/// Thunderclap hits all enemies, Sword Boomerang hits its selected target
/// three times); `GainBlock`, `ApplyStatusToSelf`, and `DrawCards` affect
/// `actor` once regardless of `targets`.
///
/// `is_attack_card` tells the damage pipeline whether the source is an Attack
/// card (used by `Status::Slow`). Pass `false` for monster moves, status
/// reactions, and any non-Attack card effects.
pub(crate) fn run_effect_ops(state: &mut CombatState, ops: &[EffectOp], actor: Actor, targets: &[Actor], is_attack_card: bool) {
    for op in ops {
        match op {
            EffectOp::DealDamage(amount) => {
                for &target in targets {
                    deal_damage(state, actor, target, *amount, is_attack_card, state.cards_played_this_turn);
                }
            }
            EffectOp::GainBlock(amount) => {
                let modified = apply_block_modifiers(*amount, &state.fighter(actor).statuses);
                state.fighter_mut(actor).block += modified;
                if actor == Actor::Player {
                    fire_event(state, GameEvent::BlockGained);
                }
            }
            EffectOp::ApplyStatusToTarget(status) => {
                for &target in targets {
                    if status.is_debuff() {
                        let consumed = {
                            let fighter = state.fighter_mut(target);
                            let pos = fighter.statuses.iter().position(|s| {
                                matches!(s, Status::Artifact(n) if *n > 0)
                            });
                            if let Some(pos) = pos {
                                match &mut fighter.statuses[pos] {
                                    Status::Artifact(n) => *n -= 1,
                                    _ => unreachable!(),
                                }
                                if matches!(fighter.statuses[pos], Status::Artifact(0)) {
                                    fighter.statuses.remove(pos);
                                }
                                true
                            } else {
                                false
                            }
                        };
                        if consumed {
                            continue;
                        }
                    }
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
                        state.discard_pile.push(CardInstance::new(card_name.clone()));
                    }
                }
            }
            EffectOp::AddCardToDiscard(card_name) => {
                state.discard_pile.push(CardInstance::new(card_name.clone()));
            }
            EffectOp::SpawnMonster(name, hp) => {
                // Spawned monster starts with no intent — the monster
                // processing loop in lib.rs will resolve one via
                // select_next_intent on its first EndTurn.
                state.monsters.push(Monster {
                    fighter: Fighter {
                        hp: *hp,
                        max_hp: *hp,
                        block: 0,
                        statuses: vec![],
                    },
                    attack: 0,
                    name: Some(name.clone()),
                    intent: None,
                    last_move: None,
                    move_streak: 0,
                    moves_used: Vec::new(),
                });
            }
            EffectOp::ExhaustTopOfDrawPile => {
                if let Some(card) = state.draw_pile.pop() {
                    state.exhaust_pile.push(card);
                    fire_event(state, GameEvent::CardExhausted);
                }
            }
            EffectOp::ExhaustRandomFromHand(filter) => {
                let candidates: Vec<usize> = state
                    .hand
                    .iter()
                    .enumerate()
                    .filter(|(_, card)| filter.matches(&card.name))
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
                let (matching, remaining): (Vec<CardInstance>, Vec<CardInstance>) =
                    state.hand.drain(..).partition(|card| filter.matches(&card.name));
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
                    .filter(|(_, card)| filter.matches(&card.name))
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
                    deal_damage(state, actor, target, amount, is_attack_card, state.cards_played_this_turn);
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
                        deal_damage(state, actor, target, *amount, is_attack_card, state.cards_played_this_turn);
                    }
                }
            }
            EffectOp::AddRandomCardToHand(pool) => {
                if !pool.is_empty() {
                    let pick = state.rng.gen_range(0..pool.len());
                    state.hand.push(CardInstance::new(pool[pick].clone()));
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
                    deal_damage(state, actor, Actor::Monster(i), *amount, is_attack_card, state.cards_played_this_turn);
                }
            }
            EffectOp::DealDamageToRandomEnemy(amount) => {
                let living = state.living_monster_indices();
                if !living.is_empty() {
                    let pick = living[state.rng.gen_range(0..living.len())];
                    deal_damage(state, actor, Actor::Monster(pick), *amount, is_attack_card, state.cards_played_this_turn);
                }
            }
            EffectOp::DealDamageToLastAttacker(amount) => {
                if let Some(i) = state.last_attacker {
                    deal_damage(state, actor, Actor::Monster(i), *amount, is_attack_card, state.cards_played_this_turn);
                }
            }
            EffectOp::GainBlockScaled { base, per_unit, source } => {
                let raw = base + per_unit * source.read(state, actor, actor);
                let modified = apply_block_modifiers(raw, &state.fighter(actor).statuses);
                state.fighter_mut(actor).block += modified;
                if actor == Actor::Player {
                    fire_event(state, GameEvent::BlockGained);
                }
            }
            EffectOp::GainEnergyScaled { base, per_unit, source } => {
                let amount = base + per_unit * source.read(state, actor, actor);
                if actor == Actor::Player {
                    state.player_energy += amount;
                }
            }
        }
    }
}
