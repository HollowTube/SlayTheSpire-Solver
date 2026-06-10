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
    {"legal_actions": [...], "values": {action: float}, "state_value": float}

`build_state` is a pure function from the JSON `state` dict to a
`CombatState`, reusing the snapshot-reconstruction constructor parameters
(`player_block`, `player_statuses`, `turn`, the card piles, and per-monster
`block`/`statuses`/`intent`/`last_move`/`move_streak`) so a mid-fight game
state can be reconstructed faithfully, not just a fresh turn-0 combat.
"""

import argparse
import json
import socketserver

from . import CombatState, Monster, evaluate, legal_actions
from . import mcts as _mcts

DEFAULT_PORT = 8765


def _statuses(entries):
    """Translate a JSON list of `[name, amount]` pairs into the list of
    `(name, amount)` tuples the `CombatState`/`Monster` constructors expect."""
    return [(name, amount) for name, amount in entries]


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
            intent=m.get("intent"),
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
        hand=state.get("hand", []),
        player_max_hp=player.get("max_hp"),
        player_max_energy=player.get("max_energy"),
        player_block=player.get("block", 0),
        player_statuses=_statuses(player.get("statuses", [])),
        turn=state.get("turn", 0),
        draw_pile=state.get("draw_pile", []),
        discard_pile=state.get("discard_pile", []),
        exhaust_pile=state.get("exhaust_pile", []),
    )


def handle_request(payload):
    """Dispatch one decoded request to its handler and return a dict to be
    JSON-encoded as the response. `cmd` is dispatched explicitly so future
    commands can be added without changing the wire format of `"analyze"`."""
    cmd = payload.get("cmd")
    if cmd == "analyze":
        state = build_state(payload)
        iterations = payload.get("iterations", _mcts.DEFAULT_ITERATIONS)
        actions = legal_actions(state)
        values = _mcts.action_values(state, iterations=iterations)
        state_value = max(values.values()) if values else evaluate(state)
        return {
            "legal_actions": actions,
            "values": values,
            "state_value": state_value,
        }
    raise ValueError(f"unknown cmd: {cmd!r}")


class _Handler(socketserver.StreamRequestHandler):
    def handle(self):
        for line in self.rfile:
            line = line.decode("utf-8").strip()
            if not line:
                continue
            try:
                response = handle_request(json.loads(line))
            except Exception as exc:  # noqa: BLE001 - report to client, don't crash
                response = {"error": str(exc)}
            self.wfile.write((json.dumps(response) + "\n").encode("utf-8"))


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def make_server(host="127.0.0.1", port=DEFAULT_PORT):
    """Construct (but don't start) a `Server` bound to `(host, port)`. Pass
    `port=0` to let the OS assign a free port (useful for tests) — read the
    actual port back via `server.server_address`."""
    return Server((host, port), _Handler)


def serve(host="127.0.0.1", port=DEFAULT_PORT):
    """Bind and serve forever. The CLI entry point."""
    with make_server(host, port) as server:
        host, port = server.server_address
        print(f"sts_sim analysis server listening on {host}:{port}")
        server.serve_forever()


def main():
    parser = argparse.ArgumentParser(description="sts_sim TCP/JSON analysis server")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    args = parser.parse_args()
    serve(args.host, args.port)


if __name__ == "__main__":
    main()
