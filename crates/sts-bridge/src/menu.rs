//! Where the game is ([`Probe`], [`Phase`], [`classify`]) and getting it to a
//! known place: [`wait_run_ready`] and [`ensure_main_menu`].
//!
//! [`classify`] is the one place that decides whether the game is loading.
//! Nothing here sends a command while it is: a room-changing command sent
//! while a new run loads crashes the game.

use std::time::{Duration, Instant};

use crate::backend::{Backend, BridgeError, WedgeReason};
use crate::command::{Command, MenuNav};
use crate::lifecycle::Timeouts;
use crate::settle::{poll_until, Observation, SettlePolicy};
use crate::state::Screen;

/// One cheap read of where the game is: the bridge's `ping`, its
/// `get_screen` and, on the main menu, whether the main-menu node exists.
/// Equal reads compare equal (no latency field), so it can be polled for
/// stability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// Ping's `status` was `ok`.
    pub ok: bool,
    /// Ping's screen. `None` when the bridge could not read it.
    pub screen: Option<Screen>,
    pub run_in_progress: Option<bool>,
    pub in_combat: Option<bool>,
    /// `get_screen`, read after ping; `None` when ping had no screen.
    pub detail: Option<ScreenDetail>,
    /// Whether the main-menu scene node exists. Read only when ping says
    /// `MAIN_MENU` with no run in progress; `None` otherwise.
    pub main_menu_node: Option<bool>,
}

/// The bridge's `get_screen` reply.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScreenDetail {
    pub screen: Option<Screen>,
    /// How the bridge derived the screen, e.g. `active_screen_context`,
    /// `overlay_stack`, `run_state` (no room yet) or `fallback`.
    pub source: Option<String>,
    /// The current room's type, e.g. `EventRoom`.
    pub room_type: Option<String>,
    /// The active screen's type, e.g. `NGameOverScreen`.
    pub context: Option<String>,
}

impl Observation for Probe {
    fn screen(&self) -> Option<&Screen> {
        self.screen.as_ref()
    }
}

/// What the game is doing, as far as driving it is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Phase {
    /// A run is loading or tearing down. Never send anything.
    Loading,
    /// `MAIN_MENU` with no run in progress. A candidate only: `MAIN_MENU` is
    /// also the bridge's fallback name, see [`ensure_main_menu`].
    MainMenu,
    /// A menu screen outside a run (character select, settings, a popup).
    MenuSubscreen,
    /// The game-over screen.
    GameOver,
    /// Between two stable screens (the enemy turn, a room opening).
    Transition,
    /// A stable, interactive screen inside a run.
    InRun,
    /// Ping answered without a screen.
    Unreadable,
    /// None of the above; nothing is sent.
    Unknown,
}

const IN_RUN: &[&str] = &[
    "COMBAT_PLAYER_TURN",
    "HAND_SELECT",
    "CARD_SELECTION",
    "MAP",
    "EVENT",
    "SHOP",
    "REST_SITE",
    "TREASURE",
    "REWARD",
    "CARD_REWARD",
    "RELIC_SELECTION",
    "OVERLAY",
];

const MENU_SCREENS: &[&str] = &[
    "CHARACTER_SELECT",
    "SINGLEPLAYER_MENU",
    "SETTINGS",
    "TIMELINE",
    "OVERLAY",
];

/// The single decision of what phase a [`Probe`] shows.
///
/// `Loading` is any `*LOADING*` screen; `MAIN_MENU` with a run in progress
/// (the bridge's name for a run that is loading or tearing down, which holds
/// for tens of seconds after `start_run`); and `MAP` derived from the run
/// state (`screen_source: run_state`, a run with no room yet).
pub fn classify(probe: &Probe) -> Phase {
    let Some(screen) = probe.screen.as_ref().filter(|_| probe.ok) else {
        return Phase::Unreadable;
    };
    let s = screen.as_str();
    let run = probe.run_in_progress == Some(true);
    let detail = probe.detail.as_ref();
    let detail_screen = detail.and_then(|d| d.screen.as_ref());
    let source = detail.and_then(|d| d.source.as_deref());

    if screen.is_loading() || detail_screen.is_some_and(Screen::is_loading) {
        return Phase::Loading;
    }
    if run && (s == "MAIN_MENU" || (s == "MAP" && source == Some("run_state"))) {
        return Phase::Loading;
    }
    // Ping and get_screen are separate reads: disagreement is a change in
    // progress.
    if detail_screen.is_some_and(|d| d != screen) {
        return Phase::Transition;
    }
    let context = detail.and_then(|d| d.context.as_deref()).unwrap_or("");
    if s == "GAME_OVER" || (s == "OVERLAY" && context.contains("GameOver")) {
        return Phase::GameOver;
    }
    if !run {
        return match s {
            "MAIN_MENU" => Phase::MainMenu,
            "MENU_NLogoAnimation" => Phase::Transition,
            _ if MENU_SCREENS.contains(&s) || s.starts_with("MENU_") => Phase::MenuSubscreen,
            _ => Phase::Unknown,
        };
    }
    match s {
        "COMBAT_ENEMY_TURN" | "MENU_NCombatRoom" => Phase::Transition,
        // Menus open over a run. Other in-run `MENU_*` screens stay Unknown:
        // they may be rooms opening, where a Back is a guess.
        "SETTINGS" | "TIMELINE" => Phase::MenuSubscreen,
        _ if IN_RUN.contains(&s) => Phase::InRun,
        _ if s.starts_with("COMBAT") => Phase::Transition,
        _ => Phase::Unknown,
    }
}

