//! Settling: waiting until the game's observable state stops changing and
//! satisfies a condition, and actions that return only once their effect is
//! visible.
//!
//! [`poll_until`] is the one polling loop; [`wait_until`], [`wait_settled`]
//! and [`Session::act`] are built on it. Waiting is always observed (repeated
//! reads under a deadline), never a fixed sleep.

use std::thread;
use std::time::{Duration, Instant};

use crate::backend::{Backend, BridgeError};
use crate::command::{Ack, Command, MenuNav};
use crate::state::{Actions, AvailableAction, CardRef, CombatView, GameState, Piles, Screen};

/// How long and how often to poll, and how many identical consecutive reads
/// count as stable.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SettlePolicy {
    /// Give up after this long.
    pub timeout: Duration,
    /// The pause before a confirming read, and the first pause after a read
    /// that did not satisfy the condition; the latter doubles up to
    /// `poll_max` while nothing changes, and resets when the state does.
    pub poll_min: Duration,
    pub poll_max: Duration,
    /// Consecutive identical reads that must satisfy the condition. A single
    /// `state()` is several bridge calls and can tear, hence more than one.
    pub stable_reads: usize,
}

impl Default for SettlePolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            poll_min: Duration::from_millis(25),
            poll_max: Duration::from_millis(100),
            stable_reads: 2,
        }
    }
}

impl SettlePolicy {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets `poll_min` and `poll_max`.
    pub fn with_poll(mut self, min: Duration, max: Duration) -> Self {
        self.poll_min = min;
        self.poll_max = max.max(min);
        self
    }

    /// At least 1.
    pub fn with_stable_reads(mut self, n: usize) -> Self {
        self.stable_reads = n.max(1);
        self
    }
}

/// Something [`poll_until`] can read repeatedly: compared for equality to
/// detect stability, and naming its screen when a wait times out.
pub trait Observation: PartialEq {
    fn screen(&self) -> Option<&Screen>;
}

impl Observation for GameState {
    fn screen(&self) -> Option<&Screen> {
        Some(&self.screen)
    }
}

impl Observation for Actions {
    fn screen(&self) -> Option<&Screen> {
        Some(&self.screen)
    }
}

/// A condition that did not hold: what was expected and what was seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmet {
    pub expected: String,
    pub observed: String,
}

impl Unmet {
    pub fn new(expected: impl Into<String>, observed: impl Into<String>) -> Self {
        Self {
            expected: expected.into(),
            observed: observed.into(),
        }
    }
}

/// Reads with `read` until `accept` holds on `policy.stable_reads` identical
/// consecutive reads, and returns the last one. A read error stops the wait
/// and is returned as is. At the deadline: [`BridgeError::NotSettled`] naming
/// `unmet` and the last screen read.
pub fn poll_until<T: Observation>(
    policy: &SettlePolicy,
    unmet: &str,
    read: impl FnMut() -> Result<T, BridgeError>,
    accept: impl FnMut(&T) -> bool,
) -> Result<T, BridgeError> {
    poll(policy, read, accept)?.map_err(|expired| BridgeError::NotSettled {
        waited: expired.waited,
        screen: expired.last.screen().cloned(),
        unmet: unmet.to_string(),
    })
}

/// [`poll_until`] over [`Backend::state`].
pub fn wait_until<B: Backend + ?Sized>(
    backend: &mut B,
    policy: &SettlePolicy,
    unmet: &str,
    accept: impl FnMut(&GameState) -> bool,
) -> Result<GameState, BridgeError> {
    poll_until(policy, unmet, || backend.state(), accept)
}

/// Waits for a stable, interactive state (see [`is_interactive`]).
pub fn wait_settled<B: Backend + ?Sized>(
    backend: &mut B,
    policy: &SettlePolicy,
) -> Result<GameState, BridgeError> {
    wait_until(backend, policy, INTERACTIVE, is_interactive)
}

const INTERACTIVE: &str = "an interactive screen";

/// The game accepts input on this screen: not loading, not the enemy turn,
/// and in combat only on the player's turn or an in-combat overlay
/// (`HAND_SELECT`, `CARD_SELECTION`). Out of combat, every other screen is,
/// except `MAIN_MENU` with a run in progress: that is a run loading or
/// tearing down (see [`crate::classify`]).
pub fn is_interactive(state: &GameState) -> bool {
    let screen = state.screen.as_str();
    if state.screen.is_loading() || screen == "COMBAT_ENEMY_TURN" {
        return false;
    }
    if screen == "MAIN_MENU" && state.run.is_some() {
        return false;
    }
    if screen.starts_with("COMBAT") {
        return screen == "COMBAT_PLAYER_TURN"
            && state.combat.as_ref().is_some_and(|c| c.is_player_turn);
    }
    true
}

