"""TCP/JSON analysis server: the bridge between an external process (e.g. an
in-game overlay mod) and this package's `evaluate`/`mcts.action_values`.

Protocol: line-delimited JSON over TCP. Each request is a single JSON object
on its own line; each response is a single JSON object on its own line.

Request (`cmd: "analyze"`):
    {"cmd": "analyze", "iterations": 200, "seed": 12345,
     "state": {"player": {...}, "hand": [...], "draw_pile": [...],
               "discard_pile": [...], "exhaust_pile": [...], "turn": 3,
               "monsters": [{...}, ...]}}

Response:
    {"legal_actions": [...], "values": {action: float}, "state_value": float,
     "expected_hp_lost": float, "action_hp_lost": {action: float}}

Request (`cmd: "deck_baseline"`):
    {"cmd": "deck_baseline", "deck": [...],
     "monsters": [{"name": "Fuzzy Wurm Crawler", "intent": "ACID_GOOP",
                   "statuses": [["Vulnerable", 1]]}],
     "seeds": 30, "iterations": 200}

    Legacy form (single named encounter, still supported):
    {"cmd": "deck_baseline", "deck": [...], "monster": "fuzzy-wurm-crawler",
     "seeds": 30, "iterations": 200}

Response:
    {"mean_hp_lost": float, "win_rate": float}

`deck_baseline` is a "deck vs. monster, before any cards are drawn" figure:
it runs `sts_sim.bench.run_deck` over `seeds` fresh fights (full player HP,
the monster's canonical starting HP/stats, the given `deck` reshuffled per
seed) and reports the average HP lost over a full fight under MCTS play. This
is independent of any specific draw, unlike `expected_hp_lost` (which is
anchored to the current, already-drawn state) - intended as a one-time
per-fight baseline computed when combat starts.

`state_value`/`values` are `evaluate`'s `player_fraction - monsters_fraction`
(see `src/lib.rs`), roughly -1..1. `action_hp_lost` converts each action's
value into an absolute HP figure the same way: the player's
fraction-remaining is approximated as `clamp(value, 0, 1)` (a winning value
~= player_fraction since monsters_fraction ~= 0; a non-positive value is
treated as "dead, 0 HP remaining"), then `hp_lost = player_hp -
remaining_fraction * player_max_hp`. This is a quick per-action ranking
signal, not a calibrated HP estimate — `action_values` averages over MCTS
rollouts whose tails play uniformly at random, which inflates the figure well
above what skilled play would actually lose.

`expected_hp_lost` (the top-line, "if I keep playing this out" number) is
instead `simulate_hp_lost` averaged over a few seeds: it plays the fight to
completion choosing each move via `mcts_search` against the true state
(non-clairvoyant - it never sees future draws), then reports the player's
actual HP lost. That mirrors a chess engine's principal-variation eval rather
than a single rollout-poisoned position value.

`build_state` is a pure function from the JSON `state` dict to a
`CombatState`, reusing the snapshot-reconstruction constructor parameters
(`player_block`, `player_statuses`, `turn`, the card piles, and per-monster
`block`/`statuses`/`intent`/`last_move`/`move_streak`) so a mid-fight game
state can be reconstructed faithfully, not just a fresh turn-0 combat.
"""

import argparse
import http.server
import json
import os
import socketserver
import sys
import threading
import time

from . import CombatState, Monster, apply, evaluate, legal_actions, simulate_hp_lost
from . import mcts as _mcts
from . import names as _names

DEFAULT_PORT = 8765

# Number of independent simulate_hp_lost playouts averaged into the top-line
# expected_hp_lost. Each playout re-runs MCTS search at every decision until
# the fight ends, so this multiplies the request's cost roughly
# `playouts * (decisions per fight)`-fold over a single analyze call.
DEFAULT_PLAYOUTS = 3

# Number of independent seeds (fresh shuffles/draws) averaged into
# "deck_baseline"'s mean_hp_lost. run_deck's fight_outcomes_per_fight path
# runs all seeds in one rayon-parallel Rust call, so 30 seeds is still under
# 2s at the default iteration count.
DEFAULT_DECK_BASELINE_SEEDS = 30


