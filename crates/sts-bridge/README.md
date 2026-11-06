# sts-bridge

Typed interface to a live Slay the Spire 2 game. Consumers use the `Backend`
trait (`state()`, `actions()`, `health()`/`ping()`, `send()`) and the
`GameState` model; `McpTestBackend` implements them over the MCPTest bridge
(newline-delimited JSON-RPC on TCP port 21337, host from `STS2_BRIDGE_HOST`,
default `127.0.0.1`).

Every bridge failure is a typed `BridgeError`, never a default or partial
state. `BridgeError` is `#[non_exhaustive]`: match the variants you handle and
keep a catch-all arm.

## Acting on the game

The bridge acknowledges a command before the game applies it (`play_card`
only enqueues the play), and a command sent mid-transition can wedge the
game's `CombatManager`. So writes go through a `Session`, which waits for the
game to settle before and after each command:

```rust
use sts_bridge::{Command, McpTestBackend, Session};

let mut game = Session::new(McpTestBackend::from_env());
let state = game.act(&Command::PlayCard { card_index: 2, target: Some(0) })?;
// `state` is settled and already shows the play's effects.
```

The shared primitives, for code that waits on or sends to the game:

| Item | Module | What it is |
|---|---|---|
| `SettlePolicy` | `settle` | `timeout`, `poll_min`/`poll_max` (backoff between reads), `stable_reads` (identical consecutive reads required, default 2). A plain value: pass tighter timeouts when `instant` is on. |
| `poll_until(policy, unmet, read, accept)` | `settle` | The one polling loop. Calls `read` until `accept` holds on `stable_reads` identical reads; any `T: Observation` (`GameState`, `Actions`, or your own type). A read error stops it; the deadline is `NotSettled`. Confirming reads come `poll_min` apart, so "stable" spans more than one frame; reads that miss back off up to `poll_max`. |
| `wait_until(backend, policy, unmet, accept)` | `settle` | `poll_until` over `Backend::state`, for any predicate (e.g. "the main menu, no run"). |
| `wait_settled(backend, policy)` | `settle` | `wait_until(is_interactive)`. |
| `is_interactive(&GameState)` | `settle` | Not loading, not `COMBAT_ENEMY_TURN`; in combat only `COMBAT_PLAYER_TURN` on the player's turn or an in-combat overlay (`HAND_SELECT`, `CARD_SELECTION`); any other screen out of combat. |
| `Session<B>` | `settle` | Owns a backend and a policy; `&mut self` everywhere, so one command is in flight. `act`, `act_with`, `wait_until`, `wait_settled`, `state`, `backend_mut` for raw reads and sends. |
| `Unmet { expected, observed }` | `settle` | What a postcondition reports when it does not hold. |
| `Command` | `command` | `#[non_exhaustive]`: `PlayCard { card_index, target }`, `EndTurn`, `Choose(AvailableAction)`. |
| `Backend::send(&Command) -> Ack` | `backend` | A raw send: returns when the backend accepted the command, not when it took effect. |
| `ReplayTransport::push` / `requests` | `mcptest` | Scripted offline transport: per-method reply queues (the last reply sticks) and a log of every request sent. |

`Session::act(command)`:

