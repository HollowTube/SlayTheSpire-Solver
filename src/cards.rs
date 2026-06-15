use crate::engine::{EffectOp, HandFilter, ScaleSource, Status};
use std::collections::HashSet;

#[derive(Clone, PartialEq)]
pub(crate) enum CardType {
    Attack,
    Skill,
    Power,
    // Non-interactive "junk" cards monsters stick into the player's deck
    // (e.g. Slimed) — like Powers, they exhaust on play rather than
    // returning to discard.
    Status,
}

/// Boolean-ish per-card flags, mirroring STS2's `CardModel.Keywords:
/// IReadOnlySet<CardKeyword>`. Kept as a set (rather than separate bools) so
/// upgrades can add/remove individual keywords without new `CardData` fields.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum CardKeyword {
    // Whether this card goes to the exhaust pile (rather than discard) after
    // being played. Power/Status cards always exhaust regardless of this
    // keyword (handled separately in `apply`); this covers Attacks/Skills
    // that the wiki explicitly marks with the Exhaust keyword (e.g. Offering,
    // Tremble, Impervious, NotYet).
    Exhaust,
    // Ethereal: if this card is still in hand at the end of the turn, it
    // exhausts instead of being discarded (handled in the `EndTurn` handler).
    Ethereal,
    // Unplayable: never appears as a legal `PlayCard:` action regardless of
    // cost/energy (e.g. status cards like Dazed).
    Unplayable,
    // Innate: always starts in the opening hand (e.g. Aggression+). Not yet
    // wired into the engine's opening-hand logic; declared so upgrades that
    // add it (per STS2's `OnUpgrade`) round-trip through `card_data`.
    Innate,
}

/// A card's energy cost and declarative effect pipeline (run once any
/// `RequestChoice` steps, e.g. `SelectTarget`, have been resolved into a
/// `PendingDecision`). Adding an ordinary card means adding an entry here,
/// not new engine logic.
// `targeted` tells the generic `PlayCard:` handler whether to enter
// `SelectTarget` before running `effects` — e.g. `Strike` needs a target to
// deal damage to, while `Defend` resolves immediately against the player.
pub(crate) struct CardData {
    pub(crate) cost: i32,
    pub(crate) targeted: bool,
    pub(crate) card_type: CardType,
    pub(crate) effects: Vec<EffectOp>,
    pub(crate) keywords: HashSet<CardKeyword>,
}

/// How a card's `CardData` changes at `upgrade_level >= 1`, mirroring STS2's
/// `CardModel.OnUpgrade()`. Most cards just nudge one or two numbers
/// (`DealDamage`/`GainBlock` amounts, extra status applications, or the
/// card's cost) — the common fields below cover those. A handful of cards
/// change *which* effects run on upgrade (e.g. True Grit lets the player
/// choose which card to Exhaust instead of picking randomly); for those,
/// `effects_override` replaces `effects` wholesale instead of nudging values.
#[derive(Clone, Default)]
pub(crate) struct UpgradeDelta {
    /// Added to every `EffectOp::DealDamage(n)`/`DealDamageToAllEnemies(n)`
    /// in `effects`.
    pub(crate) damage_delta: i32,
    /// Added to every `EffectOp::GainBlock(n)` in `effects`.
    pub(crate) block_delta: i32,
    /// Added to every `EffectOp::GainEnergy(n)` in `effects` (e.g.
    /// Bloodletting+: 2 -> 3 energy).
    pub(crate) energy_delta: i32,
    /// Added to every `EffectOp::DrawCards(n)` in `effects` (e.g.
    /// BurningPact+: 2 -> 3 cards).
    pub(crate) draw_delta: i32,
    /// Added to `base`/`per_unit` of every `EffectOp::DealDamageScaled` in
    /// `effects` (e.g. AshenStrike+/Bully+'s `per_unit`, Conflagration+'s
    /// `base` and `per_unit`).
    pub(crate) scaled_base_delta: i32,
    pub(crate) scaled_per_unit_delta: i32,
    /// Extra `ApplyStatusToTarget` ops appended after the base effects (e.g.
    /// Bash+'s third Vulnerable stack).
    pub(crate) extra_status_applications: Vec<Status>,
    /// Added to `CardData.cost` (e.g. Barricade+: -1).
    pub(crate) cost_delta: i32,
    pub(crate) keywords_added: HashSet<CardKeyword>,
    pub(crate) keywords_removed: HashSet<CardKeyword>,
    /// When set, replaces `CardData.effects` entirely instead of applying
    /// `damage_delta`/`block_delta`/`extra_status_applications`.
    pub(crate) effects_override: Option<Vec<EffectOp>>,
}