def _statuses(entries):
    """Translate a JSON list of `[name, amount]` pairs into the list of
    `(name, amount)` tuples the `CombatState`/`Monster` constructors expect."""
    return [(name, amount) for name, amount in entries]


def _hand_ids(hand: list) -> list:
    """Extract card IDs from a hand list that may be plain strings or [id, cost] pairs."""
    return [e[0] if isinstance(e, list) else e for e in hand]


def _hand_costs(hand: list) -> dict:
    """Return {card_id: cost_str} for display. -1 → 'X', else str(n)."""
    result = {}
    for e in hand:
        if isinstance(e, list):
            card_id, cost = e[0], e[1]
            result[card_id] = "X" if cost == -1 else str(cost)
    return result


def build_state(payload):
    """Reconstruct a `CombatState` from an "analyze" request's `state` dict."""
    state = payload["state"]
    player = state["player"]
    monsters = [
        Monster(
            hp=m["hp"],
            attack=m.get("attack", 0),
            max_hp=m.get("max_hp"),
            name=m.get("name"),
            block=m.get("block", 0),
            statuses=_statuses(m.get("statuses", [])),
            intent=_names.intent(m.get("name"), m.get("intent")),
            last_move=m.get("last_move"),
            move_streak=m.get("move_streak", 0),
        )
        for m in state["monsters"]
    ]
    return CombatState(
        player_hp=player["hp"],
        player_energy=player["energy"],
        monsters=monsters,
        seed=payload.get("seed", 0),
        hand=_hand_ids(state.get("hand", [])),
        player_max_hp=player.get("max_hp"),
        player_max_energy=player.get("max_energy"),
        player_block=player.get("block", 0),
        player_statuses=_statuses(player.get("statuses", [])),
        turn=state.get("turn", 0),
        draw_pile=state.get("draw_pile", []),
        discard_pile=state.get("discard_pile", []),
        exhaust_pile=state.get("exhaust_pile", []),
        attacks_played_this_turn=state.get("attacks_played_this_turn", 0),
    )


def _expected_hp_lost(value, state):
    """Convert an `evaluate`-style value (`player_fraction -
    monsters_fraction`, roughly -1..1) into an absolute expected-HP-lost
    figure — see the module docstring for the approximation used."""
    remaining_fraction = min(max(value, 0.0), 1.0)
    expected_final_hp = remaining_fraction * state.player_max_hp
    return max(state.player_hp - expected_final_hp, 0.0)


def _simulated_expected_hp_lost(state, iterations, seed, playouts):
    """Average `simulate_hp_lost` over `playouts` seeds derived from `seed`:
    each playout plays the fight to completion picking every move via
    `mcts_search` against the true state (non-clairvoyant), then reports the
    player's actual HP lost. See the module docstring for why this — rather
    than `_expected_hp_lost` over `state_value` — is the top-line figure."""
    losses = [
        simulate_hp_lost(state, iterations=iterations, seed=seed + i)
        for i in range(playouts)
    ]
    return sum(losses) / len(losses)


