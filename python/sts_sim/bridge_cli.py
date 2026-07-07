"""sts2 — bridge CLI for inspecting and controlling STS2 via MCPTest (port 21337).

Usage:
    sts2 [--host HOST] [--json] <command>

Set STS2_BRIDGE_HOST env var for WSL (e.g. export STS2_BRIDGE_HOST=172.x.x.1).
"""

from __future__ import annotations

import json
import sys
from typing import Any

import click

from . import bridge_client as bc


# ── rendering ────────────────────────────────────────────────────────────────


def _block(name: str, data: dict[str, Any]) -> str:
    lines = [f"{name}:"]
    for k, v in data.items():
        lines.append(f"  {k}: {v}")
    return "\n".join(lines)


def _table(name: str, rows: list[dict], fields: list[str]) -> str:
    header = f"{name}[{len(rows)}]{{{','.join(fields)}}}:"
    lines = [header]
    for row in rows:
        lines.append("  " + ",".join(str(row.get(f, "")) for f in fields))
    return "\n".join(lines)


def _hint(hints: list[str]) -> str:
    return "\n".join([f"help[{len(hints)}]:"] + [f"  {h}" for h in hints])


def _error(msg: str, help_text: str | None = None) -> str:
    lines = [f"error: {msg}"]
    if help_text:
        lines.append(f"help: {help_text}")
    return "\n".join(lines)


def _fmt_intent(intent: dict | str | None) -> str:
    if not intent or not isinstance(intent, dict):
        return str(intent or "?")
    intents = intent.get("intents", [])
    if not intents:
        return intent.get("move_id", "?")
    parts = []
    for i in intents:
        t = i.get("type", "?")
        if t == "Attack":
            dmg, hits = i.get("damage", "?"), i.get("hits", 1)
            parts.append(f"Attack({dmg}x{hits})" if hits > 1 else f"Attack({dmg})")
        else:
            parts.append(t)
    return " + ".join(parts)


def _card_name(c: Any) -> str:
    """Extract display name from a card — handles both dict and raw STS2 ID string."""
    if isinstance(c, dict):
        return c.get("name") or c.get("card_name") or "?"
    return str(c)  # raw STS2 ID e.g. "STRIKE_IRONCLAD"


def _card_upgraded(c: Any) -> bool:
    if isinstance(c, dict):
        return c.get("upgraded", False)
    return "+" in str(c)


# ── bridge call wrapper ───────────────────────────────────────────────────────


_CONNECTION_ERRORS = (
    "Bridge not running",
    "Bridge timed out",
    "Bridge unreachable",
    "Bridge communication failed",
)


def _call(fn, *args, **kwargs) -> dict:
    result = fn(*args, **kwargs)
    # send_request returns raw {"result": {...}} — unwrap it
    if isinstance(result, dict):
        result = result.get("result", result)
    if isinstance(result, dict) and result.get("error"):
        err = result["error"]
        hint = (
            "Set STS2_BRIDGE_HOST if running from WSL"
            if any(e in err for e in _CONNECTION_ERRORS)
            else None
        )
        click.echo(_error(err, hint))
        sys.exit(1)
    return result


# ── main group ───────────────────────────────────────────────────────────────


def _home_hints(screen: str, context_type: str = "") -> list[str]:
    """Context-aware hints for the no-args home view."""
    s = (screen or "").upper()
    c = (context_type or "").upper()
    if "COMBAT" in s or s == "COMBAT_PLAYER_TURN":
        return [
            "Run `sts2 actions` to see legal moves",
            "Run `sts2 act <n>` to execute a move",
            "Run `sts2 dev win` to instantly win, or `sts2 dev kill all` to kill enemies",
        ]
    if s in ("MAP", "MAP_SCREEN"):
        return [
            "Run `sts2 actions` to see available map paths",
            "Run `sts2 act <n>` to travel to a node",
            "Run `sts2 dev fight <ID>` to jump straight to a specific fight",
        ]
    if s == "REWARD":
        return [
            "Run `sts2 actions` to see reward choices",
            "Run `sts2 act <n>` to pick a reward or proceed",
            "Run `sts2 dev fight <ID>` to jump to the next fight directly",
        ]
    if s in ("MAIN_MENU", "TITLE", "", "?"):
        return [
            "Run `sts2 start` to begin a new run",
            "Run `sts2 dev fight <ID>` to jump straight into a specific fight (e.g. JAW_WORM)",
            "Run `sts2 --help` for all commands",
        ]
    if s == "EVENT" and c == "NEVENTROOM":
        return [
            "Run `sts2 actions` to see Neow's blessing options",
            "Run `sts2 act <n>` to pick a blessing by index",
            "Run `sts2 dev fight <ID>` to skip ahead to a specific fight (e.g. `sts2 dev fight JAW_WORM`)",
        ]
    # Default: no active combat — show how to get into one
    return [
        "Run `sts2 actions` to see what you can do on this screen",
        "Run `sts2 dev fight <ID>` to jump to a fight (e.g. `sts2 dev fight JAW_WORM`)",
        "Run `sts2 start` to begin a new run",
    ]