1. waits for a settled, interactive state;
2. checks the command against it (unplayable card, bad target, index out of
   range, action not on offer, not the player's turn) and refuses it without
   sending;
3. sends it, then waits for a stable, interactive state where its
   postcondition holds, and returns that state.

| Command | Postcondition (or the combat ended) |
|---|---|
| `PlayCard` | The copies of the card's id across hand, draw, discard and exhaust are back to at least what they were, and the combat changed. While a card resolves, it is in none of the piles the bridge reports. The game moves it to its result pile only after its effects and `AfterCardPlayed` hooks, so by then its damage and debuffs are visible. Draws, copies and exhausts of the same id don't break this. A `Power` card instead loses one copy in total, which the piles alone can't tell apart from "still resolving"; check its effect with `act_with`. |
| `EndTurn` | The round went up and the screen is `COMBAT_PLAYER_TURN` again, so the enemy turn has resolved. |
| `Choose` | The state or the actions on offer changed (both read on the same screen). A `Choose` of a listed `play_card`/`end_turn` is treated as that command. |

A `PlayCard` or `EndTurn` that opens an in-combat selection (`HAND_SELECT`,
`CARD_SELECTION`) returns on it: the rest of its effect waits for the
caller's `Choose`.

`act_with(command, check)` adds a caller postcondition on `(before, after)`,
e.g. a predicted state delta.

Errors from `act`:

- `ActionRefused { action, reason }`: rejected by the local check or by the
  bridge's method handler (its `error`, or `success: false`). Nothing
  happened. A request-level bridge failure (e.g. "Main thread dispatch timed
  out") stays `Remote`, because the command may still run.
- `PostconditionFailed { action, expected, observed }`: the game settled, but
  the effect did not happen (e.g. a play the game silently cancelled). Waits
  the full timeout first, since an effect may still be on its way.
- `NotSettled { waited, screen, unmet }`: no stable interactive state within
  the timeout; names the last screen and what never held.

A bridge read that fails during a wait (`Timeout`, `Remote`, …) is returned
as is; deciding that the game is wedged is up to the caller.

`is_interactive` is false for `MAIN_MENU` while a run is in progress: that is
a run loading or tearing down, not a menu.

## Loading, the main menu and the game's lifecycle

**Never send a room-changing command (console `fight`, `room`, `event`, …)
while a run is loading: it crashes the game (SIGSEGV).** After `start_run`
the bridge reports `MAIN_MENU` with `run_in_progress: true`, stably, for 30 to
50 s; then `MAP` derived from the run state (no room yet); then the first
room. `get_run_state` already says `in_progress: true` during it, so
`state()` reads as a main menu with a run. Call `wait_run_ready` first.

```rust
use sts_bridge::{Game, RunSpec};

let mut game = Game::from_env();          // MCPTest bridge + Steam launcher
game.start()?;                            // launches through Steam if needed
let run = game.start_run(&RunSpec::new("Ironclad", "S"))?;
// main menu (verified), `start_run`, then waited until ready and checked to
// be that run: only now may a console `fight` go out
```

`Game::start_run(&RunSpec)` goes through `ensure_main_menu` (abandoning a
run in progress), sends `Command::StartRun` only on the verified main menu,
waits with `wait_run_ready`, and checks `run.seed`, `run.character` and
`run.ascension` (`PostconditionFailed` if the run is another one). It
returns `StartedRun { state, accepted_after, ready_after }`. `Session::act`
refuses `StartRun`: the run loads for longer than a settle wait.

| Item | Module | What it is |
|---|---|---|
| `Probe { ok, screen, run_in_progress, in_combat, detail: Option<ScreenDetail>, main_menu_node }` | `menu` | One cheap read: `ping`, `get_screen` (`ScreenDetail { screen, source, room_type, context }`) and, on a no-run `MAIN_MENU`, whether GodotExplorer finds the `MainMenu` node. An `Observation`; equal reads compare equal. |
| `Backend::probe() -> Result<Probe>` | `backend` | Reads a `Probe`; sends nothing. |
| `classify(&Probe) -> Phase` | `menu` | The one decision of where the game is. `Phase` (`#[non_exhaustive]`): `Loading` (any `*LOADING*` screen; `MAIN_MENU` with a run in progress; `MAP` from `screen_source: run_state` in a run), `MainMenu` (`MAIN_MENU`, no run: a candidate only, see below), `MenuSubscreen`, `GameOver` (`GAME_OVER`, or the `OVERLAY` whose context is `NGameOverScreen`, which is what the current mod reports), `Transition` (enemy turn, `MENU_NCombatRoom`, other `COMBAT_*`, ping and `get_screen` disagreeing), `InRun` (a stable interactive in-run screen), `Unreadable` (ping without a screen), `Unknown`. |
| `wait_run_ready(backend, &Timeouts) -> Result<Probe>` | `menu` | Waits, sending nothing, for `InRun` on 3 stable probes spanning at least 1 s, up to `run_load`. The precondition for every room-changing command. `NotSettled` at the deadline (names the last screen); `Wedged` if the bridge stops answering. |
| `ensure_main_menu(backend, &Timeouts) -> Result<Probe>` | `menu` | From any screen to a **verified main menu**: `MainMenu` on stable probes, the `MainMenu` node present, and `state().run == None`. Waits out `Loading` and `Transition`; abandons a run from `InRun`; leaves `GameOver`; goes back from a menu screen; sends nothing on the main menu. At most `MAX_NAV_STEPS` (8) commands. |
| `Command::Navigate(MenuNav)` | `command` | `MenuNav::{AbandonRun, Back, LeaveGameOver}`. `LeaveGameOver` tries `navigate_menu return_to_main_menu`; on the current mod's "Unknown target" it remembers that and calls the game-over screen's `ReturnToMainMenu` through GodotExplorer (port 27020) instead. `Session::act` refuses a `Navigate` while loading (`ActionRefused`, "room still loading"). |
| `McpTestBackend::request(method, params) -> Result<Value>` | `mcptest` | A raw, decoded bridge call: unverified, for test positioning only. |
| `McpTestBackend::with_explorer(transport)` | `mcptest` | The GodotExplorer client (`from_env` sets it up on port 27020). Without one, or if it refuses, the main-menu check and `LeaveGameOver` fail with `ExplorerUnavailable`. |
| `Game<B, L: Launcher>` | `lifecycle` | `start`, `stop`, `restart`, `status() -> GameStatus`, `ensure_main_menu`, `wait_run_ready`, `recover(&cause) -> Recovered`, `recoveries()`, `backend_mut`. `Game::from_env()` on Unix. |
| `Launcher` / `SteamLauncher` | `lifecycle` | `launch`, `pid`, `kill(pid, grace)`. `SteamLauncher` runs `steam -applaunch 2868840` (`STS2_STEAM_APP_ID`, or the whole command from `STS2_LAUNCH_CMD`), finds `SlayTheSpire2` in `/proc`, and stops it with SIGTERM, then SIGKILL. |
| `GameStatus` | `lifecycle` | `NotRunning`, `Starting { pid }`, `Loading(Probe)`, `Ready(Probe)`, `Wedged { reason, screen }`, `Crashed { pid, last_screen }`. |
| `Timeouts` | `lifecycle` | `boot` 120 s, `run_load` 300 s, `step` 30 s, `ensure_total` 420 s, `stuck_screen` 15 s, `kill_grace` 10 s, `exit` 30 s, and `probe` (the poll policy: 500 ms, 3 stable reads). `with_*` builders; `uniform` for scripted tests. |

`start()` launches the game only if no `SlayTheSpire2` process exists, and
then waits for a verified main menu; on a game that is already running it
returns once the bridge answers, without relaunching or sending anything.
A game that is still booting (process up, bridge refusing) is waited for,
never restarted.

`SteamLauncher` sees only local processes: `Game` assumes the game runs on
this machine. With `STS2_BRIDGE_HOST` pointing at another host, use the
`menu` functions over `McpTestBackend` directly, not `Game`.

New errors (`backend`):

- `Wedged { screen, reason: WedgeReason }`: `BridgeTimeout` (accepted, never
  answered: e.g. a SIGSTOPped game), `MainThreadTimeout`, `PingWithoutScreen`,
  `StuckScreen(screen)` (`MENU_NCombatRoom` past `stuck_screen`),
  `TransitionTimeout` (the same transition screen held past `step`).
- `Crashed { pid, last_screen }`: the process `Game` was driving is gone and
  the bridge refuses or resets the connection.
- `Lifecycle { op, message }`: launching or killing failed.
- `ExplorerUnavailable { addr, reason }`: GodotExplorer is not reachable.

`Game::ensure_main_menu` and `Game::wait_run_ready` call `recover` once, and
only on `Wedged` or `Crashed`: it waits up to `exit` for a crashed process
that still lingers (its core dump being written), kills what is left,
relaunches and runs `ensure_main_menu`. A second failure is returned, not
retried. `NotSettled`, `PostconditionFailed` and `ActionRefused` never
restart the game: they are bugs a relaunch would hide. `Game::wait_run_ready`
after a recovery fails with `NotSettled`, since the run it waited for is gone.

## Console commands

`Session::console(&ConsoleCommand)` runs a dev console command and returns only
when the state afterwards shows its effect. The game's console reports success
for commands that did nothing, and the bridge drops its result, so the reply is
never trusted: a command that reports success and changes nothing is
`PostconditionFailed` naming the field (`piles.hand`, `enemies[0].hp`, …).

```rust
use sts_bridge::{CardId, CombatPile, ConsoleCommand, Session, NIBBITS_WEAK};

let mut game = Session::new(backend);
game.console(&ConsoleCommand::Fight(NIBBITS_WEAK))?;            // waits for a held in-run screen first
game.console(&ConsoleCommand::AddCard { card: CardId::Bash, pile: CombatPile::Hand })?;
game.set_instant(true)?;                                        // a set state, never a blind toggle
```

| Item | What it is |
|---|---|
| `ConsoleCommand` | `#[non_exhaustive]`, typed for what fixtures need: `Fight`, `Damage`, `DamageAllEnemies`, `HealPlayer`, `Block`, `Energy`, `Power`, `AddCard`, `RemoveCardFromHand`, `RemoveCardFromDeck` (the master deck, in or out of combat; checked against `RunInfo.deck`), `RelicAdd`, `RelicRemove`, `Draw`, `Upgrade`, `Win`. `line()` renders the exact console string. `Probe(String)` sends a raw line (its first word must be in the catalog) and passes only if the state changed: for checking a no-effect path. `ToggleInstant` exists but `console` refuses it. |
| `Target` | `Player` or `Enemy(i)`. The console indexes creatures as the player first, then enemies (`damage`, `block`, `power`: `1 + i`); `heal` is player-only here and `kill` is not typed. |
| `Encounter`, `PowerName`, `Relic` | Id newtypes, checked against the catalog (`UnknownId`); `NIBBITS_WEAK` is a const. A card is a `CardId` and is checked against the catalog's card ids too. |
| `Session::console` / `console_with(cmd, &ConsoleWait)` | `act` for the console, under a longer timeout (`CONSOLE_TIMEOUT`, 45 s) because a state read is five bridge calls at 0.7 to 1 s each while the game is unfocused. The session's own policy is restored on every path. A room-changing command first waits for a stable in-run screen (`wait_run_ready`; its probe policy is the dwell). |
| `Session::set_instant(on)` / `set_instant_with` | Reads `Health.fast_mode`, sends nothing if it already matches, otherwise sends one toggle and waits for the mode to read back. `ActionRefused` if the bridge does not report `fast_mode`. |
| `RunInfo.relics`, `RunInfo.deck`, `Health.fast_mode` | Class names of the player's relics and the master deck (both read from `get_player_state`, one more call per `state()`); instant mode when the bridge reports it. |

Preconditions are checked on the settled state and refuse without sending (no
run, not the player's turn, card not in hand, hand full, dead or missing
target, `power` amount 0, `upgrade` of an upgraded card). Every console command
is also refused while `classify` says a run is loading, **including the first
read**: a room-changing command sent then crashes the game (SIGSEGV), and
`fight`, `event`, `room`, `act` and `ancient` are the commands that change rooms
(`enters_room` in the catalog, found by the extractor).

What no console command can do (so no fixture can): raise an enemy's hp or set
an exact hp, clear block, reduce energy, remove a power or use a negative
amount, remove from the draw, discard or exhaust piles, pick an upgraded copy,
add an already upgraded card, set the round, or set an enemy's intent.
`remove_card X discard` is rejected by the game while the bridge says success.

## Verified combat fixtures

A test declares the combat it needs as data; `Fixtures::ensure` returns a
state proven equal to it, from any starting state.

```rust
use sts_bridge::{CardId, CardSpec, EnemySpec, Fixture, Fixtures, Game, Hp, NIBBITS_WEAK};

let mut fixtures = Fixtures::new(Game::from_env());
let fixture = Fixture::new(NIBBITS_WEAK)          // Ironclad, seed HOL108FIX, ascension 0
    .enemy(EnemySpec::new(20))                    // one spec per enemy the encounter spawns
    .player_hp(Hp::Exact(60))
    .hand([CardSpec::from(CardId::Bash), CardSpec::upgraded(CardId::StrikeIronclad)])
    .draw([CardId::DefendIronclad]);              // top first
let verified = fixtures.ensure(&fixture)?;        // VerifiedState { state, path, timings, instant }
```

Every field has a deterministic default (player at max hp with the turn's
energy, no block or powers, the starting relic, every pile empty), so a field
a test leaves out is not left to chance. Piles are exact and ordered; relics
are an exact multiset; an enemy's powers are `AsSpawned` unless declared.

- **Per session** (and after every reset): `Game::start`; the fixture's
  seeded run, or a full reset into it; `set_instant(true)`, recorded as
  `InstantMode::Unverified` when the bridge does not report `fast_mode`
  (never toggled blind); and the master deck emptied with
  `RemoveCardFromDeck`, so every fight opens with empty piles and no
  opening-draw race. If the deck cannot be emptied, `plan` clears each fight's
  piles instead. The fixture's run is therefore deliberately not a normal
  run.
- **Per `ensure`**: a static check that sends nothing; relics normalised
  before the fight (so combat-start effects apply); `fight`; the steps `plan`
  gives (player hp, enemy hp, powers, block, energy, then piles), each sent
  through `Session::console` and verified; then `Fixture::diff` on the last
  settled state. On a mismatch or a failed step: one full seeded reset
  (`Game::start_run`) and a second attempt. Teardown is the next setup.

Errors (`FixtureError`, `#[non_exhaustive]`): `Unreachable { fields, needs }`
(nothing sent), `Mismatch { fields: Vec<FieldDiff>, after_reset }` naming each
field (`player.energy`, `enemies[0].hp`, `enemies[0].powers[VULNERABLE_POWER]`,
`piles.draw`, `run.relics`, …), each `FieldDiff` marked `needs_setter` when no
console command can reach it, and `Bridge { source, after_reset }`.

What the console reaches from a fresh fight on a blank deck: player hp up to
`max_hp`; block (from 0); enemy hp at or below its spawn; energy at or above
the start; powers added on top of the spawn's; hand, draw (appended at the
bottom, so declared order holds), discard and exhaust exactly; relics. The
rest (enemy hp above the spawn, energy below the start, lowering a power,
upgraded cards outside the hand, round, intent, enemy `max_hp`) waits for
`Backend::setters()`: a `#[non_exhaustive]` `Setters` of capability flags,
all off by default. A flag set there makes `plan` treat its field as
reachable; the step that sets it comes with the backend that sets the flag.
Intent and enemy `max_hp` are not fixture fields and vary between fights.

`fixture.project(&state)` is the declared fields' values, for checking that
the same fixture yields the same state every time.

### The command catalog

`catalog/<version>/console_commands.json` lists every console command of one
game version (name, usage, description, networked, debug-only, `enters_room`, a
hash of its `Process` body) and the id domains (cards, encounters, powers,
relics, piles). It is generated from the decompiled game and the PCK's
`release_info.json`, never edited by hand, and holds no game code:

```bash
.venv/bin/python scripts/console_catalog.py extract --game-version 0.107.1 \
    --decompiled ~/projects/sts2-modding-mcp/decompiled \
    --pck "<Steam>/Slay the Spire 2/SlayTheSpire2.pck"
.venv/bin/python scripts/console_catalog.py diff old.json new.json   # exit 1 on any change
```

`build.rs` compiles it into one `const CommandSpec` per command
(`console::catalog`). Each constructor names its const and pins its usage
string, so on patch day: extract the new version's catalog, bump `GAME_VERSION`
in `build.rs`, and `cargo check -p sts-bridge` names each command whose name
or usage changed. CI never regenerates the catalog (the decompiled game is
local); it builds and tests against the committed file.

## Tests

```bash
# Offline: fixtures, scripted replies and a local TCP stub. No game needed.
cargo test -p sts-bridge

# Live contract tests: opt-in, serial. Position the game first (main menu or a
# combat on the player's turn); an unreachable bridge fails the test.
cargo test -p sts-bridge --test live_contract -- --ignored --test-threads=1

# Live lifecycle tests: opt-in, serial, slow (each starts runs; on a slow
# machine about 1 s per bridge call). They need Steam logged in and both mods;
# they launch the game, and kill (SIGKILL) and freeze (SIGSTOP) it to test
# recovery.
cargo test -p sts-bridge --test live_lifecycle -- --ignored --test-threads=1

# Live console tests: opt-in, serial, slow (each starts runs). They set the game
# up themselves: Game::start, ensure_main_menu, a seeded start_run,
# wait_run_ready, then Session::console(Fight(NIBBITS_WEAK)). --nocapture
# prints each command's result and duration as it happens.
cargo test -p sts-bridge --test live_console -- --ignored --test-threads=1 --nocapture

# Live fixture tests: opt-in, serial, slow (about 10-15 min on a focused game;
# far longer unfocused). They time five state reads first and, on a slow
# bridge, print SLOW BRIDGE and report the timing criteria instead of
# asserting them. Each ends on the main menu.
cargo test -p sts-bridge --test live_fixture -- --ignored --test-threads=1 --nocapture

# Regenerate the published schema after changing GameState.
UPDATE_SCHEMA=1 cargo test -p sts-bridge --test offline

# Record the current screen (main menu or combat) into tests/fixtures/mcptest/recorded/.
STS2_RECORD=1 cargo test -p sts-bridge --test live_contract record_fixtures -- --ignored
```

The `act_*` live tests need an Ironclad combat on the player's turn (a
seeded `start_run`, then the console `fight NIBBITS_WEAK`) and set up hand,
energy and enemy block through raw console commands. Let the run's first
screen settle before `fight`: sent on the opening `EVENT` a few seconds after
`start_run` (without a settle check), it crashed the game with a SIGSEGV.

`tests/fixture_offline.rs` runs fixtures against `FakeGame`
(`tests/common/fake_game.rs`), an in-memory `Backend` with the console's
effects and limits, a loading phase after `start_run`, seeded spawn hp and an
opening draw; it panics on any command sent while a run loads.

`tests/settle.rs` scripts state sequences by editing the synthesized
`combat` fixture (`tests/common/mod.rs`, `WireState`). `tests/lifecycle.rs`
scripts screens with `Scene` (ping, `get_screen` and `find_nodes` replies in
the shapes recorded from the live game), a `FaultTransport` that wraps the
replay and can refuse or time out, and a `FakeLauncher`.

Run the tests with `-p sts-bridge`. A workspace-root `cargo test` also works:
the `sts-sim` library has no Python, and the pyo3 extension (`crates/sts-py`)
has no test target.

`combat_intent_totals_are_consistent` is an expected failure (`should_panic`)
until HOL-102 fixes multi-hit intent `total_damage` in the bridge.

The schema for agents is `schema/game_state.schema.json`.

## Parity suite

`crates/sts-parity` builds on `Fixtures::ensure` and `Session::act` to compare the sim against the game card by card; see the repository README, "Live integration tests".
