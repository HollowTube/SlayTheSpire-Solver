# sts-bridge

Typed interface to a live Slay the Spire 2 game. Consumers use the `Backend`
trait (`state()`, `actions()`, `health()`/`ping()`) and the `GameState` model;
`McpTestBackend` implements them over the MCPTest bridge (newline-delimited
JSON-RPC on TCP port 21337, host from `STS2_BRIDGE_HOST`, default `127.0.0.1`).

Every bridge failure is a typed `BridgeError` (`Refused`, `Timeout`, `Io`,
`Remote`, `Protocol`, `Schema`), never a default or partial state.

## Tests

```bash
# Offline: recorded fixtures + a local TCP stub. No game needed.
cargo test -p sts-bridge

# Live contract tests: opt-in, serial. Position the game first (main menu or a
# combat on the player's turn); an unreachable bridge fails the test.
cargo test -p sts-bridge --test live_contract -- --ignored --test-threads=1

# Regenerate the published schema after changing GameState.
UPDATE_SCHEMA=1 cargo test -p sts-bridge --test offline

# Record the current screen (main menu or combat) into tests/fixtures/mcptest/recorded/.
STS2_RECORD=1 cargo test -p sts-bridge --test live_contract record_fixtures -- --ignored
```

Run the tests with `-p sts-bridge`: a bare `cargo test` at the workspace root
also tries to link the pyo3 `sts-sim` cdylib.

`combat_intent_totals_are_consistent` is an expected failure (`should_panic`)
until HOL-102 fixes multi-hit intent `total_damage` in the bridge.

The schema for agents is `schema/game_state.schema.json`.