@click.group(invoke_without_command=True)
@click.option(
    "--host",
    envvar="STS2_BRIDGE_HOST",
    default="127.0.0.1",
    show_default=True,
    help="Bridge host (set STS2_BRIDGE_HOST for WSL).",
)
@click.option("--json", "as_json", is_flag=True, help="Output raw JSON.")
@click.option("--timeout", default=12.0, show_default=True, type=float)
@click.pass_context
def main(ctx: click.Context, host: str, as_json: bool, timeout: float) -> None:
    """Inspect and control STS2 via the MCPTest bridge (port 21337)."""
    ctx.ensure_object(dict)
    ctx.obj["as_json"] = as_json
    bc.BRIDGE_HOST = host
    bc.TIMEOUT = timeout

    if ctx.invoked_subcommand is not None:
        return

    data = _call(bc.get_full_state)
    if as_json:
        click.echo(json.dumps(data, indent=2))
        return

    parts: list[str] = []
    screen = data.get("screen", "?")
    run = data.get("run", {})
    parts.append(
        _block(
            "game",
            {
                "screen": screen,
                "floor": run.get("floor", "?"),
                "act": run.get("act", "?"),
                "hp": f"{run.get('current_hp', '?')}/{run.get('max_hp', '?')}",
                "gold": run.get("gold", "?"),
                "energy": f"{data.get('energy', '?')}/{data.get('max_energy', '?')}",
            },
        )
    )
    hand = data.get("hand", [])
    if hand:
        parts.append(
            _table(
                "hand",
                [
                    {
                        "name": _card_name(c),
                        "cost": c.get("energy_cost", "?")
                        if isinstance(c, dict)
                        else "?",
                    }
                    for c in hand
                ],
                ["name", "cost"],
            )
        )
    enemies = data.get("enemies", [])
    if enemies:
        parts.append(
            _table(
                "enemies",
                [
                    {
                        "name": e.get("name", "?"),
                        "hp": f"{e.get('hp', '?')}/{e.get('max_hp', '?')}",
                        "intent": _fmt_intent(e.get("intent")),
                    }
                    for e in enemies
                ],
                ["name", "hp", "intent"],
            )
        )
    context_type = (data.get("available_actions") or {}).get("screen_context_type", "")
    parts.append(_hint(_home_hints(screen, context_type)))
    click.echo("\n".join(parts))


# ── commands ─────────────────────────────────────────────────────────────────


@main.command()
@click.pass_context
def ping(ctx: click.Context) -> None:
    """Check bridge connectivity."""
    data = _call(bc.ping)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    click.echo(
        _block(
            "bridge",
            {
                "status": data.get("status", "ok"),
                "version": data.get("version", "?"),
                "screen": data.get("screen", "?"),
            },
        )
    )


@main.command()
@click.pass_context
def state(ctx: click.Context) -> None:
    """Run state: floor, act, HP, gold, seed."""
    data = _call(bc.get_run_state)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    click.echo(
        _block(
            "run",
            {
                "floor": data.get("floor", "?"),
                "act": data.get("act", "?"),
                "hp": f"{data.get('current_hp', '?')}/{data.get('max_hp', '?')}",
                "gold": data.get("gold", "?"),
                "seed": data.get("seed", "?"),
            },
        )
    )


