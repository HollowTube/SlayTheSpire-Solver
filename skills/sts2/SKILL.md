---
name: sts2
description: >
  Control and inspect a live Slay the Spire 2 session via the sts2 bridge CLI.
  Use when navigating the game, inspecting combat state, taking actions, running
  dev cheats, or automating a run from the terminal.
---

# sts2 — bridge CLI

`sts2` wraps the MCPTest bridge (port 21337) to inspect and control a live STS2 session from the terminal. Output is TOON-formatted for agent consumption.

## Setup

```bash
# WSL: set the Windows host IP so sts2 can reach the bridge
export STS2_BRIDGE_HOST=172.26.176.1   # check /etc/resolv.conf if different

# Verify connection
sts2 ping
```

## Home view

Run `sts2` with no arguments to see the current screen, HP, hand, enemies, and contextual next-step hints:

```
game:
  screen: COMBAT_PLAYER_TURN
  floor: 3 / act: 1 / hp: 78/80 / gold: 99
hand[4]{name,cost,upgraded}: ...
enemies[1]{name,hp,intent}: ...
help[3]:
  Run `sts2 actions` to see legal moves
  Run `sts2 act <n>` to execute a move
  ...
```

The hints change based on screen: Neow, MAP, REWARD, COMBAT, and MAIN_MENU each get specific guidance.

## Core commands

```bash
sts2                    # current screen state + contextual hints
sts2 ping               # check bridge connectivity
sts2 state              # run state: floor, act, HP, gold, seed
sts2 combat             # full combat state: hand, enemies, energy, turn
sts2 piles              # draw / hand / discard / exhaust piles
sts2 player             # player stats, deck, relics
sts2 map                # map state and available paths
sts2 log                # recent game log (bridge responses, mod output)
sts2 actions            # numbered list of legal actions for current screen
sts2 act <n>            # execute action by index (from `sts2 actions`)
sts2 start              # start a new run (--char, --seed, --asc, --fight, --godmode)
sts2 console "<cmd>"    # raw game console passthrough
```

## Dev shortcuts — `sts2 dev`

`sts2 dev` with no arguments shows a dashboard of all available cheat commands grouped by category. Each subcommand validates args, runs the console command, and returns updated combat context inline.

```bash
sts2 dev                          # dashboard + current screen/HP
```

### Navigate
```bash
sts2 dev fight <ID>               # jump to a specific fight (e.g. JAW_WORM, MAWLER)
sts2 dev event <ID>               # jump to a specific event
```

### Combat cheats
```bash
sts2 dev win                      # instantly win the current fight
sts2 dev kill [all|<n>]           # kill all enemies or one by index
sts2 dev godmode                  # toggle invincibility
sts2 dev energy <n>               # add energy
sts2 dev heal [<n>]               # heal HP (default 999 = full heal)
sts2 dev block <n>                # add block (0=player, 1+=enemy)
sts2 dev power <ID> <n> <target>  # apply a power (0=player, 1=first enemy)
```

### Cards
```bash
sts2 dev card <ID> [pile]         # spawn card into hand/draw/discard/exhaust
sts2 dev draw <n>                 # draw cards
sts2 dev upgrade <index>          # upgrade card at hand index (0=leftmost)
sts2 dev remove <ID>              # remove card from hand
```

### Loot
```bash
sts2 dev gold <n>                 # add gold
sts2 dev relic <ID>               # add a relic
sts2 dev potion <ID>              # add a potion
```

All IDs use SCREAMING_SNAKE_CASE (e.g. `JAW_WORM`, `BASH`, `BURNING_BLOOD`). Run `sts2 dev <cmd> --help` for per-subcommand usage.

## Common workflows

**Get out of Neow fast:**
```bash
sts2 actions            # see blessing options by index
sts2 act <n>            # pick one
# or skip straight to a fight:
sts2 dev fight JAW_WORM
```

**Inspect and play a combat turn:**
```bash
sts2 combat             # see hand, enemies, energy
sts2 actions            # see legal moves with indices
sts2 act 0              # play first action
```

**Jump to a specific fight and cheat through it:**
```bash
sts2 dev fight MAWLER
sts2 dev godmode
sts2 dev win
```

**Check the overlay analysis:**
```bash
sts2 log                # shows raw analyze responses with action values + expected HP lost
```

## Output format

All output is [TOON](https://toonformat.dev/) — token-efficient, agent-readable. Pass `--json` to any command for raw JSON instead.

## Notes

- Bridge connects via MCPTest on port 21337 (Windows host from WSL)
- Analysis server runs in WSL on port 8765; the bridge mod connects to it automatically
- `sts2 log` shows `[sts_sim_bridge_mod] analyze response: {...}` with MCTS action values and expected HP lost per action
- ShrinkPower and other STS2-specific debuffs are not modelled by the Rust sim — the overlay's expected HP values may be optimistic when debuffed

## Session hook — ambient game state on every session start

Install a `SessionStart` hook so Claude Code automatically shows live game state at the top of every session — screen, HP, hand, enemies, and contextual hints — before you type anything:

```bash
sts2 setup-hook              # install into ./.claude/settings.json
sts2 setup-hook --remove     # uninstall
```

The hook auto-detects the WSL bridge host from `/etc/resolv.conf` — no `STS2_BRIDGE_HOST` export needed. It fails silently when the bridge is unreachable so sessions open normally even if the game isn't running.

After installing, restart Claude Code to activate.