impl UpgradeDelta {
    fn apply(&self, mut data: CardData) -> CardData {
        data.cost += self.cost_delta;
        for keyword in &self.keywords_removed {
            data.keywords.remove(keyword);
        }
        for keyword in &self.keywords_added {
            data.keywords.insert(keyword.clone());
        }
        if let Some(effects) = &self.effects_override {
            data.effects = effects.clone();
            return data;
        }
        for effect in &mut data.effects {
            match effect {
                EffectOp::DealDamage(amount) => *amount += self.damage_delta,
                EffectOp::DealDamageToAllEnemies(amount) => *amount += self.damage_delta,
                EffectOp::GainBlock(amount) => *amount += self.block_delta,
                EffectOp::GainEnergy(amount) => *amount += self.energy_delta,
                EffectOp::DrawCards(amount) => {
                    *amount = (*amount as i32 + self.draw_delta) as usize
                }
                EffectOp::DealDamageScaled { base, per_unit, .. } => {
                    *base += self.scaled_base_delta;
                    *per_unit += self.scaled_per_unit_delta;
                }
                _ => {}
            }
        }
        for status in &self.extra_status_applications {
            data.effects.push(EffectOp::ApplyStatusToTarget(status.clone()));
        }
        data
    }
}

fn upgrade_delta(name: &str) -> Option<UpgradeDelta> {
    match name {
        // Strike+: 6 -> 9 damage.
        "Strike" => Some(UpgradeDelta { damage_delta: 3, ..Default::default() }),
        // Defend+: 5 -> 8 block.
        "Defend" => Some(UpgradeDelta { block_delta: 3, ..Default::default() }),
        // Bash+: 8 -> 10 damage, 2 -> 3 Vulnerable stacks.
        "Bash" => Some(UpgradeDelta {
            damage_delta: 2,
            extra_status_applications: vec![Status::Vulnerable],
            ..Default::default()
        }),
        // Barricade+: cost 3 -> 2.
        "Barricade" => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Aggression+: gains the Innate keyword.
        "Aggression" => Some(UpgradeDelta {
            keywords_added: HashSet::from([CardKeyword::Innate]),
            ..Default::default()
        }),
        // AshenStrike+: 6 + 3*exhaust -> 6 + 4*exhaust.
        "AshenStrike" => Some(UpgradeDelta { scaled_per_unit_delta: 1, ..Default::default() }),
        // Bloodletting+: gain 2 energy -> gain 3 energy.
        "Bloodletting" => Some(UpgradeDelta { energy_delta: 1, ..Default::default() }),
        // BloodWall+: 16 -> 20 block.
        "BloodWall" => Some(UpgradeDelta { block_delta: 4, ..Default::default() }),
        // Bludgeon+: 32 -> 42 damage.
        "Bludgeon" => Some(UpgradeDelta { damage_delta: 10, ..Default::default() }),
        // BodySlam+: cost 1 -> 0.
        "BodySlam" => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Break+: 20 -> 30 damage, 5 -> 7 Vulnerable stacks.
        "Break" => Some(UpgradeDelta {
            damage_delta: 10,
            extra_status_applications: vec![Status::Vulnerable, Status::Vulnerable],
            ..Default::default()
        }),
        // Breakthrough+: 9 -> 13 damage to all enemies.
        "Breakthrough" => Some(UpgradeDelta { damage_delta: 4, ..Default::default() }),
        // Bully+: 4 + 2*vulnerable -> 4 + 3*vulnerable.
        "Bully" => Some(UpgradeDelta { scaled_per_unit_delta: 1, ..Default::default() }),
        // BurningPact+: draw 2 -> draw 3.
        "BurningPact" => Some(UpgradeDelta { draw_delta: 1, ..Default::default() }),
        // Cinder+: 18 -> 24 damage.
        "Cinder" => Some(UpgradeDelta { damage_delta: 6, ..Default::default() }),
        // Colossus+: 5 -> 8 block.
        "Colossus" => Some(UpgradeDelta { block_delta: 3, ..Default::default() }),
        // Conflagration+: 8 + 2*attacks -> 9 + 3*attacks.
        "Conflagration" => Some(UpgradeDelta {
            scaled_base_delta: 1,
            scaled_per_unit_delta: 1,
            ..Default::default()
        }),
        _ => None,
    }
}