def handle_request(payload):
    """Dispatch one decoded request to its handler and return a dict to be
    JSON-encoded as the response. `cmd` is dispatched explicitly so future
    commands can be added without changing the wire format of `"analyze"`."""
    cmd = payload.get("cmd")
    if cmd == "analyze":
        state = build_state(payload)
        iterations = payload.get("iterations", _mcts.DEFAULT_ITERATIONS)
        seed = payload.get("seed", 0)
        playouts = payload.get("playouts", DEFAULT_PLAYOUTS)
        actions = legal_actions(state)
        values = _mcts.action_values(state, iterations=iterations)
        state_value = max(values.values()) if values else evaluate(state)
        # Per-target greedy evaluation for single-target cards in multi-monster
        # fights. MCTS action_values already fold in the optimal target
        # internally; this tells the player *which* target that is. Uses
        # evaluate() (one-step greedy) rather than another MCTS pass — the
        # ranking across targets is what matters, not calibrated HP estimates.
        from . import PlayCardAction, SelectTargetAction

        target_values = {}
        if len(state.monsters) > 1:
            for action in actions:
                if not isinstance(action, PlayCardAction):
                    continue
                try:
                    mid = apply(state, action)
                except Exception:
                    continue
                sub = legal_actions(mid)
                if not sub or not isinstance(sub[0], SelectTargetAction):
                    continue
                tv = {}
                for sub_action in sub:
                    try:
                        tv[str(sub_action)] = evaluate(apply(mid, sub_action))
                    except Exception:
                        pass
                if tv:
                    target_values[str(action)] = tv
        return {
            "legal_actions": [str(a) for a in actions],
            "values": {str(a): v for a, v in values.items()},
            "state_value": state_value,
            "expected_hp_lost": _simulated_expected_hp_lost(
                state, iterations, seed, playouts
            ),
            "action_hp_lost": {
                str(a): _expected_hp_lost(v, state) for a, v in values.items()
            },
            "target_values": target_values,
        }
    if cmd == "deck_baseline":
        from .bench import run_deck
        from .scenarios import (
            IRONCLAD_STARTING_DECK,
            MONSTER_STARTING_HP,
            PLAYER_STARTING_HP,
        )

        monsters_list = payload.get("monsters")
        if monsters_list is not None:
            # Dynamic monsters protocol: build a scenario_fn closure from the
            # live monster list rather than a named Encounter key. Each entry
            # must have "name" (sts_sim monster name) and may have "intent"
            # (raw STS2 move id) and "statuses" ([[name, amount], ...]).
            monster_specs = []
            for m in monsters_list:
                name = m["name"]
                hp = MONSTER_STARTING_HP.get(name)
                if hp is None:
                    raise ValueError(
                        f"unknown monster name for deck_baseline: {name!r}"
                    )
                monster_specs.append(
                    {
                        "name": name,
                        "hp": hp,
                        "intent": _names.intent(name, m.get("intent")),
                        "statuses": _statuses(m.get("statuses", [])),
                    }
                )

            def _scenario_fn(seed, deck):
                return CombatState(
                    player_hp=PLAYER_STARTING_HP,
                    player_energy=3,
                    monsters=[
                        Monster(
                            hp=spec["hp"],
                            attack=0,
                            name=spec["name"],
                            intent=spec["intent"],
                            statuses=spec["statuses"],
                        )
                        for spec in monster_specs
                    ],
                    seed=seed,
                    deck=list(deck if deck is not None else IRONCLAD_STARTING_DECK),
                )

            result = run_deck(
                payload.get("deck"),
                scenario_fn=_scenario_fn,
                seeds=payload.get("seeds", DEFAULT_DECK_BASELINE_SEEDS),
                iterations=payload.get("iterations", _mcts.DEFAULT_ITERATIONS),
            )
        else:
            result = run_deck(
                payload.get("deck"),
                monster=payload["monster"],
                seeds=payload.get("seeds", DEFAULT_DECK_BASELINE_SEEDS),
                iterations=payload.get("iterations", _mcts.DEFAULT_ITERATIONS),
            )
        return {
            "mean_hp_lost": result.mean_hp_lost,
            "win_rate": result.win_rate,
        }
    raise ValueError(f"unknown cmd: {cmd!r}")


# Append-mode log file; None until serve() sets it. Written atomically per
# line (os.write) so threads don't interleave, and bypasses Python buffering.
_log_file: "int | None" = None

# Last analyzed state — updated on every analyze request, read by the debug UI.
_last_debug: dict = {}
_debug_lock = threading.Lock()


