use crate::ids::CardId;
use crate::engine::{EffectOp, HandFilter, ScaleSource, Status};
use std::collections::HashSet;

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum CardType {
    Attack,
    Skill,
    Power,
    // Non-interactive "junk" cards monsters stick into the player's deck
    // (e.g. Slimed) — like Powers, they exhaust on play rather than
    // returning to discard.
    Status,
}

/// Rarity tier of a card, matching STS2's `EntityRarity` enum. Used by
/// run-level reward logic to weight card-reward draws.
#[derive(Clone, PartialEq)]
pub(crate) enum CardRarity {
    Starter,
    Common,
    Uncommon,
    Rare,
    Special,
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
    pub(crate) rarity: CardRarity,
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

fn upgrade_delta(id: CardId) -> Option<UpgradeDelta> {
    match id {
        // Strike+: 6 -> 9 damage.
        CardId::StrikeIronclad => Some(UpgradeDelta { damage_delta: 3, ..Default::default() }),
        // Defend+: 5 -> 8 block.
        CardId::DefendIronclad => Some(UpgradeDelta { block_delta: 3, ..Default::default() }),
        // Bash+: 8 -> 10 damage, 2 -> 3 Vulnerable stacks.
        CardId::Bash => Some(UpgradeDelta {
            damage_delta: 2,
            extra_status_applications: vec![Status::Vulnerable],
            ..Default::default()
        }),
        // Barricade+: cost 3 -> 2.
        CardId::Barricade => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Aggression+: gains the Innate keyword.
        CardId::Aggression => Some(UpgradeDelta {
            keywords_added: HashSet::from([CardKeyword::Innate]),
            ..Default::default()
        }),
        // AshenStrike+: 6 + 3*exhaust -> 6 + 4*exhaust.
        CardId::AshenStrike => Some(UpgradeDelta { scaled_per_unit_delta: 1, ..Default::default() }),
        // Bloodletting+: gain 2 energy -> gain 3 energy.
        CardId::Bloodletting => Some(UpgradeDelta { energy_delta: 1, ..Default::default() }),
        // BloodWall+: 16 -> 20 block.
        CardId::BloodWall => Some(UpgradeDelta { block_delta: 4, ..Default::default() }),
        // Bludgeon+: 32 -> 42 damage.
        CardId::Bludgeon => Some(UpgradeDelta { damage_delta: 10, ..Default::default() }),
        // BodySlam+: cost 1 -> 0.
        CardId::BodySlam => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Break+: 20 -> 30 damage, 5 -> 7 Vulnerable stacks.
        CardId::Break => Some(UpgradeDelta {
            damage_delta: 10,
            extra_status_applications: vec![Status::Vulnerable, Status::Vulnerable],
            ..Default::default()
        }),
        // Breakthrough+: 9 -> 13 damage to all enemies.
        CardId::Breakthrough => Some(UpgradeDelta { damage_delta: 4, ..Default::default() }),
        // Bully+: 4 + 2*vulnerable -> 4 + 3*vulnerable.
        CardId::Bully => Some(UpgradeDelta { scaled_per_unit_delta: 1, ..Default::default() }),
        // BurningPact+: draw 2 -> draw 3.
        CardId::BurningPact => Some(UpgradeDelta { draw_delta: 1, ..Default::default() }),
        // Cinder+: 18 -> 24 damage.
        CardId::Cinder => Some(UpgradeDelta { damage_delta: 6, ..Default::default() }),
        // Colossus+: 5 -> 8 block.
        CardId::Colossus => Some(UpgradeDelta { block_delta: 3, ..Default::default() }),
        // Conflagration+: 8 + 2*attacks -> 9 + 3*attacks.
        CardId::Conflagration => Some(UpgradeDelta {
            scaled_base_delta: 1,
            scaled_per_unit_delta: 1,
            ..Default::default()
        }),
        // FightMe+: damage 5→6 per hit, self Strength 3→4.
        CardId::FightMe => Some(UpgradeDelta {
            effects_override: Some(vec![
                EffectOp::DealDamage(6),
                EffectOp::DealDamage(6),
                EffectOp::ApplyStatusToSelf(Status::Strength(4)),
                EffectOp::ApplyStatusToTarget(Status::Strength(1)),
            ]),
            ..Default::default()
        }),
        // Juggling+: upgrade only adds Innate (deferred).
        CardId::Juggling => Some(UpgradeDelta { ..Default::default() }),
        // StoneArmor+: Plating +2 → 6.
        CardId::StoneArmor => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::ApplyStatusToSelf(Status::Plating(6))]),
            ..Default::default()
        }),
        // Unmovable+: cost 2 → 1.
        CardId::Unmovable => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Vicious+: Vicious +1 → 2.
        CardId::Vicious => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::ApplyStatusToSelf(Status::Vicious(2))]),
            ..Default::default()
        }),
        // PactsEnd+: 17 → 23 AoE damage.
        CardId::PactsEnd => Some(UpgradeDelta { damage_delta: 6, ..Default::default() }),
        // HowlFromBeyond+: 16 → 21 AoE damage.
        CardId::HowlFromBeyond => Some(UpgradeDelta { damage_delta: 5, ..Default::default() }),
        // Iron Wave+: 5 damage → 7 damage, 5 block → 7 block.
        CardId::IronWave => Some(UpgradeDelta { damage_delta: 2, block_delta: 2, ..Default::default() }),
        // Inflame+: 2 Strength → 3 Strength.
        CardId::Inflame => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(3))]),
            ..Default::default()
        }),
        // Sword Boomerang+: 3 hits of 3 → 3 hits of 4.
        CardId::SwordBoomerang => Some(UpgradeDelta { damage_delta: 1, ..Default::default() }),
        // Thunderclap+: 4 damage → 7 damage.
        CardId::Thunderclap => Some(UpgradeDelta { damage_delta: 3, ..Default::default() }),
        // Rage+: cost 0 → 0 (no stat change — applies the same Rage status).
        CardId::Rage => Some(UpgradeDelta { ..Default::default() }),
        // DemonForm+: cost 3 → 2.
        CardId::DemonForm => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // CrimsonMantle+: cost 2 → 1.
        CardId::CrimsonMantle => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Inferno+ (Power): cost 1 → 0.
        CardId::Inferno => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // DarkEmbrace+: cost 2 → 1.
        CardId::DarkEmbrace => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // FeelNoPain+: cost 1 → 0.
        CardId::FeelNoPain => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Juggernaut+: Juggernaut 5 → 7 (damage on block gain).
        CardId::Juggernaut => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::ApplyStatusToSelf(Status::Juggernaut(7))]),
            ..Default::default()
        }),
        // FlameBarrier+: 12 block → 16 block.
        CardId::FlameBarrier => Some(UpgradeDelta { block_delta: 4, ..Default::default() }),
        // Corruption+: cost 3 → 2.
        CardId::Corruption => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Cruelty+: cost 1 → 0.
        CardId::Cruelty => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Mangle+: 15 damage → 20 damage.
        CardId::Mangle => Some(UpgradeDelta { damage_delta: 5, ..Default::default() }),
        // OneTwoPunch+: cost 1 → 0.
        CardId::OneTwoPunch => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // Pommel Strike+: 9 damage → 10 damage.
        CardId::PommelStrike => Some(UpgradeDelta { damage_delta: 1, ..Default::default() }),
        // Hemokinesis+: 15 damage → 20 damage.
        CardId::Hemokinesis => Some(UpgradeDelta { damage_delta: 5, ..Default::default() }),
        // Offering+: draw 3 → draw 5.
        CardId::Offering => Some(UpgradeDelta { draw_delta: 2, ..Default::default() }),
        // Tremble+: 3 Vulnerable → 4 Vulnerable.
        CardId::Tremble => Some(UpgradeDelta {
            extra_status_applications: vec![Status::Vulnerable],
            ..Default::default()
        }),
        // Impervious+: 30 block → 40 block.
        CardId::Impervious => Some(UpgradeDelta { block_delta: 10, ..Default::default() }),
        // NotYet+: heal 10 → heal 14.
        CardId::NotYet => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::Heal(14)]),
            ..Default::default()
        }),
        // TrueGrit+: 7 block → 9 block.
        CardId::TrueGrit => Some(UpgradeDelta { block_delta: 2, ..Default::default() }),
        // Thrash+: 4+4=8 damage → 5+5=10 damage.
        CardId::Thrash => Some(UpgradeDelta { damage_delta: 1, ..Default::default() }),
        // SecondWind+: 5 block per card exhausted → 7 block per card.
        CardId::SecondWind => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::ExhaustAllFromHand {
                filter: HandFilter::NonAttack,
                gain_block_per_card: 7,
            }]),
            ..Default::default()
        }),
        // Headbutt+: 9 damage → 12 damage.
        CardId::Headbutt => Some(UpgradeDelta { damage_delta: 3, ..Default::default() }),
        // FiendFire+: 7 damage per card in hand → 10.
        CardId::FiendFire => Some(UpgradeDelta {
            effects_override: Some(vec![
                EffectOp::DealDamageScaled {
                    base: 0,
                    per_unit: 10,
                    source: ScaleSource::HandSize,
                },
                EffectOp::ExhaustAllFromHand {
                    filter: HandFilter::Any,
                    gain_block_per_card: 0,
                },
            ]),
            ..Default::default()
        }),
        // InfernalBlade+: cost 1 → 0.
        CardId::InfernalBlade => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // TwinStrike+: 5×2=10 damage → 7×2=14 damage.
        CardId::TwinStrike => Some(UpgradeDelta { damage_delta: 2, ..Default::default() }),
        // ShrugItOff+: 8 block → 11 block.
        CardId::ShrugItOff => Some(UpgradeDelta { block_delta: 3, ..Default::default() }),
        // Taunt+: 7 block → 10 block.
        CardId::Taunt => Some(UpgradeDelta { block_delta: 3, ..Default::default() }),
        // Uppercut+: 13 damage → 17 damage.
        CardId::Uppercut => Some(UpgradeDelta { damage_delta: 4, ..Default::default() }),
        // PerfectedStrike+: 6 + 2*strikes → 6 + 3*strikes.
        CardId::PerfectedStrike => Some(UpgradeDelta { scaled_per_unit_delta: 1, ..Default::default() }),
        // TearAsunder+: 5 damage per hit → 7 damage per hit.
        CardId::TearAsunder => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::DealDamageRepeated {
                amount: 7,
                hits_base: 1,
                hits_per_unit: 1,
                hits_source: ScaleSource::DamageTakenThisCombat,
            }]),
            ..Default::default()
        }),
        // Spite+: 5 damage per hit → 8 damage per hit.
        CardId::Spite => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::DealDamageRepeated {
                amount: 8,
                hits_base: 1,
                hits_per_unit: 1,
                hits_source: ScaleSource::HpLostThisTurn,
            }]),
            ..Default::default()
        }),
        // Dismantle+: 8 damage per hit → 11 damage per hit.
        CardId::Dismantle => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::DealDamageRepeated {
                amount: 11,
                hits_base: 1,
                hits_per_unit: 1,
                hits_source: ScaleSource::TargetHasVulnerable,
            }]),
            ..Default::default()
        }),
        // Dominate+: applies 2 Vulnerable instead of 1, then gains Strength.
        CardId::Dominate => Some(UpgradeDelta {
            effects_override: Some(vec![
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::GainStrengthEqualToTargetVulnerable,
            ]),
            ..Default::default()
        }),
        // MoltenFist+: 10 damage → 14 damage.
        CardId::MoltenFist => Some(UpgradeDelta { damage_delta: 4, ..Default::default() }),
        // Setup Strike+: 7 damage → 10 damage.
        CardId::SetupStrike => Some(UpgradeDelta { damage_delta: 3, ..Default::default() }),
        // Unrelenting+: 12 damage → 16 damage.
        CardId::Unrelenting => Some(UpgradeDelta { damage_delta: 4, ..Default::default() }),
        // Evil Eye+: 8 block (up to 16) → 11 block (up to 22).
        CardId::EvilEye => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::GainBlockScaled {
                base: 11,
                per_unit: 11,
                source: ScaleSource::ExhaustedCardThisTurn,
            }]),
            ..Default::default()
        }),
        // Forgotten Ritual+: 3 energy → 4 energy.
        CardId::ForgottenRitual => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::GainEnergyScaled {
                base: 0,
                per_unit: 4,
                source: ScaleSource::ExhaustedCardThisTurn,
            }]),
            ..Default::default()
        }),
        // Pyre+: gains the Innate keyword.
        CardId::Pyre => Some(UpgradeDelta {
            keywords_added: HashSet::from([CardKeyword::Innate]),
            ..Default::default()
        }),
        // Anger+: 6 damage → 8 damage.
        CardId::Anger => Some(UpgradeDelta { damage_delta: 2, ..Default::default() }),
        // DrumOfBattle+: draw 2 → draw 3.
        CardId::DrumOfBattle => Some(UpgradeDelta { draw_delta: 1, ..Default::default() }),
        // Stomp+: 12 AoE damage → 17 AoE damage.
        CardId::Stomp => Some(UpgradeDelta { damage_delta: 5, ..Default::default() }),
        // Havoc+: cost 1 → 0.
        CardId::Havoc => Some(UpgradeDelta { cost_delta: -1, ..Default::default() }),
        // BattleTrance+: draw 3 → 4.
        CardId::BattleTrance => Some(UpgradeDelta { draw_delta: 1, ..Default::default() }),
        // Whirlwind+: base damage 5 → 8.
        CardId::Whirlwind => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::DealDamageRepeated {
                amount: 8,
                hits_base: 0,
                hits_per_unit: 1,
                hits_source: ScaleSource::EnergyX,
            }]),
            ..Default::default()
        }),
        // Cascade+: play X+1 cards (one extra card).
        CardId::Cascade => Some(UpgradeDelta {
            effects_override: Some(vec![EffectOp::PlayTopOfDeckScaled {
                count_base: 1,
                count_per_unit: 1,
                count_source: ScaleSource::EnergyX,
                exhaust: false,
            }]),
            ..Default::default()
        }),
        _ => None,
    }
}

