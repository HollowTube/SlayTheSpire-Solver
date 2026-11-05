//! Owning the game process: start it, stop it, tell whether it is up, and
//! recover it from a wedge or a crash with a restart.

use std::time::{Duration, Instant};

use crate::backend::{Backend, BridgeError, WedgeReason};
use crate::command::{Command, RunSpec};
use crate::menu::{self, classify, Phase, Probe};
use crate::settle::{poll_until, refuse_while_loading, run_is, Observation, SettlePolicy};
use crate::state::{GameState, Screen};

/// Deadlines for lifecycle and navigation waits. Sized for a slow machine:
/// a fresh run took 31 to 75 s from `start_run` to its first room, and the
/// first run after a cold launch more than 180 s.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Timeouts {
    /// Launch to a verified main menu.
    pub boot: Duration,
    /// A run loading, to [`Phase::InRun`].
    pub run_load: Duration,
    /// One navigation step to a visible change; and the longest a transition
    /// screen (e.g. the enemy turn) may hold before it is a wedge.
    pub step: Duration,
    /// One whole [`menu::ensure_main_menu`].
    pub ensure_total: Duration,
    /// `MENU_NCombatRoom` or a screenless ping held this long is a wedge.
    pub stuck_screen: Duration,
    /// SIGTERM to SIGKILL.
    pub kill_grace: Duration,
    /// A dead process lingering (e.g. while its core dump is written), and a
    /// stopped one disappearing.
    pub exit: Duration,
    /// How probes are polled; its `timeout` is replaced by the deadlines above.
    pub probe: SettlePolicy,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            boot: Duration::from_secs(120),
            run_load: Duration::from_secs(300),
            step: Duration::from_secs(30),
            ensure_total: Duration::from_secs(420),
            stuck_screen: Duration::from_secs(15),
            kill_grace: Duration::from_secs(10),
            exit: Duration::from_secs(30),
            probe: SettlePolicy::default()
                .with_poll(Duration::from_millis(500), Duration::from_secs(1))
                .with_stable_reads(3),
        }
    }
}

macro_rules! with {
    ($($name:ident: $field:ident),* $(,)?) => {
        $(
            pub fn $name(mut self, d: Duration) -> Self {
                self.$field = d;
                self
            }
        )*
    };
}

impl Timeouts {
    with!(
        with_boot: boot,
        with_run_load: run_load,
        with_step: step,
        with_ensure_total: ensure_total,
        with_stuck_screen: stuck_screen,
        with_kill_grace: kill_grace,
        with_exit: exit,
    );

    pub fn with_probe(mut self, policy: SettlePolicy) -> Self {
        self.probe = policy;
        self
    }

    /// Every deadline set to `d`, with fast polling: for scripted tests.
    pub fn uniform(d: Duration, poll: Duration) -> Self {
        Self {
            boot: d,
            run_load: d,
            step: d,
            ensure_total: d * 4,
            stuck_screen: d,
            kill_grace: d,
            exit: d,
            probe: SettlePolicy::default()
                .with_poll(poll, poll)
                .with_stable_reads(3),
        }
    }
}

/// Starts and stops the game process.
pub trait Launcher {
    /// Starts the game; returns without waiting for it to come up.
    fn launch(&mut self) -> Result<(), BridgeError>;
    /// The running game's process id, if one is running.
    fn pid(&mut self) -> Option<u32>;
    /// Asks `pid` to exit, and kills it after `grace`.
    fn kill(&mut self, pid: u32, grace: Duration) -> Result<(), BridgeError>;
}

/// Launches the game through Steam, so its launch options (e.g. a
/// `libgcc_s` preload) apply, and finds it by its process name.
///
/// The command is `steam -applaunch <STS2_STEAM_APP_ID, default 2868840>`,
/// or `STS2_LAUNCH_CMD` (split on whitespace) when set.
#[cfg(unix)]
#[derive(Debug, Clone)]
pub struct SteamLauncher {
    command: Vec<String>,
    process_name: String,
}

#[cfg(unix)]
impl SteamLauncher {
    pub const APP_ID: &'static str = "2868840";
    pub const PROCESS_NAME: &'static str = "SlayTheSpire2";