@main.command()
@click.pass_context
def combat(ctx: click.Context) -> None:
    """Combat state: hand, enemies, energy, turn."""
    data = _call(bc.get_combat_state)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    parts: list[str] = []
    player = (data.get("players") or [{}])[0]
    parts.append(
        _block(
            "combat",
            {
                "screen": data.get("screen", "?"),
                "energy": f"{player.get('energy', '?')}/{player.get('max_energy', '?')}",
                "turn": data.get("round", "?"),
                "hp": f"{player.get('hp', '?')}/{player.get('max_hp', '?')}",
                "block": player.get("block", 0),
            },
        )
    )
    hand = player.get("hand", data.get("hand", []))
    if hand:
        parts.append(
            _table(
                "hand",
                [
                    {
                        "name": _card_name(c),
                        "cost": c.get("energy_cost", "?")
                        if isinstance(c, dict)
                        else "?",
                        "upgraded": _card_upgraded(c),
                    }
                    for c in hand
                ],
                ["name", "cost", "upgraded"],
            )
        )
    enemies = data.get("enemies", [])
    if enemies:
        parts.append(
            _table(
                "enemies",
                [
                    {
                        "name": e.get("name", "?"),
                        "hp": f"{e.get('hp', '?')}/{e.get('max_hp', '?')}",
                        "block": e.get("block", 0),
                        "intent": _fmt_intent(e.get("intent")),
                    }
                    for e in enemies
                ],
                ["name", "hp", "block", "intent"],
            )
        )
    parts.append(
        _hint(
            [
                "Run `sts2 actions` to see legal moves",
                "Run `sts2 act <n>` to execute a move",
            ]
        )
    )
    click.echo("\n".join(parts))


@main.command()
@click.pass_context
def piles(ctx: click.Context) -> None:
    """Card piles: draw, hand, discard, exhaust (handles STS2 ID strings)."""
    raw = bc.get_card_piles()
    if isinstance(raw, dict):
        raw = raw.get("result", raw)
    if isinstance(raw, dict) and raw.get("error"):
        click.echo(f"piles: {raw['error']}")
        return
    data = raw
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    parts: list[str] = []
    pile_keys = {
        "draw": "draw_pile",
        "hand": "hand",
        "discard": "discard_pile",
        "exhaust": "exhaust_pile",
    }
    for label, key in pile_keys.items():
        raw = data.get(key, data.get(label, []))
        # MCPTest returns {count, cards:[...]}; flat list also accepted
        cards = raw.get("cards", raw) if isinstance(raw, dict) else raw
        if cards:
            parts.append(
                _table(
                    label,
                    [
                        {
                            "name": _card_name(c),
                            "upgraded": _card_upgraded(c),
                            "type": c.get("type", "?") if isinstance(c, dict) else "?",
                            "cost": c.get("energy_cost", "?")
                            if isinstance(c, dict)
                            else "?",
                        }
                        for c in cards
                    ],
                    ["name", "upgraded", "type", "cost"],
                )
            )
        else:
            parts.append(f"{label}: (empty)")
    click.echo("\n".join(parts))


@main.command()
@click.pass_context
def player(ctx: click.Context) -> None:
    """Player state: deck, relics, HP."""
    data = _call(bc.get_player_state)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    parts: list[str] = []
    parts.append(
        _block(
            "player",
            {
                "character": data.get("character", "?"),
                "hp": f"{data.get('current_hp', '?')}/{data.get('max_hp', '?')}",
                "gold": data.get("gold", "?"),
            },
        )
    )
    deck = data.get("deck", [])
    if deck:
        parts.append(
            _table(
                "deck",
                [
                    {
                        "name": _card_name(c),
                        "upgraded": _card_upgraded(c),
                        "type": c.get("type", "?") if isinstance(c, dict) else "?",
                    }
                    for c in deck
                ],
                ["name", "upgraded", "type"],
            )
        )
    click.echo("\n".join(parts))


