use crate::engine::{EffectOp, Status};

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
    pub(crate) effects: Vec<EffectOp>,
}

pub(crate) fn card_data(name: &str) -> Option<CardData> {
    match name {
        "Strike" => Some(CardData {
            cost: 1,
            targeted: true,
            effects: vec![EffectOp::DealDamage(6)],
        }),
        "Defend" => Some(CardData {
            cost: 1,
            targeted: false,
            effects: vec![EffectOp::GainBlock(5)],
        }),
        "Bash" => Some(CardData {
            cost: 2,
            targeted: true,
            effects: vec![
                EffectOp::DealDamage(8),
                EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            ],
        }),
        "Iron Wave" => Some(CardData {
            cost: 1,
            targeted: true,
            effects: vec![EffectOp::DealDamage(5), EffectOp::GainBlock(5)],
        }),
        "Inflame" => Some(CardData {
            cost: 1,
            targeted: false,
            effects: vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))],
        }),
        _ => None,
    }
}
