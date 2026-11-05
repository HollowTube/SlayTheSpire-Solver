//! Typed write commands. [`crate::Backend::send`] sends one raw, without
//! settling; [`crate::Session::act`] sends one and waits for its effect.

use std::fmt;

use crate::console::ConsoleCommand;
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
    /// A menu step: leave a run or a menu screen.
    Navigate(MenuNav),
    /// A dev console command; sent verified by [`crate::Session::console`].
    Console(ConsoleCommand),
    /// Starts a seeded run from the main menu. The run then loads for tens
    /// of seconds: start it with [`crate::Game::start_run`], which waits.
    StartRun(RunSpec),
}

/// A seeded run.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RunSpec {
    /// The character's class name, e.g. `Ironclad`.
    pub character: String,
    pub seed: String,
    pub ascension: u32,
}

impl RunSpec {
    pub fn new(character: impl Into<String>, seed: impl Into<String>) -> Self {
        Self {
            character: character.into(),
            seed: seed.into(),
            ascension: 0,
        }
    }

    pub fn with_ascension(mut self, ascension: u32) -> Self {
        self.ascension = ascension;
        self
    }
}

impl fmt::Display for RunSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} seed {} ascension {}",
            self.character, self.seed, self.ascension
        )
    }
}

/// A step toward the main menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MenuNav {
    /// Abandon the run in progress; the game shows its game-over screen.
    AbandonRun,
    /// Leave the current menu screen or popup.
    Back,
    /// Leave the game-over screen for the main menu.
    LeaveGameOver,
}

impl MenuNav {
    pub fn as_str(&self) -> &'static str {
        match self {
            MenuNav::AbandonRun => "abandon_run",
            MenuNav::Back => "back",
            MenuNav::LeaveGameOver => "leave_game_over",
        }
    }
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
            Command::Navigate(nav) => write!(f, "navigate({})", nav.as_str()),
            Command::Console(c) => write!(f, "console({})", c.line()),
            Command::StartRun(spec) => write!(f, "start_run({spec})"),
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
