"""sts2 — bridge CLI for inspecting and controlling STS2 via MCPTest (port 21337).

Usage:
    sts2 [--host HOST] [--json] <command>

Set STS2_BRIDGE_HOST env var for WSL (e.g. export STS2_BRIDGE_HOST=172.x.x.1).
"""

from __future__ import annotations

import json
import os
import shutil
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


def _first_player(data: Any) -> dict[str, Any]:
    """Extract the first player dict from a bridge response.

    The MCPTest bridge wraps player data in a ``players`` array.
    This helper normalises both the wrapped form and the flat form.
    """
    if isinstance(data, dict):
        players = data.get("players")
        if isinstance(players, list) and players:
            return players[0]
    return {}  # type: ignore[return-value]


def _call(fn, *args, **kwargs) -> dict:
    result = fn(*args, **kwargs)
    # send_request returns raw {"result": {...}} — unwrap it
    if isinstance(result, dict):
        result = result.get("result", result)
    if not isinstance(result, dict):
        click.echo(
            _error(
                f"Bridge returned {type(result).__name__} instead of dict",
                "The bridge may have sent malformed or double-encoded data",
            )
        )
        sys.exit(1)
    if result.get("error"):
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


def _home_hints(
    screen: str, context_type: str = "", actions: list[dict] | None = None
) -> list[str]:
    """Context-aware hints for the no-args home view.

    Uses the *actions* list to detect multi-step flow phases (e.g. Neow event
    pick-blessing vs proceed-to-map, or card-upgrade select vs confirm).
    """
    s = (screen or "").upper()
    c = (context_type or "").upper()
    action_types = {a.get("action", "") for a in (actions or [])}

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
    if s == "EVENT":
        if "event_option" in action_types:
            return [
                "Run `sts2 actions` to see blessing options",
                "Run `sts2 act <n>` to pick a blessing by index",
                "Run `sts2 dev fight <ID>` to skip ahead to a specific fight",
            ]
        if "event_proceed" in action_types:
            return [
                "Run `sts2 actions` to confirm the next step",
                "Run `sts2 act <n>` to proceed to the map",
                "Run `sts2 dev fight <ID>` to jump straight to a fight",
            ]
        return [
            "Run `sts2 actions` to see what you can do on this screen",
            "Run `sts2 dev fight <ID>` to jump to a fight (e.g. `sts2 dev fight JAW_WORM`)",
            "Run `sts2 start` to begin a new run",
        ]
    if s == "CARD_SELECTION":
        if "upgrade" in c or "smith" in c:
            return [
                "Run `sts2 act <n>` to select a card to upgrade, then confirm",
                "Run `sts2 actions` to see cards",
                "Run `sts2 dev fight <ID>` to skip this selection",
            ]
        if "remove" in c or "purge" in c:
            return [
                "Run `sts2 act <n>` to select a card to remove, then confirm",
                "Run `sts2 actions` to see cards",
                "Run `sts2 dev fight <ID>` to skip this selection",
            ]
        if "transform" in c:
            return [
                "Run `sts2 act <n>` to select a card to transform, then confirm",
                "Run `sts2 actions` to see cards",
                "Run `sts2 dev fight <ID>` to skip this selection",
            ]
        # Generic card selection (e.g. draft, scry)
        return [
            "Run `sts2 act <n>` to select a card, then confirm",
            "Run `sts2 actions` to see cards",
            "Run `sts2 dev fight <ID>` to skip this selection",
        ]
    if s == "CARD_SELECTION":
        return [
            "Run `sts2 actions` to see selectable cards",
            "Run `sts2 act <n>` to pick a card (selection + confirm happen automatically)",
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

    # get_full_state stores player/run info under "player" (a bridge response
    # with a "players" array) and combat info under "combat".
    player_data = _first_player(data.get("player", {}))
    combat_data = data.get("combat", {})
    combat_player = _first_player(combat_data)
    run_data = data.get("run", {})

    hp = f"{player_data.get('hp', combat_player.get('hp', run_data.get('hp', '?')))}/{player_data.get('max_hp', combat_player.get('max_hp', run_data.get('max_hp', '?')))}"
    gold = player_data.get("gold", combat_player.get("gold", run_data.get("gold", "?")))
    energy = f"{combat_player.get('energy', data.get('energy', '?'))}/{combat_player.get('max_energy', data.get('max_energy', '?'))}"

    # get_full_state now stores run info under "run" (floor, act, seed).
    # Fall back to player/combat data or top-level keys for older bridges.
    parts.append(
        _block(
            "game",
            {
                "screen": screen,
                "floor": run_data.get(
                    "floor", player_data.get("floor", combat_data.get("floor", "?"))
                ),
                "act": run_data.get(
                    "act", player_data.get("act", combat_data.get("act", "?"))
                ),
                "hp": hp,
                "gold": gold,
                "energy": energy,
            },
        )
    )
    hand = combat_player.get("hand", data.get("hand", []))
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
    enemies = combat_data.get("enemies", data.get("enemies", []))
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
    available_actions = data.get("available_actions", {})
    context_type = (available_actions or {}).get("screen_context_type", "")
    action_list = (available_actions or {}).get("actions", [])
    # Show a compact action preview so the agent can act without a follow-up call
    if action_list:
        preview_limit = 8 if "COMBAT" in (screen or "").upper() else 5
        preview = action_list[:preview_limit]
        parts.append(
            _table(
                "actions",
                [
                    {
                        "n": i,
                        "label": _action_label(a),
                        "status": _action_status(a, action_list),
                    }
                    for i, a in enumerate(preview)
                ],
                ["n", "label", "status"],
            )
        )
    parts.append(_hint(_home_hints(screen, context_type, action_list)))
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
    player = _first_player(data)
    click.echo(
        _block(
            "run",
            {
                "floor": data.get("floor", "?"),
                "act": data.get("act", "?"),
                "hp": f"{player.get('hp', data.get('current_hp', '?'))}/{player.get('max_hp', data.get('max_hp', '?'))}",
                "gold": player.get("gold", data.get("gold", "?")),
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
    data = _call(bc.get_card_piles)
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
    p = _first_player(data)
    parts: list[str] = []
    parts.append(
        _block(
            "player",
            {
                "character": p.get("character", data.get("character", "?")),
                "hp": f"{p.get('hp', data.get('current_hp', '?'))}/{p.get('max_hp', data.get('max_hp', '?'))}",
                "gold": p.get("gold", data.get("gold", "?")),
            },
        )
    )
    deck = p.get("deck", data.get("deck", []))
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
    relics = p.get("relics", data.get("relics", []))
    if relics:
        parts.append(
            _table(
                "relics",
                [
                    {
                        "name": r.get("name", "?"),
                        "rarity": r.get("rarity", "?"),
                    }
                    for r in relics
                ],
                ["name", "rarity"],
            )
        )
    potions = p.get("potions", data.get("potions", []))
    if potions:
        parts.append(
            _table(
                "potions",
                [
                    {
                        "slot": po.get("slot", "?"),
                        "name": po.get("name", "?"),
                    }
                    for po in potions
                ],
                ["slot", "name"],
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
                        "node": f"{n.get('row', '?')},{n.get('col', '?')}",
                        "type": n.get("type", "?"),
                        "available": "yes" if n.get("available") else "no",
                    }
                    for n in nodes
                ],
                ["node", "type", "available"],
            )
        )
    click.echo("\n".join(parts))


@main.command()
@click.option("--lines", default=20, show_default=True)
@click.pass_context
def log(ctx: click.Context, lines: int) -> None:
    """Recent game log entries."""
    data = _call(bc.get_game_log, max_count=lines)
    if ctx.obj["as_json"]:
        click.echo(json.dumps(data, indent=2))
        return
    entries = data.get("entries", data.get("log", []))
    for entry in entries[-lines:]:
        if isinstance(entry, dict):
            ts = entry.get("timestamp", entry.get("time", ""))
            click.echo(f"  {ts} {entry.get('message', entry)}")
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


def _action_status(action: dict, all_actions: list[dict]) -> str:
    """Return 'ready' or 'blocked' for an action based on the current screen state."""
    t = action.get("action", "")
    if t == "event_proceed":
        # Blocked if there are still blessings/options to pick
        if any(a.get("action") == "event_option" for a in all_actions):
            return "blocked"
        return "ready"
    if t == "card_confirm":
        # Blocked if there are selectable cards but none selected yet
        # (The bridge handles selection state; we approximate: if card_select
        # actions exist, confirm is likely blocked until one is chosen.)
        if any(a.get("action") == "card_select" for a in all_actions):
            return "blocked"
        return "ready"
    if t == "card_skip":
        # Often blocked on mandatory selections; we can't detect this server-side
        # so we mark it ready and let the bridge reject it.
        return "ready"
    return "ready"


def _execute(action: dict) -> dict:
    if action.get("action") == "travel":
        row, col = action["node"].split(",")
        return _call(bc.navigate_map, int(row), int(col))
    params = {k: v for k, v in action.items() if k != "action" and v is not None}
    # card_select needs confirm=True so the pick is atomic (confirm button is
    # otherwise reported as blocked and requires a ForceClick that bypasses it).
    if action.get("action") == "card_select":
        params["confirm"] = True
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
                {
                    "n": i,
                    "action": a.get("action", "?"),
                    "label": _action_label(a),
                    "status": _action_status(a, action_list),
                }
                for i, a in enumerate(action_list)
                # card_confirm is now handled automatically by card_select --confirm=True
                if a.get("action") != "card_confirm"
            ],
            ["n", "action", "label", "status"],
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
    status = _action_status(chosen, action_list)
    if status == "blocked":
        label = _action_label(chosen)
        click.echo(
            _error(
                f"Action '{label}' is not valid right now",
                "Choose a required option first, then proceed",
            )
        )
        sys.exit(1)
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
    # Phase-aware hint for the new state after the action
    new_actions = result.get("available_actions", {})
    new_context = (new_actions or {}).get("screen_context_type", "")
    new_action_list = (new_actions or {}).get("actions", [])
    parts.append(_hint(_home_hints(screen, new_context, new_action_list)))
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


# ── hook setup ────────────────────────────────────────────────────────────────

_OPENCODE_PLUGIN_TEMPLATE = """
// sts2 OpenCode plugin — injects live game state as ambient context.
// Auto-installed by `sts2 setup-hook --app opencode`.
// Remove with `sts2 setup-hook --app opencode --remove`.

export const Sts2Plugin = async ({ $ }) => {
  return {
    // Auto-detect WSL bridge host so STS2_BRIDGE_HOST never needs manual export.
    "shell.env": async (input, output) => {
      if (!process.env.STS2_BRIDGE_HOST) {
        try {
          const ns = await $`grep -m1 '^nameserver' /etc/resolv.conf`.text()
          const ip = ns.trim().split(/\s+/)[1]
          if (ip) output.env.STS2_BRIDGE_HOST = ip
        } catch {
          // Not WSL or resolv.conf missing — leave unset, falls back to 127.0.0.1
        }
      }
    },

    // Inject live game state whenever the session compacts (keeps context fresh).
    "experimental.session.compacting": async (input, output) => {
      try {
        const state = await $`STS2_BIN_PATH 2>/dev/null`.text()
        if (state?.trim()) {
          output.context.push(`## STS2 live state\n${state.trim()}`)
        }
      } catch {
        // Bridge not running — skip silently
      }
    },
  }
}
""".strip()


def _resolve_bin() -> str:
    """Return the portable sts2 binary path (name if on PATH, else absolute)."""

    bin_name = "sts2"
    resolved = shutil.which(bin_name)
    if resolved and os.path.realpath(resolved) == os.path.realpath(sys.argv[0]):
        return bin_name
    return os.path.realpath(sys.argv[0])


def _setup_claude_code(project_root: str, bin_path: str, remove: bool) -> None:
    settings_path = os.path.join(project_root, ".claude", "settings.json")
    hook_command = (
        f"export STS2_BRIDGE_HOST=$("
        f"grep '^nameserver' /etc/resolv.conf 2>/dev/null "
        f"| awk 'NR==1{{print $2}}' || echo 127.0.0.1"
        f"); {bin_path} 2>/dev/null || true"
    )
    hook_block = {
        "matcher": "",
        "hooks": [{"type": "command", "command": hook_command, "timeout": 10}],
    }

    os.makedirs(os.path.dirname(settings_path), exist_ok=True)
    try:
        with open(settings_path) as f:
            settings = json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        settings = {}

    session_hooks: list[dict] = settings.setdefault("SessionStart", [])
    existing = [
        i
        for i, b in enumerate(session_hooks)
        for h in b.get("hooks", [])
        if bin_path in h.get("command", "") or "sts2" in h.get("command", "")
    ]

    if remove:
        if not existing:
            click.echo(
                _block(
                    "hook", {"app": "claude-code", "status": "not installed (no-op)"}
                )
            )
            return
        for i in sorted(existing, reverse=True):
            session_hooks.pop(i)
        with open(settings_path, "w") as f:
            json.dump(settings, f, indent=2)
        click.echo(
            _block(
                "hook",
                {"app": "claude-code", "status": "removed", "settings": settings_path},
            )
        )
        return

    if existing:
        click.echo(
            _block(
                "hook",
                {
                    "app": "claude-code",
                    "status": "already installed (no-op)",
                    "settings": settings_path,
                },
            )
        )
        return

    session_hooks.append(hook_block)
    with open(settings_path, "w") as f:
        json.dump(settings, f, indent=2)
    click.echo(
        _block(
            "hook",
            {
                "app": "claude-code",
                "status": "installed",
                "trigger": "SessionStart",
                "settings": settings_path,
            },
        )
    )


def _setup_opencode(project_root: str, bin_path: str, remove: bool) -> None:
    plugin_dir = os.path.join(project_root, ".opencode", "plugins")
    plugin_path = os.path.join(plugin_dir, "sts2.ts")

    if remove:
        if not os.path.exists(plugin_path):
            click.echo(
                _block("hook", {"app": "opencode", "status": "not installed (no-op)"})
            )
            return
        os.remove(plugin_path)
        click.echo(
            _block(
                "hook", {"app": "opencode", "status": "removed", "plugin": plugin_path}
            )
        )
        return

    if os.path.exists(plugin_path):
        click.echo(
            _block(
                "hook",
                {
                    "app": "opencode",
                    "status": "already installed (no-op)",
                    "plugin": plugin_path,
                },
            )
        )
        return

    os.makedirs(plugin_dir, exist_ok=True)
    plugin_src = _OPENCODE_PLUGIN_TEMPLATE.replace("STS2_BIN_PATH", bin_path)
    with open(plugin_path, "w") as f:
        f.write(plugin_src + "\n")
    click.echo(
        _block(
            "hook", {"app": "opencode", "status": "installed", "plugin": plugin_path}
        )
    )


@main.command("setup-hook")
@click.option("--project-dir", default=None, help="Project root. Defaults to cwd.")
@click.option(
    "--app",
    type=click.Choice(["claude-code", "opencode", "all"], case_sensitive=False),
    default="all",
    show_default=True,
    help="Which agent app to install the hook for.",
)
@click.option("--remove", is_flag=True, help="Remove the hook instead of installing.")
def setup_hook(project_dir: str | None, app: str, remove: bool) -> None:
    """Install agent session hooks that show live STS2 game state as ambient context.

    Supports Claude Code (SessionStart hook in .claude/settings.json) and
    OpenCode (plugin in .opencode/plugins/sts2.ts). Both auto-detect the WSL
    bridge host from /etc/resolv.conf — no manual STS2_BRIDGE_HOST export needed.

    Examples:

    \b
      sts2 setup-hook                        # install for all apps
      sts2 setup-hook --app claude-code      # Claude Code only
      sts2 setup-hook --app opencode         # OpenCode only
      sts2 setup-hook --remove               # remove all
    """
    import os

    project_root = project_dir or os.getcwd()
    bin_path = _resolve_bin()

    if app in ("claude-code", "all"):
        _setup_claude_code(project_root, bin_path, remove)
    if app in ("opencode", "all"):
        _setup_opencode(project_root, bin_path, remove)

    if not remove:
        click.echo(
            _hint(
                [
                    "Restart your agent session to activate",
                    "Run `sts2 setup-hook --remove` to uninstall",
                ]
            )
        )
