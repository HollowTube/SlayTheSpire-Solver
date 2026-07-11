# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Agent skills

### Issue tracker

Issues and PRDs are tracked in Linear (Hollowtube team → "Slay The Spire Solver" project). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical role names are used as-is (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Live game interaction

Prefer the `sts2` CLI over `mcp__sts2-modding__*` tools for reading and controlling a live game. The CLI is the stable interface; the MCP tools are lower-level and more verbose. Use `sts2 combat`, `sts2 actions`, `sts2 act <n>` for the common play loop. Fall back to MCP tools only when the CLI doesn't expose what you need.

---

## Development workflow

### 1. Start from a Linear issue

Pick a `ready-for-agent` issue via `mcp__linear-server__list_issues` (filter by label). Read its full description with `mcp__linear-server__get_issue`. Update status to In Progress.

### 2. Create a worktree

Work in an isolated git worktree so changes don't touch the main checkout:

```bash
# From the repo root — name it after the issue
git worktree add .claude/worktrees/<branch-name> -b <branch-name>
cd .claude/worktrees/<branch-name>

# Build the extension in the worktree's own venv
uv venv && uv pip install -e ".[dev]"
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

Attach the PR URL to the Linear issue with `mcp__linear-server__save_issue` (`links` field) and move it to In Review.

### 5. Babysit CI with gh-axi

```bash
# Watch the latest run on your branch
gh-axi run list --branch <branch-name>

# See what failed
gh-axi run view <run-id> --log-failed

# After fixing, rerun only failed jobs
gh-axi run rerun <run-id> --failed
```

When CI is green, mark the PR ready and merge:
```bash
gh-axi pr ready <number>
gh-axi pr merge <number> --squash --delete-branch
```

Then update the Linear issue to Done.

---

## Build and development commands

All commands assume the virtualenv is active (`source .venv/bin/activate`). Run `uv venv && uv pip install -e ".[dev]"` to create one if it doesn't exist.

```bash
# Build Rust extension and install (required after any Rust change)
uv pip install -e ".[dev]"

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

# Validate generated regions are up to date
python scripts/gen_ids.py --check   # fails if stale
python scripts/gen_ids.py           # regenerate in place

# Cargo check (Rust only, no Python build)
cargo check
```

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
| `ids.rs` | `CardId` and `MonsterId` enums — all string/STS2-ID conversions. **Generated** from `data/cards.toml` + `data/monsters.toml` via `scripts/gen_ids.py`. Do not hand-edit the marked regions. |
| `state.rs` | `CombatState`, `Monster`, `Fighter`, `CardInstance` — all game state. Cloned on every action (persistent immutable-ish tree). |
| `engine.rs` | `Status` enum (the canonical source of truth for in-game statuses), `EffectOp` pipeline, `GameEvent` reactions, and `fire_event`. |
| `cards.rs` | Card data table (`card_data(id, upgrade_level) → CardData`). Has a `BEGIN GENERATED` block for `ALL_CARD_NAMES`. |
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
| `names.py` | `CardName` and `MonsterName` str-enums + `_MONSTER_MAP` dict for normalising bridge class names. **Three regions are generated** by `gen_ids.py`; marked with `# BEGIN/END GENERATED`. |
| `scenarios.py` | Per-monster factory functions (`ironclad_starter_deck_vs_*(seed, deck=None) → CombatState`) and `MONSTER_STARTING_HP` dict. Factory functions are hand-written. |
| `bench/__init__.py` | `Encounter` str-enum, `_SCENARIOS` dict, `run_deck`, `compare`, and run-level benchmarks. |
| `mcts.py` | Pure-Python MCTS tree (UCB1 select/expand/backprop). |
| `policies.py` | `simulate_run_with_reward_lookahead`. |
| `server.py` | FastAPI analysis server (receives live bridge state, returns MCTS action values). |
| `bridge.py` / `bridge_client.py` | HTTP client for the bridge server. |
| `bridge_cli.py` | `sts2` CLI — AXI-compliant wrapper around the bridge MCPTest protocol. |
| `cli.py` | `sts-sim` CLI — interactive and step-mode play. |

### Code generation

`data/cards.toml` and `data/monsters.toml` drive three generated targets:

- `src/ids.rs` — `CardId` and `MonsterId` enum bodies
- `python/sts_sim/names.py` — `CardName` enum, `MonsterName` enum, `_MONSTER_MAP` dict (tagged regions)
- `src/cards.rs` — `ALL_CARD_NAMES` array

Run `python scripts/gen_ids.py` after editing either TOML. CI runs `--check` to catch stale regions.

### Monster registration: the five catalogues

`etc/monsters.toml` is the *management* source of truth checked by `tests/test_monster_registration.py`, which guards all five catalogues:

1. `ids.rs` (`MonsterId` enum) — generated from `data/monsters.toml`
2. `names.py` `MonsterName` enum — generated
3. `names.py` `_MONSTER_MAP` — generated
4. `scenarios.py` `MONSTER_STARTING_HP` dict — hand-maintained; uses `hp` field in `etc/monsters.toml`
5. `bench/__init__.py` `Encounter` — hand-maintained; uses `encounter` field in `etc/monsters.toml`

When adding a new monster: add to `etc/monsters.toml` (with `hp` and `encounter` if appropriate), then add to `data/monsters.toml`, run `scripts/gen_ids.py`, implement AI in `monsters.rs`, add HP constant + factory in `scenarios.py`, and add `Encounter` entry in `bench/__init__.py`. The sync test tells you exactly what's missing.

Monsters tagged with no `encounter` field in `etc/monsters.toml` (e.g. KinPriest, KinFollower, EyeWithTeeth) are covered via composite scenarios or are spawned mid-combat. Multi-monster scenarios are listed in `_MULTI_MONSTER_ENCOUNTERS` in `test_monster_registration.py`.

### Action model

Actions are strings at the Python boundary (`"EndTurn"`, `"PlayCard:Strike"`, `"SelectTarget:Monster:0"`) but `ActionKind` enum values inside Rust hot paths. `legal_actions()` returns typed PyO3 action objects; `apply()` accepts them. The MCTS and rollout paths stay in Rust, avoiding FFI per step.

### State is cloned on every action

`CombatState` is a value type cloned on each `apply()`. This enables tree search without undo. RNG state (`Pcg32`) is embedded in `CombatState` and drives shuffle/monster draws deterministically from the initial seed.

### Worktrees and the extension

Each git worktree needs its own `uv venv` + `uv pip install -e ".[dev]"` because the compiled `.so` is placed inside the venv that was active at build time. `conftest.py` at the root inserts the worktree's own `python/` directory first on `sys.path` so local edits are visible without reinstalling.
