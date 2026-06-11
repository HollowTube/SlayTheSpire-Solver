use crate::engine::{EffectOp, Status};

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
        _ => None,
    }
}