def _debug_html() -> str:
    with _debug_lock:
        d = dict(_last_debug)
    if not d:
        return "<p>No analysis received yet.</p>"

    state = d.get("state", {})
    player = state.get("player", {})
    vals = d.get("values", {})
    best_val = max(vals.values()) if vals else 0.0

    rows = ""
    for a, v in sorted(vals.items(), key=lambda x: -x[1]):
        delta = v - best_val
        colour = "#4caf50" if delta == 0 else "#ccc"
        rows += (
            f"<tr>"
            f"<td style='color:{colour};font-weight:bold'>{a.replace('PlayCard:', '')}</td>"
            f"<td style='color:{colour}'>{delta:+.3f}</td>"
            f"<td>{v:.3f}</td>"
            f"</tr>"
        )

    costs = d.get("hand_costs", {})
    hand_cards = "".join(
        f"<span style='background:#333;border-radius:4px;padding:4px 8px;margin:2px;display:inline-block'>"
        f"<span style='background:#555;border-radius:3px;padding:1px 5px;margin-right:5px;font-size:11px'>"
        f"{costs.get(c, '?')}</span>{c}</span>"
        for c in _hand_ids(state.get("hand", []))
    )
    statuses = player.get("statuses") or []
    status_str = ", ".join(f"{s[0]}={s[1]}" for s in statuses) if statuses else "none"

    monsters_html = ""
    for m in state.get("monsters", []):
        hp_pct = int(100 * m.get("hp", 0) / max(m.get("max_hp", 1), 1))
        intent = m.get("intent", "?")
        atk = m.get("attack", 0)
        monsters_html += (
            f"<div style='margin:6px 0;padding:8px;background:#222;border-radius:4px'>"
            f"<b>{m.get('name', '?')}</b> &nbsp; "
            f"HP: {m.get('hp', '?')}/{m.get('max_hp', '?')} "
            f"<div style='background:#555;border-radius:2px;width:200px;display:inline-block;height:8px;vertical-align:middle'>"
            f"<div style='background:#e53;width:{hp_pct}%;height:8px;border-radius:2px'></div></div> &nbsp;"
            f"Block: {m.get('block', 0)} &nbsp;"
            f"Intent: <b>{intent}</b>"
            f"{f' ({atk} dmg)' if atk else ''}"
            f"</div>"
        )

    elapsed_ms = d.get("elapsed_ms", 0)
    ts = d.get("ts", "?")

    return f"""
<p style='color:#888;font-size:12px'>Updated {ts} &nbsp;·&nbsp; {elapsed_ms:.0f}ms analysis</p>
<div style='margin-bottom:12px'>
  <b>Player</b>: HP {player.get("hp", "?")}/{player.get("max_hp", "?")} &nbsp;
  Block: {player.get("block", 0)} &nbsp;
  Energy: {player.get("energy", "?")}/{player.get("max_energy", "?")} &nbsp;
  Statuses: {status_str}
</div>
<div style='margin-bottom:12px'><b>Hand</b>: {hand_cards}</div>
<div style='margin-bottom:12px'><b>Monsters</b>:{monsters_html}</div>
<table style='border-collapse:collapse;width:100%'>
  <tr style='color:#888'><th style='text-align:left'>Action</th><th>Δ</th><th>Value</th></tr>
  {rows}
</table>
"""


_DEBUG_PAGE = """<!doctype html>
<html><head><meta charset=utf-8><title>sts-sim debug</title>
<style>
  body {{background:#111;color:#eee;font-family:monospace;padding:20px;}}
  table {{width:100%;border-collapse:collapse;}}
  td,th {{padding:4px 10px;text-align:left;border-bottom:1px solid #333;}}
  th {{color:#888;font-weight:normal;}}
</style>
</head><body>
<h2 style='color:#fff;margin-top:0'>sts-sim debug <span style='font-size:14px;color:#888'>auto-refresh 1s</span></h2>
<div id=content>Loading...</div>
<script>
async function refresh() {{
  const r = await fetch('/state');
  const html = await r.text();
  document.getElementById('content').innerHTML = html;
}}
refresh();
setInterval(refresh, 1000);
</script>
</body></html>"""


class _DebugHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass  # silence access log

    def do_GET(self):
        if self.path == "/state":
            body = _debug_html().encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            body = _DEBUG_PAGE.encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)


def _emit(line: str) -> None:
    """Write one log line to stdout and the log file (if open)."""
    msg = line + "\n"
    os.write(sys.stdout.fileno(), msg.encode())
    if _log_file is not None:
        os.write(_log_file, msg.encode())