@main.command("map")
@click.pass_context
def map_(ctx: click.Context) -> None:
    """Map state: available paths."""
    data = _call(bc.get_map_state)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    parts: list[str] = []
    parts.append(
        _block("map", {"floor": data.get("floor", "?"), "act": data.get("act", "?")})
    )
    nodes = data.get("available_nodes", data.get("nodes", []))
    if nodes:
        parts.append(
            _table(
                "paths",
                [
                    {
                        "node": n.get("node", n.get("id", "?")),
                        "type": n.get("type", "?"),
                    }
                    for n in nodes
                ],
                ["node", "type"],
            )
        )
    click.echo("\n".join(parts))


@main.command()
@click.option("--lines", default=20, show_default=True)
@click.pass_context
def log(ctx: click.Context, lines: int) -> None:
    """Recent game log entries."""
    data = _call(bc.get_game_log)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    entries = data.get("entries", data.get("log", []))
    for entry in entries[-lines:]:
        if isinstance(entry, dict):
            click.echo(f"  {entry.get('time', '')} {entry.get('message', entry)}")
        else:
            click.echo(f"  {entry}")


def _action_label(a: dict) -> str:
    t = a.get("action", "")
    if t == "play_card":
        card = a.get("card_name", "?")
        target = a.get("target_name")
        return f"{card} → {target}" if target else card
    if t == "travel":
        return f"node {a.get('node', '?')} ({a.get('type', '?')})"
    if t == "end_turn":
        return "End turn"
    return a.get("label", t)


def _execute(action: dict) -> dict:
    if action.get("action") == "travel":
        row, col = action["node"].split(",")
        return _call(bc.navigate_map, int(row), int(col))
    params = {k: v for k, v in action.items() if k != "action" and v is not None}
    return _call(bc.act_and_wait, action["action"], **params)


@main.command()
@click.pass_context
def actions(ctx: click.Context) -> None:
    """List legal actions for the current screen."""
    data = _call(bc.get_available_actions)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    action_list = data.get("actions", [])
    screen = data.get("screen", "?")
    if not action_list:
        click.echo(f"actions: 0 on screen {screen}")
        return
    parts = [
        _block("context", {"screen": screen}),
        _table(
            "actions",
            [
                {"n": i, "action": a.get("action", "?"), "label": _action_label(a)}
                for i, a in enumerate(action_list)
            ],
            ["n", "action", "label"],
        ),
        _hint(["Run `sts2 act <n>` to execute action by index"]),
    ]
    click.echo("\n".join(parts))


@main.command()
@click.argument("n", type=int)
@click.pass_context
def act(ctx: click.Context, n: int) -> None:
    """Execute legal action by index (from `sts2 actions`)."""
    action_list = _call(bc.get_available_actions).get("actions", [])
    if n < 0 or n >= len(action_list):
        click.echo(_error(f"index {n} out of range (0–{len(action_list) - 1})"))
        sys.exit(1)
    chosen = action_list[n]
    result = _execute(chosen)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(result, indent=2))
        return
    parts = [
        _block(
            "acted",
            {"action": chosen.get("action", "?"), "label": _action_label(chosen)},
        )
    ]
    screen = result.get("screen", "")
    combat_data = result.get("combat", {})
    if combat_data:
        p = (combat_data.get("players") or [{}])[0]
        parts.append(
            _block(
                "combat",
                {
                    "screen": screen,
                    "energy": f"{p.get('energy', '?')}/{p.get('max_energy', '?')}",
                    "turn": combat_data.get("round", "?"),
                    "hp": f"{p.get('hp', '?')}/{p.get('max_hp', '?')}",
                },
            )
        )
        hand = p.get("hand", combat_data.get("hand", []))
        if hand:
            parts.append(
                _table(
                    "hand",
                    [
                        {
                            "name": _card_name(c),
                            "cost": c.get("energy_cost", "?")
                            if isinstance(c, dict)
                            else "?",
                        }
                        for c in hand
                    ],
                    ["name", "cost"],
                )
            )
        enemies = combat_data.get("enemies", [])
        if enemies:
            parts.append(
                _table(
                    "enemies",
                    [
                        {
                            "name": e.get("name", "?"),
                            "hp": f"{e.get('hp', '?')}/{e.get('max_hp', '?')}",
                            "intent": _fmt_intent(e.get("intent")),
                        }
                        for e in enemies
                    ],
                    ["name", "hp", "intent"],
                )
            )
    elif screen:
        parts.append(f"screen: {screen}")
    parts.append(_hint(["Run `sts2 actions` to see next legal moves"]))
    click.echo("\n".join(parts))