/// Resolves `name`'s base `CardData` and, at `upgrade_level >= 1`, applies its
/// `UpgradeDelta` (if any — cards without a declared delta are unaffected by
/// upgrade level).
pub(crate) fn card_data(name: &str, upgrade_level: u8) -> Option<CardData> {
    let data = card_data_base(name)?;
    if upgrade_level >= 1 {
        if let Some(delta) = upgrade_delta(name) {
            return Some(delta.apply(data));
        }
    }
    Some(data)
}

fn card_data_base(name: &str) -> Option<CardData> {
    match name {
        "Strike" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(6)],
            keywords: HashSet::new(),
        }),
        "Defend" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlock(5)],
            keywords: HashSet::new(),
        }),
        // Per the Slay the Spire wiki, base Bash deals 8 damage and applies
        // 2 Vulnerable stacks (not 1).
        "Bash" => Some(CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(8),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::new(),
        }),
        "Iron Wave" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(5), EffectOp::GainBlock(5)],
            keywords: HashSet::new(),
        }),
        "Inflame" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))],
            keywords: HashSet::new(),
        }),
        // 3 hits of 3 damage each to a random enemy (always the same target
        // in single-enemy fights). Targeted so SelectTarget resolves first.
        "Sword Boomerang" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(3),
                EffectOp::DealDamage(3),
                EffectOp::DealDamage(3),
            ],
            keywords: HashSet::new(),
        }),
        // Hits all enemies for 4 and applies 1 Vulnerable to each.
        // Not targeted — resolves immediately against all enemies (single
        // enemy = the monster).
        "Thunderclap" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(4),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::new(),
        }),
        // Installs the Rage status: gain 2 Block each time you play an Attack.
        "Rage" => Some(CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Rage)],
            keywords: HashSet::new(),
        }),
        // Installs the Demon Form status: gain 2 Strength at the start of
        // each turn (including the turn it's played, per the wiki — but the
        // first TurnStart fires on the *next* turn since this turn is
        // already in progress).
        "DemonForm" => Some(CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::DemonForm)],
            keywords: HashSet::new(),
        }),
        // Installs the Crimson Mantle status: at the start of each turn,
        // gain 8 Block and lose HP equal to a counter that starts at 1 and
        // increases by 1 each turn.
        "CrimsonMantle" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::CrimsonMantle(1))],
            keywords: HashSet::new(),
        }),
        // Installs the Inferno status: at the start of each turn, lose 1 HP
        // (unblockable); whenever the holder loses HP on their turn, deal 6
        // damage to all enemies.
        "Inferno" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Inferno)],
            keywords: HashSet::new(),
        }),
        // Installs the Aggression status: at the start of each turn, return
        // a random Attack from the discard pile to hand.
        "Aggression" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Aggression)],
            keywords: HashSet::new(),
        }),
        // Installs the Dark Embrace status: whenever a card is Exhausted,
        // draw 1 card.
        "DarkEmbrace" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::DarkEmbrace)],
            keywords: HashSet::new(),
        }),
        // Installs the Feel No Pain status: whenever a card is Exhausted,
        // gain 3 Block.
        "FeelNoPain" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::FeelNoPain)],
            keywords: HashSet::new(),
        }),
        // Installs the Barricade status: Block is no longer removed at the
        // start of your turn.
        "Barricade" => Some(CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Barricade)],
            keywords: HashSet::new(),
        }),
        // Installs the Juggernaut status: whenever you gain Block, deal 5
        // damage to a random enemy.
        "Juggernaut" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Juggernaut(5))],
            keywords: HashSet::new(),
        }),
        // Gain 12 Block. Whenever you are attacked this turn, deal 4 damage
        // back to the attacker.
        "FlameBarrier" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::GainBlock(12),
                EffectOp::ApplyStatusToSelf(Status::FlameBarrier),
            ],
            keywords: HashSet::new(),
        }),
        // Colossus costs 1, gains 5 Block, and installs the Colossus status
        // for this turn only: incoming damage from attackers with
        // Vulnerable is halved.
        "Colossus" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::GainBlock(5),
                EffectOp::ApplyStatusToSelf(Status::Colossus(1)),
            ],
            keywords: HashSet::new(),
        }),
        // Installs the Corruption status: Skills cost 0 and Exhaust when
        // played.
        "Corruption" => Some(CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Corruption)],
            keywords: HashSet::new(),
        }),
        // Installs the Cruelty status: damage dealt to Vulnerable targets is
        // amplified by 1.75x instead of 1.5x.
        "Cruelty" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Cruelty)],
            keywords: HashSet::new(),
        }),
        // Deal 15 damage. Apply -10 Strength to the target for the rest of
        // combat.
        "Mangle" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(15),
                EffectOp::ApplyStatusToTarget(Status::Strength(-10)),
            ],
            keywords: HashSet::new(),
        }),
        // Installs the One Two Punch status: the next Attack played this
        // turn is played a second time.
        "OneTwoPunch" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::OneTwoPunch)],
            keywords: HashSet::new(),
        }),
        "Pommel Strike" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(9), EffectOp::DrawCards(1)],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Bloodletting costs 0, deals 3 unblockable damage to
        // the player, and grants 2 Energy.
        "Bloodletting" => Some(CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::LoseHp(3), EffectOp::GainEnergy(2)],
            keywords: HashSet::new(),
        }),
        // Per the wiki, BloodWall costs 2, deals 2 unblockable damage to the
        // player, and grants 16 Block.
        "BloodWall" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::LoseHp(2), EffectOp::GainBlock(16)],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Hemokinesis costs 1, deals 2 unblockable damage to
        // the player, and deals 15 damage to a chosen enemy.
        "Hemokinesis" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::LoseHp(2), EffectOp::DealDamage(15)],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Offering costs 0, deals 6 unblockable damage to the
        // player, grants 2 Energy, draws 3 cards, and Exhausts.
        "Offering" => Some(CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::LoseHp(6),
                EffectOp::GainEnergy(2),
                EffectOp::DrawCards(3),
            ],
            keywords: HashSet::from([CardKeyword::Exhaust]),
        }),
        // Per the wiki, Tremble costs 1, applies 3 Vulnerable to a chosen
        // enemy, and Exhausts.
        "Tremble" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::from([CardKeyword::Exhaust]),
        }),
        // Per the wiki, Impervious costs 2, grants 30 Block, and Exhausts.
        "Impervious" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlock(30)],
            keywords: HashSet::from([CardKeyword::Exhaust]),
        }),
        // Per the wiki, NotYet costs 2, heals 10 HP (capped at max HP), and
        // Exhausts.
        "NotYet" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::Heal(10)],
            keywords: HashSet::from([CardKeyword::Exhaust]),
        }),
        // The slime monsters' Goop/StickyShot moves stick this into the
        // player's discard pile. Per the wiki: 1 energy, draws 1 card,
        // exhausts on play.
        "Slimed" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Status,
            effects: vec![EffectOp::DrawCards(1)],
            keywords: HashSet::new(),
        }),
        // Vantom's Dismember sticks these into the player's discard pile.
        // Per the wiki, Wound is identical to "Slimed": 1 energy, draws 1
        // card, no exhaust.
        "Wound" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Status,
            effects: vec![EffectOp::DrawCards(1)],
            keywords: HashSet::new(),
        }),
        // Per the wiki: Dazed is Unplayable and Ethereal, and does nothing —
        // a junk card the Defect's orbs and some monsters stick into the
        // player's hand/draw pile to clog it up.
        "Dazed" => Some(CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Status,
            effects: vec![],
            keywords: HashSet::from([CardKeyword::Ethereal, CardKeyword::Unplayable]),
        }),
        // Per the decompiled source, Cinder costs 2, deals 18 damage to a
        // chosen enemy, then exhausts a random card from hand.
        "Cinder" => Some(CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(18),
                EffectOp::ExhaustRandomFromHand(HandFilter::Any),
            ],
            keywords: HashSet::new(),
        }),
        // Per the decompiled source, base (non-upgraded) TrueGrit costs 1,
        // gains 7 block, and exhausts a random card from hand (upgraded lets
        // the player choose; we model only the base random behavior).
        "TrueGrit" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::GainBlock(7),
                EffectOp::ExhaustRandomFromHand(HandFilter::Any),
            ],
            keywords: HashSet::new(),
        }),
        // Per the decompiled source, BurningPact costs 1; the player chooses
        // 1 card from hand to exhaust (modeled as random — see TrueGrit) and
        // draws 2 cards.
        "BurningPact" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::ExhaustRandomFromHand(HandFilter::Any),
                EffectOp::DrawCards(2),
            ],
            keywords: HashSet::new(),
        }),
        // Per the decompiled source, Thrash costs 1, deals 4 damage twice (8
        // total) to a chosen enemy, then exhausts a random Attack card from
        // hand. The decompiled source also has Thrash permanently absorb the
        // exhausted card's damage into its own — per-card-instance mutable
        // state that our string-based hand representation can't model, so
        // that part is skipped.
        "Thrash" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(4),
                EffectOp::DealDamage(4),
                EffectOp::ExhaustRandomFromHand(HandFilter::Attack),
            ],
            keywords: HashSet::new(),
        }),
        // Per the decompiled source, SecondWind costs 1; for each non-Attack
        // card in hand, exhaust it and gain 5 block (total = 5 * count).
        "SecondWind" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::ExhaustAllFromHand {
                filter: HandFilter::NonAttack,
                gain_block_per_card: 5,
            }],
            keywords: HashSet::new(),
        }),
        // Per the decompiled source, Headbutt costs 1, deals 9 damage to a
        // chosen enemy, then the player picks a card from the discard pile to
        // put on top of the draw pile (modeled as random — see
        // TrueGrit/BurningPact). Headbutt itself is already in the discard
        // pile by the time this resolves (cards move there immediately on
        // play), so it's a valid candidate and can retrieve itself — matching
        // real Slay the Spire.
        "Headbutt" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(9), EffectOp::PutRandomDiscardOnTopOfDraw],
            keywords: HashSet::new(),
        }),
        // Per the decompiled source, FiendFire costs 2 and Exhausts. It deals
        // 7 damage to a chosen enemy once per card remaining in hand
        // (counted before the rest of the hand is exhausted — hence the
        // damage op runs first), then exhausts every other card in hand.
        "FiendFire" => Some(CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamageScaled {
                    base: 0,
                    per_unit: 7,
                    source: ScaleSource::HandSize,
                },
                EffectOp::ExhaustAllFromHand {
                    filter: HandFilter::Any,
                    gain_block_per_card: 0,
                },
            ],
            keywords: HashSet::from([CardKeyword::Exhaust]),
        }),
        // Per the decompiled source, InfernalBlade costs 1 and Exhausts. It
        // adds a random Attack card to hand from the Ironclad's full
        // unlocked card pool, "free this turn" (cost override). We model the
        // pool as a hardcoded list of currently-implemented Attack cards and
        // don't model the cost override — both documented simplifications.
        "InfernalBlade" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::AddRandomCardToHand(vec![
                "Strike".to_string(),
                "Iron Wave".to_string(),
                "Sword Boomerang".to_string(),
                "Thunderclap".to_string(),
                "Pommel Strike".to_string(),
                "Hemokinesis".to_string(),
                "Cinder".to_string(),
                "Thrash".to_string(),
                "Headbutt".to_string(),
                "FiendFire".to_string(),
            ])],
            keywords: HashSet::from([CardKeyword::Exhaust]),
        }),
        // Per the wiki, Bludgeon costs 3 and deals 32 damage.
        "Bludgeon" => Some(CardData {
            cost: 3,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(32)],
            keywords: HashSet::new(),
        }),
        // Per the wiki, TwinStrike costs 1 and deals 5 damage twice (10
        // total).
        "TwinStrike" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(5), EffectOp::DealDamage(5)],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Break costs 1, deals 20 damage, and applies 5
        // Vulnerable to the chosen enemy.
        "Break" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(20),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::new(),
        }),
        // Per the wiki, ShrugItOff costs 1, gains 8 block, and draws 1 card.
        "ShrugItOff" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlock(8), EffectOp::DrawCards(1)],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Taunt costs 1, gains 7 block, and applies Vulnerable
        // to the chosen enemy.
        "Taunt" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::GainBlock(7),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Uppercut costs 2, deals 13 damage, and applies 1 Weak
        // and 1 Vulnerable to the chosen enemy.
        "Uppercut" => Some(CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(13),
                EffectOp::ApplyStatusToTarget(Status::Weak),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::new(),
        }),
        // Per the wiki, BodySlam costs 1 and deals damage equal to the
        // player's current Block.
        "BodySlam" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 0,
                per_unit: 1,
                source: ScaleSource::CurrentBlock,
            }],
            keywords: HashSet::new(),
        }),
        // Per the wiki, PerfectedStrike costs 2 and deals 6 damage plus 2 for
        // every card named "Strike" in the player's deck (counted across all
        // piles, including itself if it were named "Strike" — it isn't).
        "PerfectedStrike" => Some(CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 6,
                per_unit: 2,
                source: ScaleSource::StrikeCardsInDeck,
            }],
            keywords: HashSet::new(),
        }),
        // Per the wiki, AshenStrike costs 1 and deals 6 damage plus 3 for
        // every card in the player's exhaust pile.
        "AshenStrike" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 6,
                per_unit: 3,
                source: ScaleSource::ExhaustPileSize,
            }],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Bully costs 0 and deals 4 damage plus 2 for every
        // stack of Vulnerable on the target.
        "Bully" => Some(CardData {
            cost: 0,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 4,
                per_unit: 2,
                source: ScaleSource::VulnerableStacksOnTarget,
            }],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Conflagration costs 1 and deals 8 damage to ALL
        // enemies, plus 2 for each Attack played earlier this turn.
        "Conflagration" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 8,
                per_unit: 2,
                source: ScaleSource::AttacksPlayedThisTurn,
            }],
            keywords: HashSet::new(),
        }),
        // Per the wiki, TearAsunder costs 2 and deals 5 damage, hitting one
        // extra time for every time the player has been damaged this combat.
        "TearAsunder" => Some(CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageRepeated {
                amount: 5,
                hits_base: 1,
                hits_per_unit: 1,
                hits_source: ScaleSource::DamageTakenThisCombat,
            }],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Spite costs 0 and deals 5 damage, hitting twice if the
        // player has lost HP this turn.
        "Spite" => Some(CardData {
            cost: 0,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageRepeated {
                amount: 5,
                hits_base: 1,
                hits_per_unit: 1,
                hits_source: ScaleSource::HpLostThisTurn,
            }],
            keywords: HashSet::new(),
        }),
        // Per HOL-16, Dismantle costs 1 and deals 8 damage, hitting twice if
        // the target has Vulnerable.
        "Dismantle" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageRepeated {
                amount: 8,
                hits_base: 1,
                hits_per_unit: 1,
                hits_source: ScaleSource::TargetHasVulnerable,
            }],
            keywords: HashSet::new(),
        }),
        // Per the wiki, MoltenFist costs 1, doubles the target's existing
        // Vulnerable stacks, deals 10 damage, and Exhausts.
        "MoltenFist" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DoubleVulnerableOnTarget, EffectOp::DealDamage(10)],
            keywords: HashSet::from([CardKeyword::Exhaust]),
        }),
        // Per HOL-16, Dominate costs 1, applies Vulnerable to the target,
        // then gains Strength equal to the target's resulting Vulnerable
        // stack count, and exhausts.
        "Dominate" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::GainStrengthEqualToTargetVulnerable,
            ],
            keywords: HashSet::from([CardKeyword::Exhaust]),
        }),
        // Per the wiki, Breakthrough costs 1, makes the player Lose 1 HP,
        // and deals 9 damage to ALL enemies (non-targeted, like Thunderclap).
        "Breakthrough" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![EffectOp::LoseHp(1), EffectOp::DealDamageToAllEnemies(9)],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Setup Strike costs 1, deals 7 damage, and grants the
        // player 2 Strength for this turn only.
        "Setup Strike" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(7), EffectOp::ApplyStatusToSelf(Status::StrengthThisTurn(2))],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Unrelenting costs 1, deals 12 damage, and makes the
        // next Attack the player plays cost 0 Energy.
        "Unrelenting" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(12), EffectOp::ApplyStatusToSelf(Status::FreeAttack)],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Evil Eye costs 1, grants 8 Block, and grants another
        // 8 Block (16 total) if the player has Exhausted a card this turn.
        "Evil Eye" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlockScaled {
                base: 8,
                per_unit: 8,
                source: ScaleSource::ExhaustedCardThisTurn,
            }],
            keywords: HashSet::new(),
        }),
        // Per the wiki, Forgotten Ritual costs 0, Exhausts, and grants 3
        // Energy if the player has Exhausted a card this turn — its own
        // Exhaust (which resolves before its effects) satisfies that
        // condition, so playing it always nets +3 Energy.
        "Forgotten Ritual" => Some(CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainEnergyScaled {
                base: 0,
                per_unit: 3,
                source: ScaleSource::ExhaustedCardThisTurn,
            }],
            keywords: HashSet::from([CardKeyword::Exhaust]),
        }),
        _ => None,
    }
}

