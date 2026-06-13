use crate::engine::{EffectOp, HandFilter, ScaleSource, Status};

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
    // Whether this card goes to the exhaust pile (rather than discard) after
    // being played. Power/Status cards always exhaust regardless of this
    // flag (handled separately in `apply`); this flag covers Attacks/Skills
    // that the wiki explicitly marks with the Exhaust keyword (e.g. Offering,
    // Tremble, Impervious, NotYet).
    pub(crate) exhausts: bool,
}

pub(crate) fn card_data(name: &str) -> Option<CardData> {
    match name {
        "Strike" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(6)],
            exhausts: false,
        }),
        "Defend" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlock(5)],
            exhausts: false,
        }),
        // Per the Slay the Spire wiki, base Bash deals 8 damage and applies
        // 2 Vulnerable stacks (not 1).
        "Bash" => Some(CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Skill,
            effects: vec![
                EffectOp::DealDamage(8),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
            exhausts: false,
        }),
        "Iron Wave" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(5), EffectOp::GainBlock(5)],
            exhausts: false,
        }),
        "Inflame" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))],
            exhausts: false,
        }),
        // 3 hits of 3 damage each to a random enemy (always the same target
        // in single-enemy fights). Targeted so SelectTarget resolves first.
        "Sword Boomerang" => Some(CardData {
            cost: 2,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![
                EffectOp::DealDamage(3),
                EffectOp::DealDamage(3),
                EffectOp::DealDamage(3),
            ],
            exhausts: false,
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
            exhausts: false,
        }),
        // Installs the Rage status: gain 2 Block each time you play an Attack.
        "Rage" => Some(CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Rage)],
            exhausts: false,
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
            exhausts: false,
        }),
        // Installs the Crimson Mantle status: at the start of each turn,
        // gain 8 Block and lose HP equal to a counter that starts at 1 and
        // increases by 1 each turn.
        "CrimsonMantle" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::CrimsonMantle(1))],
            exhausts: false,
        }),
        // Installs the Inferno status: at the start of each turn, lose 1 HP
        // (unblockable); whenever the holder loses HP on their turn, deal 6
        // damage to all enemies.
        "Inferno" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Inferno)],
            exhausts: false,
        }),
        // Installs the Aggression status: at the start of each turn, return
        // a random Attack from the discard pile to hand.
        "Aggression" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Aggression)],
            exhausts: false,
        }),
        // Installs the Dark Embrace status: whenever a card is Exhausted,
        // draw 1 card.
        "DarkEmbrace" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::DarkEmbrace)],
            exhausts: false,
        }),
        // Installs the Feel No Pain status: whenever a card is Exhausted,
        // gain 3 Block.
        "FeelNoPain" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::FeelNoPain)],
            exhausts: false,
        }),
        // Installs the Barricade status: Block is no longer removed at the
        // start of your turn.
        "Barricade" => Some(CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Barricade)],
            exhausts: false,
        }),
        // Installs the Juggernaut status: whenever you gain Block, deal 5
        // damage to a random enemy.
        "Juggernaut" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Juggernaut(5))],
            exhausts: false,
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
            exhausts: false,
        }),
        // Installs the Colossus status: incoming damage from attackers with
        // Vulnerable is halved.
        "Colossus" => Some(CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Colossus(1))],
            exhausts: false,
        }),
        // Installs the Corruption status: Skills cost 0 and Exhaust when
        // played.
        "Corruption" => Some(CardData {
            cost: 3,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Corruption)],
            exhausts: false,
        }),
        // Installs the Cruelty status: damage dealt to Vulnerable targets is
        // amplified by 1.75x instead of 1.5x.
        "Cruelty" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Power,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Cruelty)],
            exhausts: false,
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
            exhausts: false,
        }),
        // Installs the One Two Punch status: the next Attack played this
        // turn is played a second time.
        "OneTwoPunch" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::OneTwoPunch)],
            exhausts: false,
        }),
        "Pommel Strike" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(9), EffectOp::DrawCards(1)],
            exhausts: false,
        }),
        // Per the wiki, Bloodletting costs 0, deals 3 unblockable damage to
        // the player, and grants 2 Energy.
        "Bloodletting" => Some(CardData {
            cost: 0,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::LoseHp(3), EffectOp::GainEnergy(2)],
            exhausts: false,
        }),
        // Per the wiki, BloodWall costs 2, deals 2 unblockable damage to the
        // player, and grants 16 Block.
        "BloodWall" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::LoseHp(2), EffectOp::GainBlock(16)],
            exhausts: false,
        }),
        // Per the wiki, Hemokinesis costs 1, deals 2 unblockable damage to
        // the player, and deals 15 damage to a chosen enemy.
        "Hemokinesis" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::LoseHp(2), EffectOp::DealDamage(15)],
            exhausts: false,
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
            exhausts: true,
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
            exhausts: true,
        }),
        // Per the wiki, Impervious costs 2, grants 30 Block, and Exhausts.
        "Impervious" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlock(30)],
            exhausts: true,
        }),
        // Per the wiki, NotYet costs 2, heals 10 HP (capped at max HP), and
        // Exhausts.
        "NotYet" => Some(CardData {
            cost: 2,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::Heal(10)],
            exhausts: true,
        }),
        // The slime monsters' Goop/StickyShot moves stick this into the
        // player's discard pile. Per the wiki: 1 energy, draws 1 card,
        // exhausts on play.
        "Slimed" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Status,
            effects: vec![EffectOp::DrawCards(1)],
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: true,
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
            exhausts: true,
        }),
        // Per the wiki, Bludgeon costs 3 and deals 32 damage.
        "Bludgeon" => Some(CardData {
            cost: 3,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(32)],
            exhausts: false,
        }),
        // Per the wiki, TwinStrike costs 1 and deals 5 damage twice (10
        // total).
        "TwinStrike" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DealDamage(5), EffectOp::DealDamage(5)],
            exhausts: false,
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
            exhausts: false,
        }),
        // Per the wiki, ShrugItOff costs 1, gains 8 block, and draws 1 card.
        "ShrugItOff" => Some(CardData {
            cost: 1,
            targeted: false,
            card_type: CardType::Skill,
            effects: vec![EffectOp::GainBlock(8), EffectOp::DrawCards(1)],
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
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
            exhausts: false,
        }),
        // Per the wiki, MoltenFist costs 1, doubles the target's existing
        // Vulnerable stacks, deals 10 damage, and Exhausts.
        "MoltenFist" => Some(CardData {
            cost: 1,
            targeted: true,
            card_type: CardType::Attack,
            effects: vec![EffectOp::DoubleVulnerableOnTarget, EffectOp::DealDamage(10)],
            exhausts: true,
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
            exhausts: true,
        }),
        _ => None,
    }
}