struct Expired<T> {
    last: T,
    stable: bool,
    waited: Duration,
}

fn poll<T: PartialEq>(
    policy: &SettlePolicy,
    mut read: impl FnMut() -> Result<T, BridgeError>,
    mut accept: impl FnMut(&T) -> bool,
) -> Result<Result<T, Expired<T>>, BridgeError> {
    let started = Instant::now();
    let stable_reads = policy.stable_reads.max(1);
    let mut pause = policy.poll_min;
    let mut previous: Option<T> = None;
    let mut run = 0;
    loop {
        let current = read()?;
        if previous.as_ref() == Some(&current) {
            run += 1;
        } else {
            run = 1;
            pause = policy.poll_min;
        }
        let accepted = accept(&current);
        if accepted && run >= stable_reads {
            return Ok(Ok(current));
        }
        let waited = started.elapsed();
        if waited >= policy.timeout {
            return Ok(Err(Expired {
                last: current,
                stable: run >= stable_reads,
                waited,
            }));
        }
        // An accepted read is confirmed after the shortest pause, so that
        // "stable" spans more than one frame; otherwise back off.
        let remaining = policy.timeout - waited;
        if accepted {
            thread::sleep(policy.poll_min.min(remaining));
        } else {
            thread::sleep(pause.min(remaining));
            pause = (pause * 2).min(policy.poll_max);
        }
        previous = Some(current);
    }
}

/// A game behind a [`Backend`], driven one settled command at a time.
/// Every operation takes `&mut self`, so one command is in flight at most.
pub struct Session<B: Backend> {
    backend: B,
    policy: SettlePolicy,
}

impl<B: Backend> Session<B> {
    pub fn new(backend: B) -> Self {
        Self::with_policy(backend, SettlePolicy::default())
    }

    pub fn with_policy(backend: B, policy: SettlePolicy) -> Self {
        Self { backend, policy }
    }

    pub fn policy(&self) -> &SettlePolicy {
        &self.policy
    }

    pub fn set_policy(&mut self, policy: SettlePolicy) {
        self.policy = policy;
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// For reads and raw sends that bypass settling.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn into_backend(self) -> B {
        self.backend
    }

    /// One unsettled read.
    pub fn state(&mut self) -> Result<GameState, BridgeError> {
        self.backend.state()
    }

    /// [`wait_until`] with this session's policy.
    pub fn wait_until(
        &mut self,
        unmet: &str,
        accept: impl FnMut(&GameState) -> bool,
    ) -> Result<GameState, BridgeError> {
        wait_until(&mut self.backend, &self.policy, unmet, accept)
    }

    /// [`wait_settled`] with this session's policy.
    pub fn wait_settled(&mut self) -> Result<GameState, BridgeError> {
        wait_settled(&mut self.backend, &self.policy)
    }

    /// Sends `command` once the game is settled and returns the settled state
    /// in which its effect is visible.
    ///
    /// 1. Waits for a settled, interactive state; nothing is sent mid-transition.
    /// 2. Checks the command against that state; a command the game would
    ///    reject (an unplayable card, a bad target, an action not on offer) is
    ///    [`BridgeError::ActionRefused`] and is never sent.
    /// 3. Sends it, then waits until a stable interactive state satisfies the
    ///    command's postcondition.
    ///
    /// If the game settles without the effect: [`BridgeError::PostconditionFailed`];
    /// if it never settles: [`BridgeError::NotSettled`].
    pub fn act(&mut self, command: &Command) -> Result<GameState, BridgeError> {
        self.act_with(command, |_, _| Ok(()))
    }