    pub fn from_env() -> Self {
        let command = match std::env::var("STS2_LAUNCH_CMD") {
            Ok(cmd) if !cmd.trim().is_empty() => {
                cmd.split_whitespace().map(str::to_string).collect()
            }
            _ => {
                let app = std::env::var("STS2_STEAM_APP_ID")
                    .ok()
                    .filter(|a| !a.is_empty())
                    .unwrap_or_else(|| Self::APP_ID.to_string());
                vec!["steam".into(), "-applaunch".into(), app]
            }
        };
        Self {
            command,
            process_name: Self::PROCESS_NAME.into(),
        }
    }

    pub fn command(&self) -> &[String] {
        &self.command
    }

    fn signal(pid: u32, signal: &str) -> Result<(), BridgeError> {
        let failed = |message: String| BridgeError::Lifecycle {
            op: "kill".into(),
            message: format!("kill {signal} {pid}: {message}"),
        };
        let out = std::process::Command::new("kill")
            .args([signal, &pid.to_string()])
            .output()
            .map_err(|e| failed(e.to_string()))?;
        // A process that exited meanwhile needs no signal.
        if !out.status.success() && Self::alive(pid) {
            return Err(failed(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        Ok(())
    }

    /// Alive and not a zombie.
    fn alive(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
            stat.rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().next())
                .is_some_and(|state| state != "Z" && state != "X")
        })
    }
}

#[cfg(unix)]
impl Launcher for SteamLauncher {
    fn launch(&mut self) -> Result<(), BridgeError> {
        let (program, args) = self
            .command
            .split_first()
            .ok_or_else(|| BridgeError::Lifecycle {
                op: "launch".into(),
                message: "the launch command is empty".into(),
            })?;
        std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| BridgeError::Lifecycle {
                op: "launch".into(),
                message: format!("`{}`: {e} (is Steam installed?)", self.command.join(" ")),
            })
    }

    fn pid(&mut self) -> Option<u32> {
        let entries = std::fs::read_dir("/proc").ok()?;
        entries
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
            .find(|&pid| {
                std::fs::read_to_string(format!("/proc/{pid}/comm"))
                    .is_ok_and(|c| c.trim_end() == self.process_name)
                    && Self::alive(pid)
            })
    }

    fn kill(&mut self, pid: u32, grace: Duration) -> Result<(), BridgeError> {
        Self::signal(pid, "-TERM")?;
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            if !Self::alive(pid) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        // A stopped process (SIGSTOP) never handles SIGTERM.
        Self::signal(pid, "-KILL")
    }
}

/// What [`Game::status`] sees.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum GameStatus {
    /// No game process, and the bridge refuses.
    NotRunning,
    /// A game process, but the bridge is not answering with a screen yet.
    Starting { pid: Option<u32> },
    /// A run is loading.
    Loading(Probe),
    /// The bridge answers with a screen.
    Ready(Probe),
    Wedged {
        reason: WedgeReason,
        screen: Option<Screen>,
    },
    /// The process we saw earlier is gone and the bridge refuses.
    Crashed {
        pid: Option<u32>,
        last_screen: Option<Screen>,
    },
}

/// A run [`Game::start_run`] started, loaded and verified.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct StartedRun {
    /// One read once the run held a stable in-run screen.
    pub state: GameState,
    /// From the call to the bridge accepting `start_run`.
    pub accepted_after: Duration,
    /// From the call to the run being ready for a room change.
    pub ready_after: Duration,
}

/// A [`Game::recover`] that relaunched the game.
#[derive(Debug, Clone, PartialEq)]
pub struct Recovered {
    /// The error that triggered it.
    pub cause: String,
    pub restarted: bool,
    /// The verified main menu it ended on.
    pub probe: Probe,
}

/// The game process behind a [`Backend`], with a [`Launcher`] to start and
/// stop it.
///
/// [`Game::ensure_main_menu`] and [`Game::wait_run_ready`] recover once, by
/// a restart, from [`BridgeError::Wedged`] or [`BridgeError::Crashed`], and
/// never from other errors, which are bugs a relaunch would hide.
pub struct Game<B: Backend, L: Launcher> {
    backend: B,
    launcher: L,
    timeouts: Timeouts,
    /// The game process we are driving, once seen.
    pid: Option<u32>,
    last_screen: Option<Screen>,
    recoveries: Vec<Recovered>,
}

#[cfg(unix)]
impl Game<crate::McpTestBackend<crate::mcptest::TcpTransport>, SteamLauncher> {
    /// The MCPTest bridge from the environment, launched through Steam.
    pub fn from_env() -> Self {
        Self::new(crate::McpTestBackend::from_env(), SteamLauncher::from_env())
    }
}

