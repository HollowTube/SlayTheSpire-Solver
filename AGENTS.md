# AGENTS.md

This file provides guidance to coding agents (Claude Code and others) when working with code in this repository.

## Agent skills

### Issue tracker

Issues and PRDs are tracked in Linear (Hollowtube team → "Slay The Spire Solver" project). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical role names are used as-is (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Live game interaction

Prefer the `sts2` CLI over `mcp__sts2-modding__*` tools for reading and controlling a live game. The CLI is the stable interface; the MCP tools are lower-level and more verbose. Use `sts2 combat`, `sts2 actions`, `sts2 act <n>` for the common play loop. Fall back to MCP tools only when the CLI doesn't expose what you need.

Running `integtests/` (`pytest integtests/ -m live -v`) against a real game requires settle-polling screen transitions rather than fixed sleeps — a console command issued mid-transition can permanently wedge the game's `CombatManager` until a full relaunch. See the README's "Live integration tests" section and `integtests/conftest.py`'s `_wait_settled`/`_stable_piles` helpers before adding new fixture code that issues console commands after a screen change.

---

## Development workflow

### 1. Start from a Linear issue

Pick a `ready-for-agent` issue and read its full description. Agents read and write the tracker through the code-task pipeline's `tracker.py` (see `docs/agents/issue-tracker.md`), not the Linear MCP tools. Move the issue to In Progress when you start.

### 2. Create a worktree

Work in an isolated git worktree so changes don't touch the main checkout:

```bash
# From the repo root — name it after the issue
git worktree add .claude/worktrees/<branch-name> -b <branch-name>
cd .claude/worktrees/<branch-name>

# Build the extension in the worktree's own venv
scripts/dev-setup.sh
source .venv/bin/activate
```

Or use the `EnterWorktree` tool in Claude Code to create and switch into a worktree automatically.

### 3. Implement with /tdd

Use `/tdd` to work in the red → green loop:

- Write one failing test at the public seam (pytest file in `tests/`)
- Write only enough code to pass it
- Repeat per slice — don't bulk-write tests ahead of implementation

Run the loop locally:
```bash
pytest tests/test_<feature>.py -q          # red
# ... implement ...
pytest tests/test_<feature>.py -q          # green
pytest tests/ -q                           # full suite — confirm no regressions
```

Always run `cargo check` after Rust changes before running pytest (catches type errors without a full maturin rebuild).

### 4. Open a draft PR

When the implementation is ready:
```bash
git add <files>
git commit -m "feat(...): ..."
git push -u origin <branch-name>
gh-axi pr create --title "..." --body "..." --draft
```

Put `Linear: <issue url>` in the PR body so the GitHub integration links it, and move the issue to In Review with `tracker.py move`. `tracker.py` lives at `~/.claude/skills/code-task/scripts/tracker.py` and only dry-runs unless `--apply` is passed.

### 5. Babysit CI with gh-axi

```bash
# Watch the latest run on your branch
gh-axi run list --branch <branch-name>

# See what failed
gh-axi run view <run-id> --log-failed

# After fixing, rerun only failed jobs
gh-axi run rerun <run-id> --failed
```

Your job ends at a draft PR with green CI. Leave it a draft: a human marks it ready (`gh-axi pr ready`) and merges it (`gh-axi pr merge --squash`). The merge closes the Linear issue automatically; don't move it to Done.

---

## Build and development commands

All commands assume the virtualenv is active (`source .venv/bin/activate`). Create it with `scripts/dev-setup.sh`; `scripts/verify.sh` runs every CI check (it needs no activation and works from any directory).

```bash
# Build Rust extension and install (required after any Rust change)
uv pip install -e ".[dev]"

# All CI checks in one go (gen_ids, cargo check, ruff, mypy, pytest, cargo test)
scripts/verify.sh

# Faster rebuild after changing Rust code only
maturin develop

# Run the full test suite
pytest tests/ -q

# Run a single test file or specific test
pytest tests/test_combat_loop.py -q
pytest tests/test_combat_loop.py::test_jaw_worm_hp -q

# Lint and format
ruff check python/ tests/
ruff format python/ tests/       # auto-fix
ruff format --check python/ tests/  # CI check

# Type-check
mypy python/ tests/ --ignore-missing-imports

# Validate the registry stub (_registry.pyi) is up to date
python scripts/gen_ids.py --check   # fails if stale
python scripts/gen_ids.py           # regenerate in place

# Cargo check (Rust only, no Python build) — covers sts-sim, crates/sts-ids and crates/sts-bridge
cargo check

# Registry and bridge crate tests (offline; live bridge ones are opt-in — see crates/sts-bridge/README.md)
cargo test -p sts-ids -p sts-bridge

# Restart analysis server (pull latest, rebuild Rust if src/ changed, kill old, start fresh)
./scripts/start-server.sh
./scripts/start-server.sh --no-pull   # skip git pull during active development
```

PyO3 0.23 supports Python ≤ 3.13, so the venv is 3.12 (CI's version); a bare `uv venv` can pick a newer system Python and the build fails. For a manual `cargo check`, set `PYO3_PYTHON` to the venv Python as an absolute path (`verify.sh` does): pyo3's build script runs from another directory, so a relative path fails. A root `cargo test` doesn't link (the root crate is a pyo3 `extension-module`), hence the `-p` crate names above.

The `.venv` must be active for `pytest`, `ruff`, `mypy`, `sts-sim`, and `sts2`. The Rust extension (`_sts_sim.*.so`) is required to import `sts_sim` — tests fail at collection without it.

## Architecture

### Two-layer design: Rust engine + Python harness

The engine is a Rust `cdylib` exposed to Python via PyO3. Python drives everything above the combat loop (scenarios, benchmarking, MCTS policy, CLI, bridge overlay).

```
src/           ← Rust (compiled to python/sts_sim/_sts_sim.*.so)
python/sts_sim/ ← Python (pure)
tests/          ← pytest (require the .so to be built)
```

### Rust layer (`src/`)

| File | Role |
|---|---|
| `ids.rs` | Re-exports `CardId`, `MonsterId`, `StatusId` from the registry crate `crates/sts-ids` (see "The registry" below). |
| `state.rs` | `CombatState`, `Monster`, `Fighter`, `CardInstance` — all game state. Cloned on every action (persistent immutable-ish tree). |
| `engine.rs` | `Status` enum (the canonical source of truth for in-game statuses), `EffectOp` pipeline, `GameEvent` reactions, and `fire_event`. |
| `cards.rs` | Card data table (`card_data(id, upgrade_level) → CardData`) and `card_pool()`, the reward pool derived from the registry. |
| `monsters.rs` | Monster AI: `opening_intent`, `monster_move` (per-move effects), `select_next_intent`. Add a new monster by adding arms to all three functions. |
| `mcts.rs` | Rayon-parallel MCTS: `fight_outcomes_per_fight`, `mcts_search`, etc. |
| `action.rs` | PyO3 action types (`EndTurnAction`, `PlayCardAction`, `SelectTargetAction`). |
| `lib.rs` | PyO3 module entry point. Houses `apply_action`, `legal_actions_typed`, `optimal_value_rec`, and the `#[pymodule]` registration block. |
| `run.rs` | Run-level state (`RunState`) and functions (`run_apply`, `run_is_terminal`, etc.) for multi-fight runs. |
| `act.rs` | Overgrowth encounter/elite pool draws (`draw_overgrowth_monster_sequence`, `draw_overgrowth_elite`). |
| `encounters.rs` | Encounter pool tables. |

### Python layer (`python/sts_sim/`)

| File | Role |
|---|---|
| `__init__.py` | Re-exports everything from `_sts_sim` (the compiled extension). |
| `sim/names.py` | `CardName`, `MonsterName`, `StatusName` str-enums + `card()`/`monster()` bridge-name normalisation. Built at import time from the registry rows by `sim/_registry.py` (types in the generated `_registry.pyi`). |
| `sim/scenarios.py` | Per-monster factory functions (`ironclad_starter_deck_vs_*(seed, deck=None) → CombatState`), hand-written. `MONSTER_STARTING_HP` is derived from the registry `hp` field. |
| `bench/__init__.py` | `_SCENARIOS` dict (`Encounter` → factory), `run_deck`, `compare`, and run-level benchmarks. `Encounter` comes from the registry. |
| `sim/mcts.py` | Pure-Python MCTS tree (UCB1 select/expand/backprop). |
| `sim/policies.py` | `simulate_run_with_reward_lookahead`. |
| `server.py` | FastAPI analysis server (receives live bridge state, returns MCTS action values). |
| `bridge/translate.py` / `bridge/client.py` | Translate live bridge JSON into `CombatState` / TCP client for the MCPTest bridge mod. |
| `bridge/cli.py` | `sts2` CLI — AXI-compliant wrapper around the bridge MCPTest protocol. |
| `cli.py` | `sts-sim` CLI — interactive and step-mode play. |
| `parity/` | Ironclad card parity: extracts card data from the decompiled game and the PCK into `tests/fixtures/game/<version>/`, and checks `card_spec` against it (`tests/parity/`). After a game patch: `scripts/card_parity.py extract`, then `diff`, then fix or file each mismatch (README "Card parity"). |

### The registry and monster registration

Cards, monsters and statuses are defined once, as rows in
`crates/sts-ids/src/registry.rs`. Rust uses the generated enums directly;
`_sts_sim.cards()`/`monsters()`/`statuses()` hand the rows to Python, where
`sim/_registry.py` builds `CardName`, `MonsterName`, `StatusName` and
`Encounter`. The only generated file is `sim/_registry.pyi`: run
`python scripts/gen_ids.py` after changing rows (and rebuilding); CI runs `--check`.

To add a monster, add its row (with `hp` and `encounter` if it has a
standalone scenario). Then `cargo check` names each `match` in `monsters.rs`
that needs an arm, and `tests/test_monster_registration.py` names the missing
scenario factory / `_SCENARIOS` entry. Multi-monster encounter keys are listed
in `MULTI_MONSTER_ENCOUNTERS` in `sim/_registry.py`; monsters with no
`encounter` (e.g. KinPriest, EyeWithTeeth) are covered by composite scenarios
or spawn mid-combat.

### Action model

Actions are strings at the Python boundary (`"EndTurn"`, `"PlayCard:Strike"`, `"SelectTarget:Monster:0"`) but `ActionKind` enum values inside Rust hot paths. `legal_actions()` returns typed PyO3 action objects; `apply()` accepts them. The MCTS and rollout paths stay in Rust, avoiding FFI per step.

### State is cloned on every action

`CombatState` is a value type cloned on each `apply()`. This enables tree search without undo. RNG state (`Pcg32`) is embedded in `CombatState` and drives shuffle/monster draws deterministically from the initial seed.

### Worktrees and the extension

Each git worktree needs its own venv (`scripts/dev-setup.sh`) because the compiled `.so` is placed inside the venv that was active at build time. `conftest.py` at the root inserts the worktree's own `python/` directory first on `sys.path` so local edits are visible without reinstalling.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