@main.command()
@click.option("--char", default="Ironclad", show_default=True)
@click.option("--seed", default=None)
@click.option("--fight", default=None, help="Jump to a specific fight (e.g. NIBBIT).")
@click.option("--asc", default=0, show_default=True, type=int)
@click.option("--godmode", is_flag=True)
@click.pass_context
def start(
    ctx: click.Context,
    char: str,
    seed: str | None,
    fight: str | None,
    asc: int,
    godmode: bool,
) -> None:
    """Start a new run."""
    data = _call(
        bc.start_run,
        character=char,
        ascension=asc,
        seed=seed,
        fight=fight,
        godmode=godmode,
    )
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    click.echo(
        _block(
            "started",
            {
                "character": char,
                "ascension": asc,
                "seed": seed or "(random)",
                "fight": fight or "(normal)",
            },
        )
    )
    click.echo(_hint(["Run `sts2` to see current state"]))


@main.command()
@click.argument("cmd", nargs=-1, required=True)
@click.pass_context
def console(ctx: click.Context, cmd: tuple[str, ...]) -> None:
    """Execute a raw game console command."""
    command = " ".join(cmd)
    data = _call(bc.execute_console_command, command)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    click.echo(
        _block(
            "console",
            {
                "command": command,
                "result": data.get("result", data.get("output", "ok")),
            },
        )
    )


# ── dev group ─────────────────────────────────────────────────────────────────

# Console IDs use SCREAMING_SNAKE_CASE throughout.
# Each subcommand: validates args → runs console command → prints result +
# enough context that the agent doesn't need a follow-up call.


def _dev_exec(command: str) -> dict:
    """Run a console command and return the raw result dict."""
    return _call(bc.execute_console_command, command)


def _dev_out(
    command: str, result: dict, context_parts: list[str] | None = None
) -> None:
    """Emit a TOON dev result block followed by optional context lines."""
    click.echo(
        _block("dev", {"command": command, "result": result.get("result", "ok")})
    )
    if context_parts:
        for part in context_parts:
            click.echo(part)


def _combat_context() -> list[str]:
    """Return TOON lines for the current combat state (best-effort, empty on error)."""
    try:
        data = _call(bc.get_combat_state)
        player = (data.get("players") or [{}])[0]
        parts = [
            _block(
                "context",
                {
                    "screen": data.get("screen", "?"),
                    "hp": f"{player.get('hp', '?')}/{player.get('max_hp', '?')}",
                    "energy": f"{player.get('energy', '?')}/{player.get('max_energy', '?')}",
                    "block": player.get("block", 0),
                },
            )
        ]
        enemies = data.get("enemies", [])
        if enemies:
            parts.append(
                _table(
                    "enemies",
                    [
                        {
                            "name": e.get("name", "?"),
                            "hp": f"{e.get('hp', '?')}/{e.get('max_hp', '?')}",
                            "intent": _fmt_intent(e.get("intent")),
                        }
                        for e in enemies
                    ],
                    ["name", "hp", "intent"],
                )
            )
        hand = player.get("hand", data.get("hand", []))
        if hand:
            parts.append(
                _table(
                    "hand",
                    [
                        {
                            "name": _card_name(c),
                            "cost": c.get("energy_cost", "?")
                            if isinstance(c, dict)
                            else "?",
                        }
                        for c in hand
                    ],
                    ["name", "cost"],
                )
            )
        return parts
    except SystemExit:
        return []


def _screen_context() -> list[str]:
    """Return a single TOON context line with the current screen (best-effort)."""
    try:
        data = _call(bc.get_run_state)
        return [_block("context", {"screen": data.get("screen", "?")})]
    except SystemExit:
        return []


