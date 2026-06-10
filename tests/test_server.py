"""End-to-end tests for the TCP/JSON analysis server: send an "analyze"
request over a real socket and check the response shape, covering both a
fresh turn-0 state and a reconstructed mid-fight snapshot."""

import json
import socket
import threading

from sts_sim import legal_actions
from sts_sim.scenarios import ironclad_starter_deck_vs_jaw_worm
from sts_sim.server import make_server


def _send(server, payload):
    host, port = server.server_address
    with socket.create_connection((host, port)) as sock:
        sock.sendall((json.dumps(payload) + "\n").encode("utf-8"))
        response = b""
        while not response.endswith(b"\n"):
            response += sock.recv(4096)
    return json.loads(response.decode("utf-8"))


def test_analyze_opening_state_returns_values_for_every_legal_action():
    state = ironclad_starter_deck_vs_jaw_worm(seed=42)
    payload = {
        "cmd": "analyze",
        "iterations": 10,
        "seed": 1,
        "state": {
            "player": {"hp": state.player_hp, "energy": state.player_energy},
            "hand": state.hand,
            "draw_pile": state.draw_pile,
            "discard_pile": state.discard_pile,
            "exhaust_pile": state.exhaust_pile,
            "turn": state.turn,
            "monsters": [
                {
                    "name": m.name,
                    "hp": m.hp,
                    "max_hp": m.max_hp,
                    "intent": m.intent,
                }
                for m in state.monsters
            ],
        },
    }

    with make_server(port=0) as server:
        threading.Thread(target=server.serve_forever, daemon=True).start()
        response = _send(server, payload)
        server.shutdown()

    assert "error" not in response
    assert sorted(response["legal_actions"]) == sorted(legal_actions(state))
    assert set(response["values"].keys()) == set(response["legal_actions"])
    assert -1.0 <= response["state_value"] <= 1.0


def test_analyze_mid_fight_snapshot_reconstructs_block_statuses_and_piles():
    payload = {
        "cmd": "analyze",
        "iterations": 10,
        "seed": 1,
        "state": {
            "player": {
                "hp": 62,
                "max_hp": 80,
                "energy": 3,
                "block": 5,
                "statuses": [["Vulnerable", 2], ["Strength", 3]],
            },
            "hand": ["Strike", "Defend", "Bash"],
            "draw_pile": ["Strike", "Defend"],
            "discard_pile": ["Strike"],
            "exhaust_pile": [],
            "turn": 3,
            "monsters": [
                {
                    "name": "Jaw Worm",
                    "hp": 30,
                    "max_hp": 44,
                    "block": 0,
                    "intent": "Thrash",
                    "last_move": "Bellow",
                    "move_streak": 1,
                    "statuses": [["Strength", 3]],
                }
            ],
        },
    }

    with make_server(port=0) as server:
        threading.Thread(target=server.serve_forever, daemon=True).start()
        response = _send(server, payload)
        server.shutdown()

    assert "error" not in response
    assert "PlayCard:Strike" in response["legal_actions"]
    assert "EndTurn" in response["legal_actions"]
    assert set(response["values"].keys()) == set(response["legal_actions"])
    assert -1.0 <= response["state_value"] <= 1.0
