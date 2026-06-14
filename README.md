# SlayTheSpire-Solver

A Slay the Spire combat simulator and solver — a Rust engine exposed as a Python library via [PyO3](https://pyo3.rs), with an MCTS solver and a playable terminal CLI.

## Requirements

- Rust toolchain (`rustup` — any recent stable)
- Python 3.10+
- [uv](https://github.com/astral-sh/uv) (for the virtual environment and package management)

## Build & install

```bash
# 1. Create and activate the virtual environment
uv venv
source .venv/bin/activate          # Windows: .venv\Scripts\activate

# 2. Build the Rust extension and install everything (including the sts-sim CLI entry point)
uv pip install -e .
```

> **The venv must be active for every command below.** If you open a new terminal, run `source .venv/bin/activate` again before using `sts-sim`, `python -m sts_sim.cli`, or `pytest`.

To rebuild after changing Rust code:

```bash
maturin develop      # faster than a full re-install
```

## Run the tests

```bash
pytest tests/
```

## Run the CI checks locally

CI (`.github/workflows/ci.yml`) runs the following checks against every push and
pull request. Run them yourself before pushing to catch issues early:

```bash
# Rust: must build with no warnings
cargo check

# Python lint and formatting
ruff check python/ tests/
ruff format --check python/ tests/    # use `ruff format` (no --check) to auto-fix

# Python type-checking
mypy python/ tests/ --ignore-missing-imports

# Test suite
pytest tests/ -q
```

`ruff` and `mypy` are part of the `dev` extras — install them with:

```bash
uv pip install -e ".[dev]"
```

## Play a fight — interactive mode

Launch a human-playable REPL against the canonical scenario (Ironclad starter deck vs. Jaw Worm):

```bash
sts-sim                 # default seed 42
sts-sim --seed 7        # reproducible fight with a different seed

# or equivalently:
python -m sts_sim.cli --seed 7
```

Each turn the CLI renders the full game state and presents a numbered menu of legal actions. Type the number and press Enter:

```
Turn 0
You: 80 HP | Block: 0 | Energy: 3
  Statuses: none
  Hand: Strike, Bash, Strike, Strike, Defend
Jaw Worm: 44 HP | Block: 0 | Intent: Chomp
  Statuses: none
Choose an action:
  1. Play Strike
  2. Play Bash
  3. Play Strike
  4. Play Strike
  5. Play Defend
  6. End Turn
>
```

Invalid input (non-numeric, out-of-range) is rejected with a message and re-prompted — a typo won't crash or skip an action. When the fight ends the CLI reports the outcome, your final HP, and the shaped reward:

```
You won! Final HP: 65
Reward: 0.81 (evaluate: 0.81)
```

## Agent / scripted mode — `step`

A non-blocking, single-shot subcommand for automated play. Each invocation replays a given action history from a seed, applies one new action, and prints the resulting state, the new legal-actions menu, and an `updated_history` — then exits immediately.

```bash
# First move
sts-sim step --seed 42 --history "" --action "EndTurn"

# Chain: pass updated_history straight back as the next --history
sts-sim step --seed 42 --history "EndTurn" --action "PlayCard:Strike"
sts-sim step --seed 42 --history "EndTurn,PlayCard:Strike" --action "SelectTarget:Monster"
```

Example output:

```
Turn 1
You: 69 HP | Block: 0 | Energy: 2
  Statuses: none
  Hand: Defend, Defend, Defend, Defend
Jaw Worm: 44 HP | Block: 0 | Intent: Thrash
  Statuses: none

  1. Target Monster

updated_history: EndTurn,PlayCard:Strike
```

The `updated_history` line is the complete action sequence to replay next time — the CLI is the single source of truth; you never need to track it yourself.

Action strings come directly from `legal_actions` (e.g. `PlayCard:Strike`, `SelectTarget:Monster`, `EndTurn`). Use the numbered menu to find valid choices, then pass the corresponding string to `--action`.

### Fight mechanics (verified against seed 42)

| Action | Effect |
|---|---|
| Strike | 6 damage to target |
| Defend | 5 block |
| Bash | 8 damage + 2× Vulnerable to target (costs 2 energy) |
| Vulnerable | Target takes 50% extra damage from attacks |
| Jaw Worm — Chomp | 11 damage (opening move) |
| Jaw Worm — Thrash | 7 damage + gains 5 block |
| Jaw Worm — Bellow | Gains 3 Strength + 6 block |

Block absorbs damage before HP and resets at the start of each combatant's own turn. Energy refreshes to 3 at the start of each player turn. The player draws 5 cards per turn; played cards go to the discard pile and reshuffle when the draw pile runs dry.

## Bridge mod — live overlay in Slay the Spire 2

`bridge_mod/` is a C# mod for Slay the Spire 2 that pushes the live combat state to the
`sts_sim` analysis server (see `python/sts_sim/server.py`) and shows the MCTS-suggested
move values — and an estimated HP loss for each — in an in-game overlay.

To build and install it (or pick up changes after editing `bridge_mod/Code/*.cs`):

```bash
cd bridge_mod
dotnet build sts_sim_bridge_mod.csproj
```

Then copy the build output into the game's `mods/stssimbridgemod/` directory, e.g.:

```bash
cp bin/Debug/net9.0/stssimbridgemod.dll bin/Debug/net9.0/stssimbridgemod.pdb \
  "<Slay the Spire 2 install dir>/mods/stssimbridgemod/"
```

**The game must be closed while copying** — `stssimbridgemod.dll` is locked while the
mod is loaded, so overwriting it while the game is running fails with a permissions
error. Close the game, copy the new build, then relaunch.

## Python API

The engine is also usable directly from Python:

```python
from sts_sim import apply, is_terminal, legal_actions, reward
from sts_sim.scenarios import ironclad_starter_deck_vs_jaw_worm

state = ironclad_starter_deck_vs_jaw_worm(seed=42)

while not is_terminal(state):
    action = legal_actions(state)[0]   # or your own policy
    state = apply(state, action)

print(f"reward: {reward(state):.2f}")
```

See `python/sts_sim/mcts.py` for the MCTS solver built on the same interface.