@main.group(invoke_without_command=True)
@click.pass_context
def dev(ctx: click.Context) -> None:
    """Dev console shortcuts — cheat commands for testing and exploration."""
    if ctx.invoked_subcommand is not None:
        return

    # No-args: show current context + command catalogue
    screen = "?"
    hp = "?"
    try:
        run = _call(bc.get_run_state)
        screen = run.get("screen", "?")
        hp = f"{run.get('current_hp', '?')}/{run.get('max_hp', '?')}"
    except SystemExit:
        pass

    click.echo(_block("dev", {"screen": screen, "hp": hp}))
    click.echo(
        _table(
            "navigate",
            [
                {
                    "cmd": "fight <ID>",
                    "example": "fight JAW_WORM",
                    "effect": "Jump to a specific fight",
                },
                {
                    "cmd": "event <ID>",
                    "example": "event ANCIENT",
                    "effect": "Jump to a specific event",
                },
            ],
            ["cmd", "example", "effect"],
        )
    )
    click.echo(
        _table(
            "combat",
            [
                {"cmd": "win", "args": "", "effect": "Instantly win the fight"},
                {
                    "cmd": "kill [all|n]",
                    "args": "all",
                    "effect": "Kill all enemies or one by index",
                },
                {"cmd": "godmode", "args": "", "effect": "Toggle invincibility"},
                {"cmd": "energy <n>", "args": "3", "effect": "Add energy"},
                {"cmd": "heal [n]", "args": "999", "effect": "Heal HP (default: full)"},
                {"cmd": "block <n>", "args": "50", "effect": "Add block"},
                {
                    "cmd": "power <ID> <n> <target>",
                    "args": "VULNERABLE 3 1",
                    "effect": "Apply a power (0=player, 1+=enemy)",
                },
            ],
            ["cmd", "args", "effect"],
        )
    )
    click.echo(
        _table(
            "cards",
            [
                {
                    "cmd": "card <ID> [pile]",
                    "args": "BASH hand",
                    "effect": "Spawn a card (default pile: hand)",
                },
                {"cmd": "draw <n>", "args": "3", "effect": "Draw cards"},
                {
                    "cmd": "upgrade <i>",
                    "args": "0",
                    "effect": "Upgrade card at hand index",
                },
                {
                    "cmd": "remove <ID>",
                    "args": "CURSE_REGRET",
                    "effect": "Remove card from hand or deck",
                },
            ],
            ["cmd", "args", "effect"],
        )
    )
    click.echo(
        _table(
            "loot",
            [
                {"cmd": "gold <n>", "args": "999", "effect": "Add gold"},
                {"cmd": "relic <ID>", "args": "BURNING_BLOOD", "effect": "Add a relic"},
                {
                    "cmd": "potion <ID>",
                    "args": "ENTROPIC_BREW",
                    "effect": "Add a potion",
                },
            ],
            ["cmd", "args", "effect"],
        )
    )
    click.echo(
        _hint(
            [
                "Run `sts2 dev <cmd> --help` for full usage",
                "Run `sts2 dev fight <ID>` to jump straight to a fight",
                "Run `sts2 dev win` to end the current combat instantly",
            ]
        )
    )


# ── navigation ────────────────────────────────────────────────────────────────


@dev.command()
@click.argument("encounter_id")
@click.pass_context
def fight(ctx: click.Context, encounter_id: str) -> None:
    """Jump to a specific fight (e.g. JAW_WORM, MAWLER, TWO_LOUSES)."""
    result = _dev_exec(f"fight {encounter_id}")
    _dev_out(f"fight {encounter_id}", result, _screen_context())
    click.echo(_hint(["Run `sts2 actions` to see available actions"]))


@dev.command()
@click.argument("event_id")
@click.pass_context
def event(ctx: click.Context, event_id: str) -> None:
    """Jump to a specific event."""
    result = _dev_exec(f"event {event_id}")
    _dev_out(f"event {event_id}", result, _screen_context())


# ── combat cheats ─────────────────────────────────────────────────────────────


@dev.command()
@click.pass_context
def win(ctx: click.Context) -> None:
    """Instantly win the current combat."""
    result = _dev_exec("win")
    _dev_out("win", result, _screen_context())


@dev.command()
@click.argument("target", default="all")
@click.pass_context
def kill(ctx: click.Context, target: str) -> None:
    """Kill enemies. TARGET is 'all' or an enemy index (default: all)."""
    cmd = f"kill {target}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result, _combat_context())


@dev.command()
@click.pass_context
def godmode(ctx: click.Context) -> None:
    """Toggle invincibility."""
    result = _dev_exec("godmode")
    _dev_out("godmode", result)


