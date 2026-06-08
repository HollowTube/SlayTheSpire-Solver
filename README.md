# SlayTheSpire-Solver

A Slay the Spire combat simulator and solver — a Rust engine exposed as a Python library via [PyO3](https://pyo3.rs), with an MCTS solver and a playable terminal CLI.

## Requirements

- Rust toolchain (`rustup` — any recent stable)
- Python 3.10+
- [uv](https://github.com/astral-sh/uv) (used for the virtual environment and package management)
- [maturin](https://maturin.rs) (installed automatically via the venv)

## Build & install

```bash
# Create and activate the virtual environment
uv venv
source .venv/bin/activate          # Windows: .venv\Scripts\activate

# Install Python dependencies (click, maturin, pytest, ...)
uv pip install -e ".[dev]"         # or: uv pip install click maturin pytest

# Compile the Rust extension and install it into the venv
maturin develop
```

After `maturin develop` the `sts_sim` package is importable from the active venv.

## Run the tests

```bash
source .venv/bin/activate
pytest tests/
```

## Play a fight — interactive mode

Launch a human-playable REPL against the canonical scenario (Ironclad starter deck vs. Jaw Worm):

```bash
python -m sts_sim.cli               # default seed 42
python -m sts_sim.cli --seed 7      # reproducible fight with a different seed
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
# First move: End Turn
python -m sts_sim.cli step --seed 42 --history "" --action "EndTurn"

# Chain: pass updated_history straight back as the next --history
python -m sts_sim.cli step --seed 42 --history "EndTurn" --action "PlayCard:Strike"
python -m sts_sim.cli step --seed 42 --history "EndTurn,PlayCard:Strike" --action "SelectTarget:Monster"
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
