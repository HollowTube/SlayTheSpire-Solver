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
`sts_sim` analysis server and shows MCTS-suggested move values — and an estimated HP loss
for each — in an in-game overlay.

All the moving parts (build, install, game launch, server) are managed by `scripts/overlay.sh`.

### overlay.sh — quick reference

| Command | What it does |
|---|---|
| `./scripts/overlay.sh fresh` | **Full reset**: stop game → build mod → launch game → start server |
| `./scripts/overlay.sh build` | Build and install the mod DLL (game must be off) |
| `./scripts/overlay.sh server` | Refresh WSL IP and start the analysis server |
| `./scripts/overlay.sh launch` | Launch STS2 via Steam |
| `./scripts/overlay.sh stop` | Close STS2 gracefully |

### First-time setup

The script calls Windows executables (`taskkill.exe`, `steam.exe`) from WSL. In non-interactive
shells these require the WSL binfmt_misc interop handler to be registered, which the script does
automatically — but it needs `sudo` the first time each WSL session:

```
==> Registering WSL Windows interop handler...
[sudo] password for tritin:
```

This only prompts once per WSL boot (the registration persists until WSL is shut down). To skip
the prompt on subsequent runs, add the following to `/etc/sudoers` (via `sudo visudo`):

```
tritin ALL=(root) NOPASSWD: /usr/bin/tee /proc/sys/fs/binfmt_misc/register
```

### Typical workflows

**Starting a fresh session after modifying bridge mod code:**

```bash
# Game must be closed before running this so the DLL isn't locked
./scripts/overlay.sh fresh
```

This stops the game (if running), builds and installs the mod, launches STS2 via Steam,
waits for it to come up, then starts the analysis server in the foreground. Enter combat
and the overlay will appear.

**Game is already running, just need the server:**

```bash
./scripts/overlay.sh server
```

**Updating the mod without a full restart (hot reload):**

```bash
./scripts/overlay.sh build   # game must be off, or use hot reload below
```

Or, if the game is running and you want to avoid a full restart:

```bash
./scripts/overlay.sh build   # close game first
# then hot-reload the installed DLL without restarting:
sts2 console "bridge_hot_reload D:\\SteamLibrary\\steamapps\\common\\Slay the Spire 2\\mods\\stssimbridgemod\\stssimbridgemod.dll"
```

### How WSL networking works

The analysis server runs in WSL; the game runs on Windows. They can't reach each other via
`127.0.0.1` — each side's loopback is private to it.

The script handles this automatically:

1. **Server** is started with `--host 0.0.0.0` so it listens on all interfaces including the WSL virtual adapter.
2. **Mod** reads its server address from `<mod_dir>/sts_sim_host.txt` at startup. The script writes the current WSL IP (e.g. `172.26.188.154`) to this file every time it runs. Since WSL2 assigns a new IP on every reboot, this file is always refreshed before the game starts.

### `sts2` — bridge CLI

The `sts2` CLI wraps the MCPTest bridge (port 21337) for inspecting and controlling a live game from the terminal.

```bash
# Show all commands
sts2 --help

# Status
sts2 ping
sts2 state          # current screen + context

# Combat
sts2 combat         # full combat state (HP, hand, enemies, intents)
sts2 piles          # draw / hand / discard / exhaust piles
sts2 actions        # numbered list of legal actions
sts2 act <n>        # take action n, wait for next stable screen

# Navigation
sts2 map            # current map state
sts2 act travel     # travel the highlighted map node

# Player
sts2 player         # player stats, relics, potions
sts2 log            # game log

# Dev console shortcuts — ergonomic wrappers with context
```bash
sts2 dev                        # dashboard: current state + command catalogue
sts2 dev fight JAW_WORM         # jump to a fight
sts2 dev win                    # instantly win the current combat
sts2 dev kill all               # kill all enemies
sts2 dev godmode                # toggle invincibility
sts2 dev gold 999               # add gold
sts2 dev heal                   # full heal (default 999)
sts2 dev card BASH              # spawn a card into hand
sts2 dev relic BURNING_BLOOD    # add a relic
sts2 dev power VULNERABLE 3 1   # apply a power (0=player, 1=first enemy)
```

# Raw console passthrough — any in-game dev console command
sts2 console "gold 999"
sts2 console "fight JAW_WORM"
sts2 console "bridge_hot_reload <path/to/stssimbridgemod.dll>"
```

By default `sts2` connects to `127.0.0.1:21337`. When running from WSL, set the Windows host IP:

```bash
export STS2_BRIDGE_HOST=172.26.176.1   # Windows IP as seen from WSL — check /etc/resolv.conf
sts2 state
```

### Dev console commands

All commands are passed through `sts2 console "<command>"`. IDs use SCREAMING_SNAKE_CASE (e.g. `BODY_SLAM`, `ENTROPIC_BREW`).

**State manipulation (work in combat)**

| Command | Args | Description |
|---|---|---|
| `gold <n>` | amount | Add gold |
| `heal <n> [index]` | amount, target (0=player) | Heal HP |
| `energy <n>` | amount | Add energy |
| `block <n> [index]` | amount, target (0=player) | Add block |
| `draw <n>` | count | Draw cards |
| `damage <n> [index]` | amount, target (0=player) | Deal damage |
| `kill [index\|all]` | target or "all" | Kill enemy/enemies |
| `win` | — | Instantly win the combat |
| `die` | — | Instantly die |
| `godmode` | — | Toggle invincibility |
| `stars <n>` | amount | Add stars |

**Card / deck manipulation**

| Command | Args | Description |
|---|---|---|
| `card <ID> [pile]` | card ID, pile name (default: hand) | Spawn a card into a pile |
| `remove_card <ID> [pile]` | card ID, pile (default: hand) | Remove a card from a pile |
| `upgrade <index>` | hand position (0 = leftmost) | Upgrade a card in hand |
| `enchant <ID> [amount] [index]` | enchantment ID, amount, hand position | Apply enchantment to a card |
| `afflict <ID> [amount] [index]` | affliction ID, amount, hand position | Apply affliction to a card |

**Relics / potions / powers**

| Command | Args | Description |
|---|---|---|
| `relic [add\|remove] <ID>` | add or remove (default: add), relic ID | Add/remove a relic |
| `potion <ID>` | potion ID | Add a potion to your belt |
| `power <ID> <amount> <index>` | power ID, stacks, target (0=player, 1=first enemy) | Apply a power/status |

**Navigation**

| Command | Args | Description |
|---|---|---|
| `fight <ID>` | encounter ID | Jump to a specific fight |
| `event <ID>` | event ID | Jump to a specific event |
| `room <ID>` | room ID | Jump to a specific room type |
| `act <n\|ID>` | act number or ID | Jump to an act |
| `travel` | — | Enable free map travel |

**Misc**

| Command | Args | Description |
|---|---|---|
| `unlock <type\|all>` | cards / potions / relics / monsters / events / all | Unlock content |
| `achievement <unlock\|revoke> [ID]` | operation, achievement ID | Unlock/revoke achievements |
| `instant` | — | Toggle instant (no animation) mode |
| `dump` | — | Dump all model IDs to log |
| `getlogs [name]` | filename | Zip and open game logs |

> **Game log**: `sts2 log` streams the live game log — useful for seeing console command results, mod warnings, and the raw analyze responses the bridge mod receives from the server.

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
