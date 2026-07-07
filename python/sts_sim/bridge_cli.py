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
        return c.get("name", c.get("card_name", "?"))
    return str(c)  # raw STS2 ID e.g. "STRIKE_IRONCLAD"


def _card_upgraded(c: Any) -> bool:
    if isinstance(c, dict):
        return c.get("upgraded", False)
    return "+" in str(c)


# ── bridge call wrapper ───────────────────────────────────────────────────────

def _call(fn, *args, **kwargs) -> dict:
    result = fn(*args, **kwargs)
    if isinstance(result, dict) and result.get("error"):
        click.echo(_error(result["error"],
                          "Set STS2_BRIDGE_HOST if running from WSL"))
        sys.exit(1)
    return result


# ── main group ───────────────────────────────────────────────────────────────

@click.group(invoke_without_command=True)
@click.option("--host", envvar="STS2_BRIDGE_HOST", default="127.0.0.1",
              show_default=True, help="Bridge host (set STS2_BRIDGE_HOST for WSL).")
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
    parts.append(_block("game", {
        "screen": screen,
        "floor": run.get("floor", "?"),
        "act": run.get("act", "?"),
        "hp": f"{run.get('current_hp', '?')}/{run.get('max_hp', '?')}",
        "gold": run.get("gold", "?"),
        "energy": f"{data.get('energy', '?')}/{data.get('max_energy', '?')}",
    }))
    hand = data.get("hand", [])
    if hand:
        parts.append(_table("hand", [
            {"name": _card_name(c), "cost": c.get("energy_cost", "?") if isinstance(c, dict) else "?"}
            for c in hand
        ], ["name", "cost"]))
    enemies = data.get("enemies", [])
    if enemies:
        parts.append(_table("enemies", [
            {"name": e.get("name", "?"), "hp": f"{e.get('hp','?')}/{e.get('max_hp','?')}",
             "intent": _fmt_intent(e.get("intent"))}
            for e in enemies
        ], ["name", "hp", "intent"]))
    parts.append(_hint([
        "Run `sts2 actions` to see legal moves",
        "Run `sts2 act <n>` to execute a move",
        "Run `sts2 --help` for all commands",
    ]))
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
    click.echo(_block("bridge", {
        "status": data.get("status", "ok"),
        "version": data.get("version", "?"),
        "screen": data.get("screen", "?"),
    }))


@main.command()
@click.pass_context
def state(ctx: click.Context) -> None:
    """Run state: floor, act, HP, gold, seed."""
    data = _call(bc.get_run_state)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    click.echo(_block("run", {
        "floor": data.get("floor", "?"),
        "act": data.get("act", "?"),
        "hp": f"{data.get('current_hp','?')}/{data.get('max_hp','?')}",
        "gold": data.get("gold", "?"),
        "seed": data.get("seed", "?"),
    }))


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
    parts.append(_block("combat", {
        "screen": data.get("screen", "?"),
        "energy": f"{player.get('energy','?')}/{player.get('max_energy','?')}",
        "turn": data.get("round", "?"),
        "hp": f"{player.get('hp','?')}/{player.get('max_hp','?')}",
        "block": player.get("block", 0),
    }))
    hand = player.get("hand", data.get("hand", []))
    if hand:
        parts.append(_table("hand", [
            {"name": _card_name(c), "cost": c.get("energy_cost", "?") if isinstance(c, dict) else "?",
             "upgraded": _card_upgraded(c)}
            for c in hand
        ], ["name", "cost", "upgraded"]))
    enemies = data.get("enemies", [])
    if enemies:
        parts.append(_table("enemies", [
            {"name": e.get("name", "?"), "hp": f"{e.get('hp','?')}/{e.get('max_hp','?')}",
             "block": e.get("block", 0), "intent": _fmt_intent(e.get("intent"))}
            for e in enemies
        ], ["name", "hp", "block", "intent"]))
    parts.append(_hint(["Run `sts2 actions` to see legal moves", "Run `sts2 act <n>` to execute a move"]))
    click.echo("\n".join(parts))