/// A bridge failure that means the game stopped making progress, as
/// [`BridgeError::Wedged`]; other errors unchanged.
pub(crate) fn as_wedge(error: BridgeError, screen: Option<Screen>) -> BridgeError {
    match error {
        BridgeError::Timeout { .. } => BridgeError::Wedged {
            screen,
            reason: WedgeReason::BridgeTimeout,
        },
        BridgeError::Remote { message, .. } if message.contains("dispatch timed out") => {
            BridgeError::Wedged {
                screen,
                reason: WedgeReason::MainThreadTimeout,
            }
        }
        other => other,
    }
}

/// Reads probes and remembers the last screen seen, for error messages.
struct Prober<'a, B: Backend + ?Sized> {
    backend: &'a mut B,
    last: Option<Screen>,
    last_probe: Option<Probe>,
}

impl<'a, B: Backend + ?Sized> Prober<'a, B> {
    fn new(backend: &'a mut B) -> Self {
        Self {
            backend,
            last: None,
            last_probe: None,
        }
    }

    fn read(&mut self) -> Result<Probe, BridgeError> {
        match self.backend.probe() {
            Ok(p) => {
                if p.screen.is_some() {
                    self.last = p.screen.clone();
                }
                self.last_probe = Some(p.clone());
                Ok(p)
            }
            Err(e) => Err(as_wedge(e, self.last.clone())),
        }
    }

    fn wait(
        &mut self,
        policy: &SettlePolicy,
        unmet: &str,
        mut accept: impl FnMut(&Probe) -> bool,
    ) -> Result<Probe, BridgeError> {
        poll_until(policy, unmet, || self.read(), |p| accept(p))
    }

    fn send(&mut self, nav: MenuNav) -> Result<(), BridgeError> {
        self.backend
            .send(&Command::Navigate(nav))
            .map(|_| ())
            .map_err(|e| as_wedge(e, self.last.clone()))
    }
}

/// Waits, sending nothing, until a stable in-run screen ([`Phase::InRun`])
/// holds on `timeouts.probe.stable_reads` reads spanning at least
/// `poll_min * (stable_reads - 1)`, bounded by `timeouts.run_load`.
///
/// This is the precondition for any room-changing command (console `fight`,
/// `room`, `event`): it is never met while a run is loading. At the deadline:
/// [`BridgeError::NotSettled`] naming the last screen. A bridge that stops
/// answering is [`BridgeError::Wedged`].
pub fn wait_run_ready<B: Backend + ?Sized>(
    backend: &mut B,
    timeouts: &Timeouts,
) -> Result<Probe, BridgeError> {
    let policy = timeouts.probe.clone().with_timeout(timeouts.run_load);
    Prober::new(backend).wait(&policy, "a stable in-run screen", |p| {
        classify(p) == Phase::InRun
    })
}

/// A combat room that never finishes opening.
const STUCK_COMBAT_ROOM: &str = "MENU_NCombatRoom";

/// The most navigation commands one [`ensure_main_menu`] sends.
pub const MAX_NAV_STEPS: usize = 8;

