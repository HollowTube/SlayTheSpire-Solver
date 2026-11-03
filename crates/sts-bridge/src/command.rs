//! Typed write commands. [`crate::Backend::send`] sends one raw, without
//! settling; [`crate::Session::act`] sends one and waits for its effect.

use std::fmt;

use crate::state::AvailableAction;

/// One command the game can be sent.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Command {
    /// Play the hand card at `card_index`, at enemy `target` when it needs one.
    PlayCard {
        card_index: usize,
        target: Option<usize>,
    },
    EndTurn,
    /// Take an action as `Backend::actions` listed it (a map node, an event
    /// option, a reward, …).
    Choose(AvailableAction),
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::PlayCard {
                card_index,
                target: Some(t),
            } => write!(f, "play_card(card {card_index} -> enemy {t})"),
            Command::PlayCard {
                card_index,
                target: None,
            } => write!(f, "play_card(card {card_index})"),
            Command::EndTurn => f.write_str("end_turn"),
            Command::Choose(a) => match &a.label {
                Some(label) => write!(f, "choose({} {label:?})", a.action),
                None => write!(f, "choose({})", a.action),
            },
        }
    }
}

/// The backend accepted a command. Acceptance is not its effect: the game
/// may still be applying it, or may silently cancel it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Ack {
    /// The backend's raw reply, for diagnostics.
    pub reply: serde_json::Value,
}

impl Ack {
    pub fn new(reply: serde_json::Value) -> Self {
        Self { reply }
    }
}