// ============================================================================
// Reference: STS2 Ironclad cards not yet implemented in `card_data`.
//
// Exact card text (from the STS2 wiki/screenshots) for cards that appeared
// in the full Ironclad card-pool dump but have no `card_data` entry above.
// Kept here so future waves (notably HOL-23's "Phase 4b" triage of Rupture,
// Vicious, Stampede, etc.) don't need to re-fetch the source material.
//
// -- Common --
// Anger (Attack): Deal 6 damage. Add a copy of this card into your Discard
//   Pile.
// Armaments (Skill): Gain 5 Block. Upgrade a card in your Hand.
// Breakthrough (Attack): Lose 1 HP. Deal 9 damage to ALL enemies.
// Havoc (Skill): Play the top card of your Draw Pile and Exhaust it.
// Iron Wave (Attack): Gain 5 Block. Deal 5 damage.
// Pommel Strike (Attack): Deal 9 damage. Draw 1 card.
// Setup Strike (Attack): Deal 7 damage. Gain 2 Strength this turn.
// Sword Boomerang (Attack): Deal 3 damage to a random enemy 3 times.
//
// -- Uncommon --
// Battle Trance (Skill): Draw 3 cards. You cannot draw additional cards this
//   turn.
// Demonic Shield (Skill): Lose 1 HP. Give another player Block equal to your
//   Block. Exhaust. [multiplayer-only — likely N/A for single-player sim]
// Drum of Battle (Power): Draw 2 cards. At the start of your turn, Exhaust
//   the top card of your Draw Pile.
// Evil Eye (Skill): Gain 8 Block. Gain another 8 Block if you have Exhausted
//   a card this turn.
// Expect a Fight (Skill): Gain Energy for each Attack in your Hand. You
//   cannot gain additional Energy this turn.
// Fight Me! (Attack): Deal 5 damage twice. Gain 3 Strength. The enemy gains
//   1 Strength.
// Forgotten Ritual (Skill): If you Exhausted a card this turn, gain 3
//   Energy. Exhaust.
// Howl from Beyond (Attack): Deal 16 damage to ALL enemies. At the start of
//   your turn, if this is in your Exhaust Pile, play it.
// Juggling (Power): Add a copy of the third Attack you play each turn into
//   your Hand.
// Pillage (Attack): Deal 6 damage. Draw cards until you draw a non-Attack
//   card.
// Rampage (Attack): Deal 9 damage. Increase this card's damage by 5 this
//   combat.
// Rupture (Power): Whenever you lose HP on your turn, gain 1 Strength.
// Stampede (Power): At the end of your turn, 1 random Attack in your Hand is
//   played against a random enemy.
// Stomp (Attack): Deal 12 damage to ALL enemies. Costs 1 less Energy for
//   each Attack played this turn.
// Stone Armor (Power): Gain 4 Plating.
// Unrelenting (Attack): Deal 12 damage. The next Attack you play costs 0
//   Energy.
// Vicious (Power): Whenever you apply Vulnerable, draw 1 card.
// Whirlwind (Attack): Deal 5 damage to ALL enemies X times.
//
// -- Rare --
// Brand (Skill): Lose 1 HP. Exhaust 1 card. Gain 1 Strength.
// Cascade (Skill): Play the top X cards of your Draw Pile.
// Feed (Attack): Deal 10 damage. If Fatal, raise your Max HP by 3. Exhaust.
// Hellraiser (Power): Whenever you draw a card containing "Strike", it is
//   played against a random enemy.
// Pact's End (Attack): Can only be played if you have 3 or more cards in
//   your Exhaust Pile. Deal 17 damage to ALL enemies.
// Primal Force (Skill): Transform all Attacks in your Hand into Giant Rock.
// Pyre (Power): Gain Energy at the start of each turn.
// Stoke (Skill): Exhaust your Hand. Add 1 random card into your Hand for
//   each card Exhausted.
// Tank (Power): Take double damage from enemies. Allies take half damage
//   from enemies. [multiplayer-only — likely N/A for single-player sim]
// Unmovable (Power): The first time you gain Block from a card each turn,
//   double the amount gained.
// ============================================================================
