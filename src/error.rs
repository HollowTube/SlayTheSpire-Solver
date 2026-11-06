use std::fmt;

/// Why the sim refused an action, a run step or a state. `Display` is the
/// exact message the Python bindings raise as `ValueError`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError {
    HandChoicePending { action: String },
    NoUpgradable { card: String },
    NotInHand { card: String },
    CannotAfford { card: String, cost: i32, energy: i32 },
    UnknownAction { action: String },
    UnknownCard { card: String },
    UnknownEncounter { name: String },
    NotOnOffer { card: String },
    UnknownRunAction { action: String },
    InvalidUpgradeIndex { action: String },
    NoCardAtIndex { index: usize },
    AlreadyUpgraded { index: usize },
    RunAtTerminal { action: String },
    ResolveAtNonCombat,
    /// `CombatParts` named a status the engine does not model.
    UnknownStatus { name: String },
    /// `CombatParts` carried a monster intent the monster does not have.
    UnknownMove { monster: String, intent: String },
    /// `CombatParts` is internally inconsistent.
    BadParts { reason: String },
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimError::HandChoicePending { action } => {
                write!(f, "{action} is not legal until the pending hand choice is made")
            }
            SimError::NoUpgradable { card } => write!(f, "no upgradable {card} in hand"),
            SimError::NotInHand { card } => write!(f, "{card} is not in hand"),
            SimError::CannotAfford { card, cost, energy } => {
                write!(f, "cannot afford {card}: costs {cost} but only {energy} energy available")
            }
            SimError::UnknownAction { action } => write!(f, "unknown action: {action}"),
            SimError::UnknownCard { card } => write!(f, "unknown card: {card}"),
            SimError::UnknownEncounter { name } => write!(f, "unknown encounter: {name}"),
            SimError::NotOnOffer { card } => write!(f, "{card} is not on offer"),
            SimError::UnknownRunAction { action } => write!(f, "unknown run action: {action}"),
            SimError::InvalidUpgradeIndex { action } => {
                write!(f, "invalid upgrade index: {action}")
            }
            SimError::NoCardAtIndex { index } => write!(f, "no card at index {index}"),
            SimError::AlreadyUpgraded { index } => {
                write!(f, "card at index {index} is already upgraded")
            }
            SimError::RunAtTerminal { action } => write!(f, "{action} at a terminal run"),
            SimError::ResolveAtNonCombat => {
                write!(f, "ResolveCombat at a terminal or non-combat node")
            }
            SimError::UnknownStatus { name } => write!(f, "unknown status: {name}"),
            SimError::UnknownMove { monster, intent } => {
                write!(f, "{monster} has no move {intent}")
            }
            SimError::BadParts { reason } => write!(f, "invalid combat parts: {reason}"),
        }
    }
}

impl std::error::Error for SimError {}
