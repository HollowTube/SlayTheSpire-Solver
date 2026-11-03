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

## Tests

```bash
# Offline: fixtures, scripted replies and a local TCP stub. No game needed.
cargo test -p sts-bridge

# Live contract tests: opt-in, serial. Position the game first (main menu or a
# combat on the player's turn); an unreachable bridge fails the test.
cargo test -p sts-bridge --test live_contract -- --ignored --test-threads=1

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

`tests/settle.rs` scripts state sequences by editing the synthesized
`combat` fixture (`tests/common/mod.rs`, `WireState`).

Run the tests with `-p sts-bridge`: a bare `cargo test` at the workspace root
also tries to link the pyo3 `sts-sim` cdylib.

`combat_intent_totals_are_consistent` is an expected failure (`should_panic`)
until HOL-102 fixes multi-hit intent `total_damage` in the bridge.

The schema for agents is `schema/game_state.schema.json`.