/// Brings the game to a verified main menu from any screen, and returns the
/// probe that verified it.
///
/// Each turn takes a stable probe and acts on its [`Phase`]: waits out
/// `Loading` (up to `run_load`) and `Transition`; abandons a run from a
/// settled in-run screen; leaves the game-over screen; goes back from a menu
/// screen. Every step is verified by the phase it leads to. On a verified
/// main menu it sends nothing.
///
/// A verified main menu is [`Phase::MainMenu`] on stable probes, the
/// main-menu node present, and [`Backend::state`] with no run.
///
/// Errors: [`BridgeError::NotSettled`] (deadline, step cap, an `Unknown`
/// screen), [`BridgeError::PostconditionFailed`] (a step was accepted but the
/// screen did not move), [`BridgeError::Wedged`] (a stuck transition, a
/// screenless ping, a bridge timeout), and any bridge error, e.g.
/// [`BridgeError::Refused`], as is.
pub fn ensure_main_menu<B: Backend + ?Sized>(
    backend: &mut B,
    timeouts: &Timeouts,
) -> Result<Probe, BridgeError> {
    let started = Instant::now();
    let mut prober = Prober::new(backend);
    let mut steps = 0;
    const GOAL: &str = "a verified main menu";
    let deadline = started + timeouts.ensure_total;
    let remaining = || deadline.saturating_duration_since(Instant::now());
    let within = |d: Duration| timeouts.probe.clone().with_timeout(d.min(remaining()));
    loop {
        if remaining().is_zero() {
            return Err(BridgeError::NotSettled {
                waited: started.elapsed(),
                screen: prober.last.clone(),
                unmet: GOAL.to_string(),
            });
        }
        let probe = prober.wait(&within(timeouts.step), "a stable screen", |_| true)?;
        let phase = classify(&probe);
        match phase {
            Phase::MainMenu => {
                if probe.main_menu_node == Some(true) {
                    let state = prober
                        .backend
                        .state()
                        .map_err(|e| as_wedge(e, prober.last.clone()))?;
                    if state.run.is_none() {
                        return Ok(probe);
                    }
                }
                // The bridge's fallback MAIN_MENU: the menu is not up yet.
                prober.wait(&within(timeouts.step), GOAL, |p| p != &probe)?;
            }
            Phase::Loading | Phase::Transition | Phase::Unreadable => {
                let screen_now = probe.screen.as_ref().map(Screen::as_str);
                let (stuck_after, unmet) = match phase {
                    Phase::Loading => (timeouts.run_load, "the run to finish loading"),
                    Phase::Unreadable => (timeouts.stuck_screen, "a screen"),
                    _ if screen_now == Some(STUCK_COMBAT_ROOM) => {
                        (timeouts.stuck_screen, "the combat room to open")
                    }
                    _ => (timeouts.step, "the transition to end"),
                };
                let full_wait = remaining() >= stuck_after;
                // Any move ends this wait; the next turn judges the new screen
                // with its own deadline. A wedge only if this screen held the
                // whole time: reads that kept changing are a game still moving.
                let mut held = true;
                match prober.wait(&within(stuck_after), unmet, |p| {
                    let moved = p.screen != probe.screen || classify(p) != phase;
                    held &= !moved;
                    moved
                }) {
                    Err(BridgeError::NotSettled { screen, .. }) if full_wait && held => {
                        let reason = match (phase, screen.as_ref().map(Screen::as_str)) {
                            (Phase::Unreadable, _) => WedgeReason::PingWithoutScreen,
                            (Phase::Loading, s) => {
                                WedgeReason::StuckScreen(format!("{} (loading)", s.unwrap_or("?")))
                            }
                            (_, Some(s @ STUCK_COMBAT_ROOM)) => WedgeReason::StuckScreen(s.into()),
                            _ => WedgeReason::TransitionTimeout,
                        };
                        return Err(BridgeError::Wedged { screen, reason });
                    }
                    other => {
                        other?;
                    }
                }
            }
            Phase::Unknown => {
                return Err(BridgeError::NotSettled {
                    waited: started.elapsed(),
                    screen: probe.screen,
                    unmet: "a screen ensure_main_menu knows how to leave".to_string(),
                })
            }
            Phase::MenuSubscreen | Phase::InRun | Phase::GameOver => {
                steps += 1;
                if steps > MAX_NAV_STEPS {
                    return Err(BridgeError::NotSettled {
                        waited: started.elapsed(),
                        screen: probe.screen,
                        unmet: format!("{GOAL} within {MAX_NAV_STEPS} menu steps"),
                    });
                }
                let (nav, expected): (MenuNav, &str) = match phase {
                    Phase::InRun => (MenuNav::AbandonRun, "the game-over screen"),
                    Phase::GameOver => (MenuNav::LeaveGameOver, "the main menu"),
                    _ => (MenuNav::Back, "the menu screen to change"),
                };
                prober.send(nav)?;
                // Any move counts: the next turn of the loop takes it from
                // there (e.g. waits out a run tearing down, up to `run_load`).
                let moved = |p: &Probe| p.screen != probe.screen || classify(p) != phase;
                match prober.wait(&within(timeouts.step), expected, moved) {
                    Err(BridgeError::NotSettled { screen, .. })
                        if prober.last_probe.as_ref() == Some(&probe) =>
                    {
                        return Err(BridgeError::PostconditionFailed {
                            action: Command::Navigate(nav).to_string(),
                            expected: expected.to_string(),
                            observed: format!(
                                "still on {}",
                                screen.as_ref().map_or("no screen", Screen::as_str)
                            ),
                        });
                    }
                    other => {
                        other?;
                    }
                }
            }
        }
    }
}