/// Resolves `name`'s base `CardData` and, at `upgrade_level >= 1`, applies its
/// `UpgradeDelta` (if any — cards without a declared delta are unaffected by
/// upgrade level).
pub(crate) fn card_data(id: CardId, upgrade_level: u8) -> CardData {
    let data = card_data_base(id);
    if upgrade_level >= 1 {
        if let Some(delta) = upgrade_delta(id) {
            return delta.apply(data);
        }
    }
    data
}

fn card_data_base(id: CardId) -> CardData {
    match id {
        CardId::StrikeIronclad => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(6)],
            keywords: HashSet::new(),
            rarity: CardRarity::Starter,
        },
        CardId::DefendIronclad => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlock(5)],
            keywords: HashSet::new(),
            rarity: CardRarity::Starter,
        },
        // Per the Slay the Spire wiki, base Bash deals 8 damage and applies
        // 2 Vulnerable stacks (not 1).
        CardId::Bash => CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(8),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Starter,
        },
        CardId::IronWave => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(5), EffectOp::GainBlock(5)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        CardId::Inflame => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // 3 hits of 3 damage each to a random enemy (always the same target
        // in single-enemy fights). Targeted so SelectTarget resolves first.
        CardId::SwordBoomerang => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(3),
                EffectOp::DealDamage(3),
                EffectOp::DealDamage(3),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Hits all enemies for 4 and applies 1 Vulnerable to each.
        // Not targeted — resolves immediately against all enemies (single
        // enemy = the monster).
        CardId::Thunderclap => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(4),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Installs the Rage status: gain 2 Block each time you play an Attack.
        CardId::Rage => CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Rage)],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Installs the Demon Form status: gain 2 Strength at the start of
        // each turn (including the turn it's played, per the wiki — but the
        // first TurnStart fires on the *next* turn since this turn is
        // already in progress).
        CardId::DemonForm => CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::DemonForm)],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Installs the Crimson Mantle status: at the start of each turn,
        // gain 8 Block and lose HP equal to a counter that starts at 1 and
        // increases by 1 each turn.
        CardId::CrimsonMantle => CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::CrimsonMantle(1))],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Installs the Inferno status: at the start of each turn, lose 1 HP
        // (unblockable); whenever the holder loses HP on their turn, deal 6
        // damage to all enemies.
        CardId::Inferno => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Inferno)],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Installs the Aggression status: at the start of each turn, return
        // a random Attack from the discard pile to hand.
        CardId::Aggression => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Aggression)],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Installs the Dark Embrace status: whenever a card is Exhausted,
        // draw 1 card.
        CardId::DarkEmbrace => CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::DarkEmbrace)],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Installs the Feel No Pain status: whenever a card is Exhausted,
        // gain 3 Block.
        CardId::FeelNoPain => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::FeelNoPain)],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Installs the Barricade status: Block is no longer removed at the
        // start of your turn.
        CardId::Barricade => CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Barricade)],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Installs the Juggernaut status: whenever you gain Block, deal 5
        // damage to a random enemy.
        CardId::Juggernaut => CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Juggernaut(5))],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Gain 12 Block. Whenever you are attacked this turn, deal 4 damage
        // back to the attacker.
        CardId::FlameBarrier => CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::GainBlock(12),
                EffectOp::ApplyStatusToSelf(Status::FlameBarrier),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Colossus costs 1, gains 5 Block, and installs the Colossus status
        // for this turn only: incoming damage from attackers with
        // Vulnerable is halved.
        CardId::Colossus => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::GainBlock(5),
                EffectOp::ApplyStatusToSelf(Status::Colossus(1)),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Installs the Corruption status: Skills cost 0 and Exhaust when
        // played.
        CardId::Corruption => CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Corruption)],
            keywords: HashSet::new(),
            rarity: CardRarity::Special,
        },
        // Installs the Cruelty status: damage dealt to Vulnerable targets is
        // amplified by 1.75x instead of 1.5x.
        CardId::Cruelty => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Cruelty)],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Deal 15 damage. Apply -10 Strength to the target for the rest of
        // combat.
        CardId::Mangle => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(15),
                EffectOp::ApplyStatusToTarget(Status::Strength(-10)),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Installs the One Two Punch status: the next Attack played this
        // turn is played a second time.
        CardId::OneTwoPunch => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::OneTwoPunch)],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        CardId::PommelStrike => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(9), EffectOp::DrawCards(1)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, Bloodletting costs 0, deals 3 unblockable damage to
        // the player, and grants 2 Energy.
        CardId::Bloodletting => CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::LoseHp(3), EffectOp::GainEnergy(2)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, BloodWall costs 2, deals 2 unblockable damage to the
        // player, and grants 16 Block.
        CardId::BloodWall => CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::LoseHp(2), EffectOp::GainBlock(16)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, Hemokinesis costs 1, deals 2 unblockable damage to
        // the player, and deals 15 damage to a chosen enemy.
        CardId::Hemokinesis => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::LoseHp(2), EffectOp::DealDamage(15)],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, Offering costs 0, deals 6 unblockable damage to the
        // player, grants 2 Energy, draws 3 cards, and Exhausts.
        CardId::Offering => CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::LoseHp(6),
                EffectOp::GainEnergy(2),
                EffectOp::DrawCards(3),
            ],
            keywords: HashSet::from([CardKeyword::Exhaust]),
            rarity: CardRarity::Rare,
        },
        // Per the wiki, Tremble costs 1, applies 3 Vulnerable to a chosen
        // enemy, and Exhausts.
        CardId::Tremble => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::from([CardKeyword::Exhaust]),
            rarity: CardRarity::Common,
        },
        // Per the wiki, Impervious costs 2, grants 30 Block, and Exhausts.
        CardId::Impervious => CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlock(30)],
            keywords: HashSet::from([CardKeyword::Exhaust]),
            rarity: CardRarity::Rare,
        },
        // Per the wiki, NotYet costs 2, heals 10 HP (capped at max HP), and
        // Exhausts.
        CardId::NotYet => CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::Heal(10)],
            keywords: HashSet::from([CardKeyword::Exhaust]),
            rarity: CardRarity::Common,
        },
        // The slime monsters' Goop/StickyShot moves stick this into the
        // player's discard pile. Per the wiki: 1 energy, draws 1 card,
        // exhausts on play.
        CardId::Slimed => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Status,
            effects: vec![EffectOp::DrawCards(1)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Vantom's Dismember sticks these into the player's discard pile.
        // Per the wiki, Wound is identical to "Slimed": 1 energy, draws 1
        // card, no exhaust.
        CardId::Wound => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Status,
            effects: vec![EffectOp::DrawCards(1)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Phrog Parasite's Infect sticks these into the player's discard pile.
        // Same stats as Wound: 1 energy, draws 1 card, exhausts on play.
        CardId::Infection => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Status,
            effects: vec![EffectOp::DrawCards(1)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki: Dazed is Unplayable and Ethereal, and does nothing —
        // a junk card the Defect's orbs and some monsters stick into the
        // player's hand/draw pile to clog it up.
        CardId::Dazed => CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Status,
            effects: vec![],
            keywords: HashSet::from([CardKeyword::Ethereal, CardKeyword::Unplayable]),
            rarity: CardRarity::Common,
        },
        // Per the decompiled source, Cinder costs 2, deals 18 damage to a
        // chosen enemy, then exhausts a random card from hand.
        CardId::Cinder => CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(18),
                EffectOp::ExhaustRandomFromHand(HandFilter::Any),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the decompiled source, base (non-upgraded) TrueGrit costs 1,
        // gains 7 block, and exhausts a random card from hand (upgraded lets
        // the player choose; we model only the base random behavior).
        CardId::TrueGrit => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::GainBlock(7),
                EffectOp::ExhaustRandomFromHand(HandFilter::Any),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the decompiled source, BurningPact costs 1; the player chooses
        // 1 card from hand to exhaust (modeled as random — see TrueGrit) and
        // draws 2 cards.
        CardId::BurningPact => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::ExhaustRandomFromHand(HandFilter::Any),
                EffectOp::DrawCards(2),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the decompiled source, Thrash costs 1, deals 4 damage twice (8
        // total) to a chosen enemy, then exhausts a random Attack card from
        // hand. The decompiled source also has Thrash permanently absorb the
        // exhausted card's damage into its own — per-card-instance mutable
        // state that our string-based hand representation can't model, so
        // that part is skipped.
        CardId::Thrash => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(4),
                EffectOp::DealDamage(4),
                EffectOp::ExhaustRandomFromHand(HandFilter::Attack),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Per the decompiled source, SecondWind costs 1; for each non-Attack
        // card in hand, exhaust it and gain 5 block (total = 5 * count).
        CardId::SecondWind => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::ExhaustAllFromHand {
                filter: HandFilter::NonAttack,
                gain_block_per_card: 5,
            }],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the decompiled source, Headbutt costs 1, deals 9 damage to a
        // chosen enemy, then the player picks a card from the discard pile to
        // put on top of the draw pile (modeled as random — see
        // TrueGrit/BurningPact). Headbutt itself is already in the discard
        // pile by the time this resolves (cards move there immediately on
        // play), so it's a valid candidate and can retrieve itself — matching
        // real Slay the Spire.
        CardId::Headbutt => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(9), EffectOp::PutRandomDiscardOnTopOfDraw],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the decompiled source, FiendFire costs 2 and Exhausts. It deals
        // 7 damage to a chosen enemy once per card remaining in hand
        // (counted before the rest of the hand is exhausted — hence the
        // damage op runs first), then exhausts every other card in hand.
        CardId::FiendFire => CardData {
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
            rarity: CardRarity::Rare,
        },
        // Per the decompiled source, InfernalBlade costs 1 and Exhausts. It
        // adds a random Attack card to hand from the Ironclad's full
        // unlocked card pool, "free this turn" (cost override). We model the
        // pool as a hardcoded list of currently-implemented Attack cards and
        // don't model the cost override — both documented simplifications.
        CardId::InfernalBlade => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::AddRandomCardToHand(vec![
                CardId::StrikeIronclad,
                CardId::IronWave,
                CardId::SwordBoomerang,
                CardId::Thunderclap,
                CardId::PommelStrike,
                CardId::Hemokinesis,
                CardId::Cinder,
                CardId::Thrash,
                CardId::Headbutt,
                CardId::FiendFire,
            ])],
            keywords: HashSet::from([CardKeyword::Exhaust]),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, Bludgeon costs 3 and deals 32 damage.
        CardId::Bludgeon => CardData {
            cost: 3,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(32)],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, TwinStrike costs 1 and deals 5 damage twice (10
        // total).
        CardId::TwinStrike => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(5), EffectOp::DealDamage(5)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, Break costs 1, deals 20 damage, and applies 5
        // Vulnerable to the chosen enemy.
        CardId::Break => CardData {
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
            rarity: CardRarity::Special,
        },
        // Per the wiki, ShrugItOff costs 1, gains 8 block, and draws 1 card.
        CardId::ShrugItOff => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlock(8), EffectOp::DrawCards(1)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, Taunt costs 1, gains 7 block, and applies Vulnerable
        // to the chosen enemy.
        CardId::Taunt => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::GainBlock(7),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, Uppercut costs 2, deals 13 damage, and applies 1 Weak
        // and 1 Vulnerable to the chosen enemy.
        CardId::Uppercut => CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(13),
                EffectOp::ApplyStatusToTarget(Status::Weak),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, BodySlam costs 1 and deals damage equal to the
        // player's current Block.
        CardId::BodySlam => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 0,
                per_unit: 1,
                source: ScaleSource::CurrentBlock,
            }],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, PerfectedStrike costs 2 and deals 6 damage plus 2 for
        // every card named "Strike" in the player's deck (counted across all
        // piles, including itself if it were named "Strike" — it isn't).
        CardId::PerfectedStrike => CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 6,
                per_unit: 2,
                source: ScaleSource::StrikeCardsInDeck,
            }],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, AshenStrike costs 1 and deals 6 damage plus 3 for
        // every card in the player's exhaust pile.
        CardId::AshenStrike => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 6,
                per_unit: 3,
                source: ScaleSource::ExhaustPileSize,
            }],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, Bully costs 0 and deals 4 damage plus 2 for every
        // stack of Vulnerable on the target.
        CardId::Bully => CardData {
            cost: 0,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 4,
                per_unit: 2,
                source: ScaleSource::VulnerableStacksOnTarget,
            }],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, Conflagration costs 1 and deals 8 damage to ALL
        // enemies, plus 2 for each Attack played earlier this turn.
        CardId::Conflagration => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageScaled {
                base: 8,
                per_unit: 2,
                source: ScaleSource::AttacksPlayedThisTurn,
            }],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Per the wiki, TearAsunder costs 2 and deals 5 damage, hitting one
        // extra time for every time the player has been damaged this combat.
        CardId::TearAsunder => CardData {
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
            rarity: CardRarity::Rare,
        },
        // Per the wiki, Spite costs 0 and deals 5 damage, hitting twice if the
        // player has lost HP this turn.
        CardId::Spite => CardData {
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
            rarity: CardRarity::Uncommon,
        },
        // Per HOL-16, Dismantle costs 1 and deals 8 damage, hitting twice if
        // the target has Vulnerable.
        CardId::Dismantle => CardData {
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
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, MoltenFist costs 1, doubles the target's existing
        // Vulnerable stacks, deals 10 damage, and Exhausts.
        CardId::MoltenFist => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DoubleVulnerableOnTarget, EffectOp::DealDamage(10)],
            keywords: HashSet::from([CardKeyword::Exhaust]),
            rarity: CardRarity::Common,
        },
        // Per HOL-16, Dominate costs 1, applies Vulnerable to the target,
        // then gains Strength equal to the target's resulting Vulnerable
        // stack count, and exhausts.
        CardId::Dominate => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::GainStrengthEqualToTargetVulnerable,
            ],
            keywords: HashSet::from([CardKeyword::Exhaust]),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, FightMe! is now FightMe below.
        // Per the wiki, Breakthrough costs 1, makes the player Lose 1 HP,
        // and deals 9 damage to ALL enemies (non-targeted, like Thunderclap).
        CardId::Breakthrough => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![EffectOp::LoseHp(1), EffectOp::DealDamageToAllEnemies(9)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, Setup Strike costs 1, deals 7 damage, and grants the
        // player 2 Strength for this turn only.
        CardId::SetupStrike => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(7), EffectOp::ApplyStatusToSelf(Status::StrengthThisTurn(2))],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, Unrelenting costs 1, deals 12 damage, and makes the
        // next Attack the player plays cost 0 Energy.
        CardId::Unrelenting => CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(12), EffectOp::ApplyStatusToSelf(Status::FreeAttack)],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, Evil Eye costs 1, grants 8 Block, and grants another
        // 8 Block (16 total) if the player has Exhausted a card this turn.
        CardId::EvilEye => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlockScaled {
                base: 8,
                per_unit: 8,
                source: ScaleSource::ExhaustedCardThisTurn,
            }],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, Forgotten Ritual costs 0, Exhausts, and grants 3
        // Energy if the player has Exhausted a card this turn — its own
        // Exhaust (which resolves before its effects) satisfies that
        // condition, so playing it always nets +3 Energy.
        CardId::ForgottenRitual => CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainEnergyScaled {
                base: 0,
                per_unit: 3,
                source: ScaleSource::ExhaustedCardThisTurn,
            }],
            keywords: HashSet::from([CardKeyword::Exhaust]),
            rarity: CardRarity::Uncommon,
        },
        // Per the wiki, Pyre (Power) costs 1. At the start of each turn, gain
        // 1 Energy — identical shape to DemonForm's TurnStart->Strength.
        CardId::Pyre => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Pyre)],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Per the wiki, Anger costs 0, deals 6 damage, and adds a copy of
        // itself to the player's discard pile.
        CardId::Anger => CardData {
            cost: 0,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(6), EffectOp::AddCardToDiscard(CardId::Anger)],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // Per the wiki, DrumOfBattle (Power) costs 1. On play, draw 2 cards.
        // At the start of each turn, Exhaust the top card of the draw pile.
        CardId::DrumOfBattle => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![
                EffectOp::DrawCards(2),
                EffectOp::ApplyStatusToSelf(Status::BattleDrum),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Stomp costs 3 base (verified in-game; wiki was wrong), minus 1 per
        // Attack played this turn (min 0), deals 12 damage to ALL enemies.
        CardId::Stomp => CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageToAllEnemies(12)],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // FightMe: 5 damage twice to target, gain 3 Strength, give target 1 Strength.
        CardId::FightMe => CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(5),
                EffectOp::DealDamage(5),
                EffectOp::ApplyStatusToSelf(Status::Strength(3)),
                EffectOp::ApplyStatusToTarget(Status::Strength(1)),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // StoneArmor: apply Plating(4) — grants block at end of player turn,
        // decrements by 1 after each enemy turn, removed when 0.
        CardId::StoneArmor => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Plating(4))],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Vicious: apply Vicious(1) — when the player self-applies Vulnerable,
        // draw N cards.
        CardId::Vicious => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Vicious(1))],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Juggling: apply Juggling(1) — after the player's 3rd attack each
        // turn, add N copies of that attack card to hand.
        CardId::Juggling => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Juggling(1))],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Unmovable: apply Unmovable(1) — the first N block gains per turn
        // from cards are doubled.
        CardId::Unmovable => CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Unmovable(1))],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Per the wiki, PactsEnd costs 0, deals 17 AoE damage, and is only
        // playable when 3+ cards are in the exhaust pile.
        CardId::PactsEnd => CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageToAllEnemies(17)],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
        // Per the wiki, HowlFromBeyond costs 3, deals 16 AoE damage, Exhausts.
        // Auto-plays from exhaust pile at the start of each player turn.
        CardId::HowlFromBeyond => CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageToAllEnemies(16)],
            keywords: HashSet::from([CardKeyword::Exhaust]),
            rarity: CardRarity::Uncommon,
        },
        // Havoc: Play the top card of your draw pile and Exhaust it.
        // Cost 1 → 0 upgraded, Skill, Common.
        CardId::Havoc => CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::PlayTopOfDeck { count: 1, exhaust: true }],
            keywords: HashSet::new(),
            rarity: CardRarity::Common,
        },
        // BattleTrance: Draw 3 cards. You cannot draw additional cards this turn.
        // Cost 0, Skill, Uncommon.
        CardId::BattleTrance => CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::DrawCards(3),
                EffectOp::ApplyStatusToSelf(Status::NoDraw),
            ],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Whirlwind: Deal 5 damage to ALL enemies X times.
        // X-cost, Attack, Uncommon.
        // Ascender's Bane is an unplayable Curse added to the deck on Ascension
        // 15+. It occupies a hand slot but can never be played.
        CardId::AscendersBane => CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Status,
            effects: vec![],
            keywords: HashSet::from([CardKeyword::Unplayable]),
            rarity: CardRarity::Common,
        },
        CardId::Whirlwind => CardData {
            cost: -1,
            targeted: false,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamageRepeated {
                amount: 5,
                hits_base: 0,
                hits_per_unit: 1,
                hits_source: ScaleSource::EnergyX,
            }],
            keywords: HashSet::new(),
            rarity: CardRarity::Uncommon,
        },
        // Cascade: Play the top X cards of your draw pile.
        // X-cost, Skill, Rare. Upgraded: X+1 cards.
        CardId::Cascade => CardData {
            cost: -1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::PlayTopOfDeckScaled {
                count_base: 0,
                count_per_unit: 1,
                count_source: ScaleSource::EnergyX,
                exhaust: false,
            }],
            keywords: HashSet::new(),
            rarity: CardRarity::Rare,
        },
    }
}