/// The bridge did not connect, or the connection broke: no process behind it.
fn is_disconnect(e: &BridgeError) -> bool {
    matches!(e, BridgeError::Refused { .. } | BridgeError::Io { .. })
}

/// A probe, or why there was none, as one pollable value.
#[derive(PartialEq)]
enum Boot {
    Up(Probe),
    Down(String),
}

impl Observation for Boot {
    fn screen(&self) -> Option<&Screen> {
        match self {
            Boot::Up(p) => p.screen.as_ref(),
            Boot::Down(_) => None,
        }
    }
}

impl<B: Backend, L: Launcher> Game<B, L> {
    pub fn new(backend: B, mut launcher: L) -> Self {
        let pid = launcher.pid();
        Self {
            backend,
            launcher,
            timeouts: Timeouts::default(),
            pid,
            last_screen: None,
            recoveries: Vec::new(),
        }
    }

    pub fn with_timeouts(mut self, timeouts: Timeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    pub fn timeouts(&self) -> &Timeouts {
        &self.timeouts
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// For reads and raw sends, e.g. test positioning.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn launcher_mut(&mut self) -> &mut L {
        &mut self.launcher
    }

    /// Every recovery so far, oldest first.
    pub fn recoveries(&self) -> &[Recovered] {
        &self.recoveries
    }

    fn probe(&mut self) -> Result<Probe, BridgeError> {
        let p = self.backend.probe()?;
        if p.screen.is_some() {
            self.last_screen = p.screen.clone();
        }
        Ok(p)
    }

    /// One look at the process and the bridge. Sends nothing.
    pub fn status(&mut self) -> Result<GameStatus, BridgeError> {
        let pid = self.launcher.pid();
        if pid.is_some() {
            self.pid = pid;
        }
        let status = match self.probe() {
            Ok(p) => match classify(&p) {
                Phase::Loading => GameStatus::Loading(p),
                Phase::Unreadable => GameStatus::Starting { pid },
                _ => GameStatus::Ready(p),
            },
            Err(e) if is_disconnect(&e) => match (pid, self.pid) {
                (Some(_), _) => GameStatus::Starting { pid },
                (None, Some(seen)) => GameStatus::Crashed {
                    pid: Some(seen),
                    last_screen: self.last_screen.clone(),
                },
                (None, None) => GameStatus::NotRunning,
            },
            Err(e) => match menu::as_wedge(e, self.last_screen.clone()) {
                BridgeError::Wedged { screen, reason } => GameStatus::Wedged { reason, screen },
                // Half up: what `start` waits through while booting.
                BridgeError::ExplorerUnavailable { .. }
                | BridgeError::Remote { .. }
                | BridgeError::Protocol { .. }
                    if pid.is_some() =>
                {
                    GameStatus::Starting { pid }
                }
                other => return Err(other),
            },
        };
        Ok(status)
    }

    /// What a bridge failure in the middle of an operation says about the
    /// game: `Wedged` when the bridge accepts but stops answering; `Crashed`
    /// when it stops connecting, once the process is gone or has lingered
    /// for `timeouts.exit` (a crashed process stays while its core dump is
    /// written). The error unchanged otherwise, e.g. `Refused` for a game
    /// never started, or a bridge that came back.
    fn lost(&mut self, error: BridgeError) -> BridgeError {
        if let Some(pid) = self.launcher.pid() {
            self.pid = Some(pid);
        }
        if !is_disconnect(&error) || self.pid.is_none() {
            return menu::as_wedge(error, self.last_screen.clone());
        }
        let deadline = Instant::now() + self.timeouts.exit;
        while self.launcher.pid().is_some() && Instant::now() < deadline {
            if self.backend.probe().is_ok() {
                return error;
            }
            std::thread::sleep(self.timeouts.probe.poll_max);
        }
        BridgeError::Crashed {
            pid: self.pid,
            last_screen: self.last_screen.clone(),
        }
    }

    /// Brings the game up and returns once it answers. If it is not running,
    /// launches it and waits, sending nothing, for a verified main menu. If
    /// it is already running, it is not relaunched: returns once the bridge
    /// answers with a screen (wherever the game is).
    pub fn start(&mut self) -> Result<Probe, BridgeError> {
        let deadline = Instant::now() + self.timeouts.boot;
        let launched = match self.launcher.pid() {
            Some(pid) => {
                self.pid = Some(pid);
                false
            }
            None => {
                self.launcher.launch()?;
                self.pid = None;
                true
            }
        };
        let policy = self.timeouts.probe.clone().with_timeout(self.timeouts.boot);
        let mut crashed = false;
        // The last read failed only because the explorer is unreachable.
        let mut explorer_down: Option<(String, String)> = None;
        let result = {
            let Self {
                backend,
                launcher,
                pid: seen,
                last_screen,
                ..
            } = self;
            poll_until(
                &policy,
                if launched {
                    "a verified main menu after launch"
                } else {
                    "the bridge to answer"
                },
                || {
                    let pid = launcher.pid();
                    if pid.is_some() {
                        *seen = pid;
                    }
                    let read = backend.probe();
                    explorer_down = match &read {
                        Err(BridgeError::ExplorerUnavailable { addr, reason }) => {
                            Some((addr.clone(), reason.clone()))
                        }
                        _ => None,
                    };
                    match read {
                        Ok(p) => {
                            if p.screen.is_some() {
                                *last_screen = p.screen.clone();
                            }
                            Ok(Boot::Up(p))
                        }
                        Err(e) if is_disconnect(&e) && pid.is_none() && seen.is_some() => {
                            crashed = true;
                            Err(e)
                        }
                        // Booting: the bridge or the explorer may not be up
                        // yet, or may answer half-initialized.
                        Err(e)
                            if is_disconnect(&e)
                                || matches!(
                                    e,
                                    BridgeError::Timeout { .. }
                                        | BridgeError::ExplorerUnavailable { .. }
                                        | BridgeError::Remote { .. }
                                        | BridgeError::Protocol { .. }
                                ) =>
                        {
                            Ok(Boot::Down(e.to_string()))
                        }
                        Err(e) => Err(e),
                    }
                },
                |b| match b {
                    Boot::Up(p) if launched => {
                        classify(p) == Phase::MainMenu && p.main_menu_node == Some(true)
                    }
                    Boot::Up(p) => classify(p) != Phase::Unreadable,
                    Boot::Down(_) => false,
                },
            )
        };
        let probe = match result {
            Ok(Boot::Up(p)) => p,
            Ok(Boot::Down(_)) => unreachable!("never accepted"),
            Err(_) if crashed => {
                return Err(BridgeError::Crashed {
                    pid: self.pid,
                    last_screen: self.last_screen.clone(),
                })
            }
            Err(BridgeError::NotSettled { .. }) if explorer_down.is_some() => {
                let (addr, reason) = explorer_down.unwrap_or_default();
                return Err(BridgeError::ExplorerUnavailable { addr, reason });
            }
            Err(e) => return Err(e),
        };
        if self.pid.is_none() {
            self.pid = self.launcher.pid();
        }
        if launched {
            let run = self.backend.state()?.run;
            if run.is_some() {
                return Err(BridgeError::NotSettled {
                    waited: self.timeouts.boot - deadline.saturating_duration_since(Instant::now()),
                    screen: probe.screen,
                    unmet: "a verified main menu (a run is in progress)".into(),
                });
            }
        }
        Ok(probe)
    }

    /// Stops the game; verified by no process and a refused bridge.
    pub fn stop(&mut self) -> Result<(), BridgeError> {
        if let Some(pid) = self.launcher.pid() {
            self.launcher.kill(pid, self.timeouts.kill_grace)?;
        }
        self.wait_gone()?;
        self.pid = None;
        Ok(())
    }

    fn wait_gone(&mut self) -> Result<(), BridgeError> {
        let deadline = Instant::now() + self.timeouts.exit + self.timeouts.kill_grace;
        loop {
            let pid = self.launcher.pid();
            let refused = matches!(self.backend.probe(), Err(ref e) if is_disconnect(e));
            if pid.is_none() && refused {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(BridgeError::Lifecycle {
                    op: "stop".into(),
                    message: match pid {
                        Some(p) => format!("process {p} is still running"),
                        None => "the process is gone but the bridge still answers".into(),
                    },
                });
            }
            std::thread::sleep(self.timeouts.probe.poll_min);
        }
    }

    /// [`Game::stop`], then [`Game::start`].
    pub fn restart(&mut self) -> Result<Probe, BridgeError> {
        self.stop()?;
        self.start()
    }

    /// Restarts the game after `cause` and brings it to a verified main menu.
    /// A crashed process that still lingers (its core being written) gets
    /// `timeouts.exit` to go before it is killed. Does not recover again if
    /// the restart fails.
    pub fn recover(&mut self, cause: &BridgeError) -> Result<Recovered, BridgeError> {
        if let Some(pid) = self.launcher.pid() {
            if matches!(cause, BridgeError::Crashed { .. }) {
                let deadline = Instant::now() + self.timeouts.exit;
                while self.launcher.pid().is_some() && Instant::now() < deadline {
                    std::thread::sleep(self.timeouts.probe.poll_min);
                }
            }
            if self.launcher.pid().is_some() {
                self.launcher.kill(pid, self.timeouts.kill_grace)?;
            }
        }
        self.wait_gone()?;
        self.pid = None;
        self.start().map_err(|e| self.lost(e))?;
        let probe =
            menu::ensure_main_menu(&mut self.backend, &self.timeouts).map_err(|e| self.lost(e))?;
        let recovered = Recovered {
            cause: cause.to_string(),
            restarted: true,
            probe,
        };
        self.recoveries.push(recovered.clone());
        Ok(recovered)
    }

    fn recovering<T>(
        &mut self,
        mut op: impl FnMut(&mut B, &Timeouts) -> Result<T, BridgeError>,
        after: impl FnOnce(&mut Self, Recovered) -> Result<T, BridgeError>,
    ) -> Result<T, BridgeError> {
        let outcome = match self.probe() {
            // A live process whose bridge does not connect yet is still
            // booting: wait for it rather than restart it.
            Err(e) if is_disconnect(&e) && self.launcher.pid().is_some() => self
                .start()
                .map_err(|e| self.lost(e))
                .and_then(|_| op(&mut self.backend, &self.timeouts).map_err(|e| self.lost(e))),
            // Already wedged: recover without waiting out the op's own reads.
            Err(e) if !is_disconnect(&e) => match self.lost(e) {
                wedged @ BridgeError::Wedged { .. } => Err(wedged),
                _ => op(&mut self.backend, &self.timeouts).map_err(|e| self.lost(e)),
            },
            _ => op(&mut self.backend, &self.timeouts).map_err(|e| self.lost(e)),
        };
        match outcome {
            Err(e @ (BridgeError::Wedged { .. } | BridgeError::Crashed { .. })) => {
                let recovered = self.recover(&e)?;
                after(self, recovered)
            }
            other => other,
        }
    }

    /// [`menu::ensure_main_menu`], recovering once from a wedge or a crash.
    pub fn ensure_main_menu(&mut self) -> Result<Probe, BridgeError> {
        self.recovering(
            |b, t| menu::ensure_main_menu(b, t),
            |_, recovered| Ok(recovered.probe),
        )
    }

    /// Starts the seeded run `spec` and returns once it is ready for a room
    /// change, verified to be that run (seed, character, ascension).
    ///
    /// Brings the game to a verified main menu first ([`Game::ensure_main_menu`],
    /// which abandons a run in progress), and sends `start_run` only there.
    /// It then waits, sending nothing, for the run to load
    /// ([`Game::wait_run_ready`]). A run that loads as another run is
    /// [`BridgeError::PostconditionFailed`].
    pub fn start_run(&mut self, spec: &RunSpec) -> Result<StartedRun, BridgeError> {
        let started = Instant::now();
        self.ensure_main_menu()?;
        let command = Command::StartRun(spec.clone());
        refuse_while_loading(&mut self.backend, &command).map_err(|e| self.lost(e))?;
        self.backend.send(&command).map_err(|e| self.lost(e))?;
        let accepted_after = started.elapsed();
        self.wait_run_ready()?;
        let state = self.backend.state().map_err(|e| self.lost(e))?;
        run_is(spec, &state).map_err(|unmet| BridgeError::PostconditionFailed {
            action: command.to_string(),
            expected: unmet.expected,
            observed: unmet.observed,
        })?;
        Ok(StartedRun {
            state,
            accepted_after,
            ready_after: started.elapsed(),
        })
    }

    /// [`menu::wait_run_ready`], recovering once from a wedge or a crash. A
    /// recovery ends on the main menu, so it then fails with `NotSettled`:
    /// the run it waited for is gone.
    pub fn wait_run_ready(&mut self) -> Result<Probe, BridgeError> {
        self.recovering(
            |b, t| menu::wait_run_ready(b, t),
            |_, recovered| {
                Err(BridgeError::NotSettled {
                    waited: Duration::ZERO,
                    screen: recovered.probe.screen,
                    unmet: format!(
                        "a stable in-run screen (the game was restarted after: {})",
                        recovered.cause
                    ),
                })
            },
        )
    }
}