def _log_analyze(payload: dict, response: dict, elapsed: float) -> None:
    """Emit a compact one-line summary of an analyze exchange."""
    state = payload.get("state", {})
    raw_hand = state.get("hand", [])
    hand = _hand_ids(raw_hand)
    costs = _hand_costs(raw_hand)
    player_statuses = state.get("player", {}).get("statuses", [])
    monsters = [
        f"{m.get('name', '?')}({m.get('hp', '?')}/{m.get('max_hp', '?')} {m.get('intent', '')})"
        for m in state.get("monsters", [])
    ]
    vals = response.get("values", {})
    best_action = max(vals, key=vals.get) if vals else "none"
    best_val = vals.get(best_action, 0.0)
    # Show overlay-style deltas: best at 0.00, rest negative — matches the
    # overlay panel exactly so you can compare log to display directly.
    ranking = " | ".join(
        f"{a.replace('PlayCard:', '')}: {v - best_val:+.2f}"
        for a, v in sorted(vals.items(), key=lambda x: -x[1])
    )
    ts = time.strftime("%H:%M:%S")
    attacks_played = state.get("attacks_played_this_turn", 0)
    status_str = f"  player_statuses={player_statuses}" if player_statuses else ""
    atk_str = f"  attacks_played={attacks_played}" if attacks_played else ""
    _emit(
        f"[{ts}] analyze {elapsed * 1000:.0f}ms"
        f"  hand={hand}"
        f"{status_str}"
        f"{atk_str}"
        f"  vs {monsters}"
        f"  overlay: [{ranking}]"
    )
    with _debug_lock:
        _last_debug.clear()
        _last_debug.update(
            {
                **response,
                "state": payload.get("state", {}),
                "hand_costs": costs,
                "ts": ts,
                "elapsed_ms": elapsed * 1000,
            }
        )


class _Handler(socketserver.StreamRequestHandler):
    def handle(self):
        for line in self.rfile:
            line = line.decode("utf-8").strip()
            if not line:
                continue
            t0 = time.monotonic()
            try:
                payload = json.loads(line)
                response = handle_request(payload)
            except Exception as exc:  # noqa: BLE001 - report to client, don't crash
                response = {"error": str(exc)}
                payload = {}
            elapsed = time.monotonic() - t0
            if payload.get("cmd") == "analyze":
                _log_analyze(payload, response, elapsed)
            self.wfile.write((json.dumps(response) + "\n").encode("utf-8"))


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def make_server(host="127.0.0.1", port=DEFAULT_PORT):
    """Construct (but don't start) a `Server` bound to `(host, port)`. Pass
    `port=0` to let the OS assign a free port (useful for tests) — read the
    actual port back via `server.server_address`."""
    return Server((host, port), _Handler)


DEFAULT_DEBUG_PORT = 8766

# Path the C# mod reads to discover which WSL IP to connect to.
_MOD_HOST_FILE = "/mnt/d/SteamLibrary/steamapps/common/Slay the Spire 2/mods/stssimbridgemod/sts_sim_host.txt"


def _write_host_file(ip: str) -> None:
    """Write current WSL IP to the mod's sts_sim_host.txt so it survives reboots."""
    try:
        import pathlib

        p = pathlib.Path(_MOD_HOST_FILE)
        if p.parent.exists():
            p.write_text(ip + "\n")
            _emit(f"[server] wrote {ip} -> {_MOD_HOST_FILE}")
    except Exception as exc:
        _emit(f"[server] could not write host file: {exc}")


def serve(
    host="127.0.0.1", port=DEFAULT_PORT, debug_port: int = DEFAULT_DEBUG_PORT
) -> None:
    """Bind and serve forever. The CLI entry point."""
    # Debug HTTP server (auto-refreshing browser UI at http://localhost:<debug_port>)
    debug_server = http.server.HTTPServer(("0.0.0.0", debug_port), _DebugHandler)
    t = threading.Thread(target=debug_server.serve_forever, daemon=True)
    t.start()

    with make_server(host, port) as server:
        host, port = server.server_address
        print(f"sts_sim analysis server listening on {host}:{port}", flush=True)
        try:
            import subprocess

            wsl_ip = (
                subprocess.check_output(["ip", "addr", "show", "eth0"], text=True)
                .split("inet ")[1]
                .split("/")[0]
            )
        except Exception:
            wsl_ip = "localhost"
        _write_host_file(wsl_ip)
        print(f"sts_sim debug UI at http://{wsl_ip}:{debug_port}/", flush=True)
        server.serve_forever()


def main():
    parser = argparse.ArgumentParser(description="sts_sim TCP/JSON analysis server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    parser.add_argument("--debug-port", type=int, default=DEFAULT_DEBUG_PORT)
    args = parser.parse_args()
    serve(args.host, args.port, args.debug_port)


if __name__ == "__main__":
    main()