/// Every card name `card_data_base` recognizes — there's no way to
/// enumerate a `match`'s patterns programmatically, so this is a
/// hand-maintained mirror (same tradeoff `bridge_mod`'s `NameMap.cs` already
/// accepts for its own translation tables). Kept immediately below
/// `card_data_base` so the two are easy to diff against each other when one
/// changes; a test asserts every entry here resolves via `card_data`,
/// catching typos/removed cards (the reverse direction — a `card_data_base`
/// entry missing from this list — isn't mechanically checkable here).
pub(crate) const ALL_CARD_NAMES: &[&str] = &[
    "Aggression",
    "Anger",
    "AshenStrike",
    "Barricade",
    "Bash",
    "BattleTrance",
    "Bloodletting",
    "BloodWall",
    "Bludgeon",
    "BodySlam",
    "Break",
    "Breakthrough",
    "Bully",
    "BurningPact",
    "Cinder",
    "Cascade",
    "Colossus",
    "Conflagration",
    "Corruption",
    "CrimsonMantle",
    "Cruelty",
    "DarkEmbrace",
    "Dazed",
    "Defend",
    "DemonForm",
    "Dismantle",
    "Dominate",
    "DrumOfBattle",
    "Evil Eye",
    "FeelNoPain",
    "FiendFire",
    "FightMe",
    "FlameBarrier",
    "Forgotten Ritual",
    "Headbutt",
    "Havoc",
    "Hemokinesis",
    "HowlFromBeyond",
    "Impervious",
    "Infection",
    "InfernalBlade",
    "Inferno",
    "Inflame",
    "Iron Wave",
    "Juggernaut",
    "Juggling",
    "Mangle",
    "MoltenFist",
    "NotYet",
    "Offering",
    "OneTwoPunch",
    "PerfectedStrike",
    "PactsEnd",
    "Pommel Strike",
    "Pyre",
    "Rage",
    "SecondWind",
    "Setup Strike",
    "ShrugItOff",
    "Slimed",
    "Spite",
    "Stomp",
    "StoneArmor",
    "Strike",
    "Sword Boomerang",
    "Taunt",
    "TearAsunder",
    "Thrash",
    "Thunderclap",
    "Tremble",
    "TrueGrit",
    "TwinStrike",
    "Unrelenting",
    "Unmovable",
    "Uppercut",
    "Vicious",
    "Whirlwind",
    "Wound",
];

#[cfg(test)]
mod card_data_tests {
    use super::*;

    #[test]
    fn every_name_in_all_card_names_resolves() {
        for name in ALL_CARD_NAMES {
            assert!(card_data(name, 0).is_some(), "{name} has no card_data entry");
        }
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