    /// [`Session::act`] with an extra postcondition on the `(before, after)`
    /// states, which must hold as well (e.g. a predicted state delta).
    pub fn act_with(
        &mut self,
        command: &Command,
        extra: impl Fn(&GameState, &GameState) -> Result<(), Unmet>,
    ) -> Result<GameState, BridgeError> {
        let command = normalize(command);
        // A menu step is refused at once rather than after a settle wait
        // that a loading run would make time out.
        if matches!(command, Command::Navigate(_)) {
            refuse_while_loading(&mut self.backend, &command)?;
        }
        let with_actions = matches!(command, Command::Choose(_));
        let backend = &mut self.backend;
        let before = match poll(
            &self.policy,
            || View::read(backend, with_actions),
            View::settled,
        )? {
            Ok(view) => view,
            Err(expired) => {
                return Err(BridgeError::NotSettled {
                    waited: expired.waited,
                    screen: Some(expired.last.state.screen),
                    unmet: INTERACTIVE.to_string(),
                })
            }
        };
        precheck(&command, &before).map_err(|reason| BridgeError::ActionRefused {
            action: command.to_string(),
            reason,
        })?;
        refuse_while_loading(&mut self.backend, &command)?;

        let _ack: Ack = self.backend.send(&command)?;

        let check = |after: &View| {
            postcondition(&command, &before, after)
                .and_then(|()| extra(&before.state, &after.state))
        };
        let backend = &mut self.backend;
        let outcome = poll(
            &self.policy,
            || View::read(backend, with_actions),
            |view| view.settled() && check(view).is_ok(),
        )?;
        match outcome {
            Ok(view) => Ok(view.state),
            Err(expired) => {
                let last = expired.last;
                match check(&last) {
                    Err(unmet) if expired.stable && last.settled() => {
                        Err(BridgeError::PostconditionFailed {
                            action: command.to_string(),
                            expected: unmet.expected,
                            observed: unmet.observed,
                        })
                    }
                    result => Err(BridgeError::NotSettled {
                        waited: expired.waited,
                        screen: Some(last.state.screen),
                        unmet: match result {
                            Ok(()) => INTERACTIVE.to_string(),
                            Err(u) => format!("{INTERACTIVE} with {}", u.expected),
                        },
                    }),
                }
            }
        }
    }
}

#[derive(PartialEq)]
struct View {
    state: GameState,
    /// Read only for commands whose effect may show only in the actions.
    actions: Option<Actions>,
}

impl View {
    fn read<B: Backend + ?Sized>(backend: &mut B, with_actions: bool) -> Result<Self, BridgeError> {
        let state = backend.state()?;
        let actions = with_actions.then(|| backend.actions()).transpose()?;
        Ok(Self { state, actions })
    }

    /// Interactive, and the two reads saw the same screen (no tear).
    fn settled(&self) -> bool {
        is_interactive(&self.state)
            && self
                .actions
                .as_ref()
                .is_none_or(|a| a.screen == self.state.screen)
    }
}

/// A command that can change rooms (`Choose`, `Navigate`) is refused while
/// [`crate::classify`] says a run is loading: `state()` cannot see all of it
/// (a run with no room yet reads as a settled `MAP`).
fn refuse_while_loading<B: Backend + ?Sized>(
    backend: &mut B,
    command: &Command,
) -> Result<(), BridgeError> {
    if !matches!(command, Command::Choose(_) | Command::Navigate(_)) {
        return Ok(());
    }
    let probe = match backend.probe() {
        // The explorer is read only on a no-run main menu: not loading.
        Err(BridgeError::ExplorerUnavailable { .. }) => return Ok(()),
        other => other?,
    };
    if crate::menu::classify(&probe) == crate::menu::Phase::Loading {
        return Err(BridgeError::ActionRefused {
            action: command.to_string(),
            reason: "room still loading".to_string(),
        });
    }
    Ok(())
}

/// The game waits for a card choice: the command's own effect is pending it.
fn awaits_selection(state: &GameState) -> bool {
    state.combat.is_some() && matches!(state.screen.as_str(), "HAND_SELECT" | "CARD_SELECTION")
}

/// `Choose` of an action that has a typed command becomes that command, so
/// it gets that command's checks.
fn normalize(command: &Command) -> Command {
    match command {
        Command::Choose(a) if a.action == "play_card" && a.card_index.is_some() => {
            Command::PlayCard {
                card_index: a.card_index.unwrap_or_default(),
                target: a.target_index,
            }
        }
        Command::Choose(a) if a.action == "end_turn" => Command::EndTurn,
        other => other.clone(),
    }
}

fn precheck(command: &Command, before: &View) -> Result<(), String> {
    let state = &before.state;
    match command {
        Command::PlayCard { card_index, target } => {
            let combat = player_turn(state)?;
            let hand = &combat.piles.hand;
            let card = hand.get(*card_index).ok_or_else(|| {
                format!(
                    "card index {card_index} is out of range (hand size {})",
                    hand.len()
                )
            })?;
            let name = card.id.name();
            if !card.playable {
                return Err(format!(
                    "{name} is not playable: {}",
                    card.unplayable_reason
                        .as_deref()
                        .unwrap_or("no reason given")
                ));
            }
            match (target, &card.valid_targets) {
                (Some(t), Some(valid)) if !valid.contains(t) => Err(format!(
                    "enemy {t} is not a valid target for {name} (valid: {valid:?})"
                )),
                (Some(t), None) => Err(format!("{name} takes no target, but got enemy {t}")),
                (None, Some(valid)) if !valid.is_empty() => {
                    Err(format!("{name} needs a target (valid: {valid:?})"))
                }
                _ => Ok(()),
            }
        }
        Command::EndTurn => player_turn(state).map(|_| ()),
        Command::Choose(action) => {
            if action.action == "console" {
                return Err("console commands are not screen choices".to_string());
            }
            let offered = before.actions.as_ref().map_or(&[][..], |a| &a.actions[..]);
            if offered.contains(action) {
                Ok(())
            } else {
                Err(format!(
                    "not on offer on {}; offered: {}",
                    state.screen,
                    describe_actions(offered)
                ))
            }
        }
        Command::Navigate(MenuNav::AbandonRun) if state.run.is_none() => {
            Err(format!("no run in progress (screen {})", state.screen))
        }
        Command::Navigate(_) => Ok(()),
    }
}