@main.command()
@click.pass_context
def piles(ctx: click.Context) -> None:
    """Card piles: draw, hand, discard, exhaust (handles STS2 ID strings)."""
    data = _call(bc.get_card_piles)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    parts: list[str] = []
    pile_keys = {"draw": "draw_pile", "hand": "hand", "discard": "discard_pile", "exhaust": "exhaust_pile"}
    for label, key in pile_keys.items():
        raw = data.get(key, data.get(label, []))
        # MCPTest returns {count, cards:[...]}; flat list also accepted
        cards = raw.get("cards", raw) if isinstance(raw, dict) else raw
        if cards:
            parts.append(_table(label, [
                {"name": _card_name(c), "upgraded": _card_upgraded(c),
                 "type": c.get("type", "?") if isinstance(c, dict) else "?",
                 "cost": c.get("energy_cost", "?") if isinstance(c, dict) else "?"}
                for c in cards
            ], ["name", "upgraded", "type", "cost"]))
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
    parts.append(_block("player", {
        "character": data.get("character", "?"),
        "hp": f"{data.get('current_hp','?')}/{data.get('max_hp','?')}",
        "gold": data.get("gold", "?"),
    }))
    deck = data.get("deck", [])
    if deck:
        parts.append(_table("deck", [
            {"name": _card_name(c), "upgraded": _card_upgraded(c),
             "type": c.get("type", "?") if isinstance(c, dict) else "?"}
            for c in deck
        ], ["name", "upgraded", "type"]))
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
    parts.append(_block("map", {"floor": data.get("floor", "?"), "act": data.get("act", "?")}))
    nodes = data.get("available_nodes", data.get("nodes", []))
    if nodes:
        parts.append(_table("paths", [
            {"node": n.get("node", n.get("id", "?")), "type": n.get("type", "?")}
            for n in nodes
        ], ["node", "type"]))
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
        return f"node {a.get('node','?')} ({a.get('type','?')})"
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
        _table("actions", [
            {"n": i, "action": a.get("action", "?"), "label": _action_label(a)}
            for i, a in enumerate(action_list)
        ], ["n", "action", "label"]),
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
        click.echo(_error(f"index {n} out of range (0–{len(action_list)-1})"))
        sys.exit(1)
    chosen = action_list[n]
    result = _execute(chosen)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(result, indent=2))
        return
    parts = [_block("acted", {"action": chosen.get("action", "?"), "label": _action_label(chosen)})]
    screen = result.get("screen", "")
    combat_data = result.get("combat", {})
    if combat_data:
        p = (combat_data.get("players") or [{}])[0]
        parts.append(_block("combat", {
            "screen": screen,
            "energy": f"{p.get('energy','?')}/{p.get('max_energy','?')}",
            "turn": combat_data.get("round", "?"),
            "hp": f"{p.get('hp','?')}/{p.get('max_hp','?')}",
        }))
        hand = p.get("hand", combat_data.get("hand", []))
        if hand:
            parts.append(_table("hand", [
                {"name": _card_name(c), "cost": c.get("energy_cost","?") if isinstance(c, dict) else "?"}
                for c in hand
            ], ["name", "cost"]))
        enemies = combat_data.get("enemies", [])
        if enemies:
            parts.append(_table("enemies", [
                {"name": e.get("name","?"), "hp": f"{e.get('hp','?')}/{e.get('max_hp','?')}",
                 "intent": _fmt_intent(e.get("intent"))}
                for e in enemies
            ], ["name", "hp", "intent"]))
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
def start(ctx: click.Context, char: str, seed: str | None, fight: str | None,
          asc: int, godmode: bool) -> None:
    """Start a new run."""
    data = _call(bc.start_run, character=char, ascension=asc, seed=seed,
                 fight=fight, godmode=godmode)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    click.echo(_block("started", {
        "character": char, "ascension": asc,
        "seed": seed or "(random)", "fight": fight or "(normal)",
    }))
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
    click.echo(_block("console", {
        "command": command,
        "result": data.get("result", data.get("output", "ok")),
    }))