@dev.command()
@click.argument("amount", type=int)
@click.pass_context
def energy(ctx: click.Context, amount: int) -> None:
    """Add energy."""
    cmd = f"energy {amount}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result, _combat_context())


@dev.command()
@click.argument("amount", type=int, default=999)
@click.argument("target", type=int, default=0)
@click.pass_context
def heal(ctx: click.Context, amount: int, target: int) -> None:
    """Heal HP. AMOUNT defaults to 999 (effectively full). TARGET is 0=player."""
    cmd = f"heal {amount} {target}" if target else f"heal {amount}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result, _combat_context())


@dev.command()
@click.argument("amount", type=int)
@click.argument("target", type=int, default=0)
@click.pass_context
def block(ctx: click.Context, amount: int, target: int) -> None:
    """Add block. TARGET is 0=player, 1+=enemy index."""
    cmd = f"block {amount} {target}" if target else f"block {amount}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result, _combat_context())


@dev.command()
@click.argument("power_id")
@click.argument("amount", type=int)
@click.argument("target", type=int)
@click.pass_context
def power(ctx: click.Context, power_id: str, amount: int, target: int) -> None:
    """Apply a power. TARGET: 0=player, 1+=enemy index. E.g.: power VULNERABLE 3 1"""
    cmd = f"power {power_id} {amount} {target}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result, _combat_context())


# ── card manipulation ─────────────────────────────────────────────────────────


@dev.command()
@click.argument("card_id")
@click.argument("pile", default="hand")
@click.pass_context
def card(ctx: click.Context, card_id: str, pile: str) -> None:
    """Spawn CARD_ID into PILE (hand, draw, discard, exhaust). Default: hand."""
    cmd = f"card {card_id} {pile}" if pile != "hand" else f"card {card_id}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result, _combat_context())
    click.echo(_hint(["Run `sts2 piles` to see all piles"]))


@dev.command()
@click.argument("amount", type=int)
@click.pass_context
def draw(ctx: click.Context, amount: int) -> None:
    """Draw AMOUNT cards."""
    cmd = f"draw {amount}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result, _combat_context())


@dev.command()
@click.argument("hand_index", type=int)
@click.pass_context
def upgrade(ctx: click.Context, hand_index: int) -> None:
    """Upgrade the card at HAND_INDEX (0 = leftmost card in hand)."""
    cmd = f"upgrade {hand_index}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result, _combat_context())


@dev.command("remove")
@click.argument("card_id")
@click.argument("pile", default="hand")
@click.pass_context
def remove_card(ctx: click.Context, card_id: str, pile: str) -> None:
    """Remove CARD_ID from PILE (default: hand)."""
    cmd = (
        f"remove_card {card_id} {pile}" if pile != "hand" else f"remove_card {card_id}"
    )
    result = _dev_exec(cmd)
    _dev_out(cmd, result)


# ── loot ──────────────────────────────────────────────────────────────────────


@dev.command()
@click.argument("amount", type=int)
@click.pass_context
def gold(ctx: click.Context, amount: int) -> None:
    """Add AMOUNT gold."""
    cmd = f"gold {amount}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result)
    click.echo(_hint(["Run `sts2 player` to verify"]))


@dev.command()
@click.argument("relic_id")
@click.argument("operation", default="add")
@click.pass_context
def relic(ctx: click.Context, relic_id: str, operation: str) -> None:
    """Add or remove a relic. OPERATION is 'add' (default) or 'remove'."""
    if operation not in ("add", "remove"):
        click.echo(
            _error(
                f"unknown operation '{operation}'", "sts2 dev relic <ID> [add|remove]"
            )
        )
        sys.exit(2)
    cmd = (
        f"relic {operation} {relic_id}"
        if operation == "remove"
        else f"relic {relic_id}"
    )
    result = _dev_exec(cmd)
    _dev_out(cmd, result)


@dev.command()
@click.argument("potion_id")
@click.pass_context
def potion(ctx: click.Context, potion_id: str) -> None:
    """Add a potion to your belt."""
    cmd = f"potion {potion_id}"
    result = _dev_exec(cmd)
    _dev_out(cmd, result)