fn player_turn(state: &GameState) -> Result<&CombatView, String> {
    match &state.combat {
        Some(c) if state.screen.as_str() == "COMBAT_PLAYER_TURN" && c.is_player_turn => Ok(c),
        _ => Err(format!("not the player's turn (screen {})", state.screen)),
    }
}

fn describe_actions(actions: &[AvailableAction]) -> String {
    let names: Vec<&str> = actions.iter().map(|a| a.action.as_str()).collect();
    format!("[{}]", names.join(", "))
}

fn postcondition(command: &Command, before: &View, after: &View) -> Result<(), Unmet> {
    let (b, a) = (&before.state, &after.state);
    if !matches!(command, Command::Choose(_)) && awaits_selection(a) {
        return Ok(()); // the effect continues once the caller chooses
    }
    match command {
        Command::PlayCard { card_index, .. } => {
            let Some(after_combat) = &a.combat else {
                return Ok(()); // the play ended the combat
            };
            let Some(card) = b
                .combat
                .as_ref()
                .and_then(|c| c.piles.hand.get(*card_index))
            else {
                return Err(Unmet::new("a card to play", "none"));
            };
            let before_combat = b.combat.as_ref().expect("checked above");
            let id = &card.id;
            let in_piles = |piles: &Piles| {
                let count = |cards: &[CardRef]| cards.iter().filter(|c| &c.id == id).count();
                count(&piles.draw) + count(&piles.discard) + count(&piles.exhaust)
            };
            let in_hand = |piles: &Piles| piles.hand.iter().filter(|c| &c.id == id).count();
            let (pb, pa) = (&before_combat.piles, &after_combat.piles);
            let total = in_piles(pb) + in_hand(pb);
            let now = in_piles(pa) + in_hand(pa);
            // While it resolves, the played card is in none of the reported
            // piles, so the copies visible drop by one until it lands. Draws,
            // copies and exhausts of the same id move copies between the
            // reported piles or add some, but never hide one.
            let (met, expected) = if card.kind == "Power" {
                // A Power leaves combat instead of landing: indistinguishable
                // from in flight by the piles alone.
                (
                    now + 1 == total,
                    format!(
                        "{} to leave combat (copies {total} -> {})",
                        id.name(),
                        total - 1
                    ),
                )
            } else {
                // A cancelled play leaves the card in hand and changes nothing.
                (
                    now >= total && after_combat != before_combat,
                    format!(
                        "{} played and back in a pile (copies {total} -> at least {total})",
                        id.name()
                    ),
                )
            };
            if met {
                Ok(())
            } else {
                Err(Unmet::new(
                    expected,
                    format!(
                        "copies in piles {}, in hand {}, energy {} -> {}",
                        in_piles(pa),
                        in_hand(pa),
                        before_combat.player.energy,
                        after_combat.player.energy
                    ),
                ))
            }
        }
        Command::EndTurn => {
            let Some(after_combat) = &a.combat else {
                return Ok(()); // the combat is over
            };
            let round = b.combat.as_ref().map_or(0, |c| c.round);
            if after_combat.round > round
                && after_combat.is_player_turn
                && a.screen.as_str() == "COMBAT_PLAYER_TURN"
            {
                Ok(())
            } else {
                Err(Unmet::new(
                    format!("round {} on COMBAT_PLAYER_TURN", round + 1),
                    format!("round {} on {}", after_combat.round, a.screen),
                ))
            }
        }
        Command::Choose(_) => {
            if before != after {
                Ok(())
            } else {
                Err(Unmet::new(
                    "the state or the actions on offer to change",
                    format!("nothing changed on {}", a.screen),
                ))
            }
        }
        Command::Navigate(_) => {
            if a.screen != b.screen || a.run.is_some() != b.run.is_some() {
                Ok(())
            } else {
                Err(Unmet::new(
                    "the screen to change",
                    format!("still on {}", a.screen),
                ))
            }
        }
    }
}
