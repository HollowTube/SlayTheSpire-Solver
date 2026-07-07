"""TCP client for the MCPTest bridge mod (port 21337).

On WSL, the game runs on the Windows host — set STS2_BRIDGE_HOST to the
WSL gateway IP (e.g. `ip route | grep default | awk '{print $3}'`).
"""

from __future__ import annotations

import json
import os
import socket
from typing import Any

BRIDGE_HOST: str = os.environ.get("STS2_BRIDGE_HOST", "127.0.0.1")
BRIDGE_PORT: int = 21337
TIMEOUT: float = 12.0
_MAX_BYTES: int = 10 * 1024 * 1024


def send(method: str, params: dict[str, Any] | None = None) -> dict:
    """Send one JSON-RPC request and return the parsed response dict."""
    req = {"method": method, "id": 1}
    if params:
        req["params"] = params

    try:
        with socket.create_connection((BRIDGE_HOST, BRIDGE_PORT), timeout=TIMEOUT) as sock:
            sock.settimeout(TIMEOUT)
            sock.sendall((json.dumps(req) + "\n").encode())
            data = b""
            while True:
                chunk = sock.recv(4096)
                if not chunk:
                    break
                data += chunk
                if len(data) > _MAX_BYTES:
                    return {"error": "response too large"}
                if b"\n" in data or b"\r" in data:
                    break
    except ConnectionRefusedError:
        return {"error": "Bridge not running. Is the game running with MCPTest mod loaded?"}
    except socket.timeout:
        return {"error": "Bridge timed out. Game may be loading or unresponsive."}
    except OSError as e:
        return {"error": f"Bridge unreachable: {e}"}

    try:
        parsed = json.loads(data.decode("utf-8-sig").strip())
    except (json.JSONDecodeError, UnicodeDecodeError) as e:
        return {"error": f"Bad response from bridge: {e}"}

    result = parsed.get("result", parsed)
    return result if isinstance(result, dict) else parsed


def ping() -> dict:
    return send("ping")

def get_run_state() -> dict:
    return send("get_run_state")

def get_combat_state() -> dict:
    return send("get_combat_state")

def get_player_state() -> dict:
    return send("get_player_state")

def get_map_state() -> dict:
    return send("get_map_state")

def get_card_piles() -> dict:
    return send("get_card_piles")

def get_available_actions() -> dict:
    return send("get_available_actions")

def get_game_log() -> dict:
    return send("get_game_log")

def get_full_state() -> dict:
    return send("get_full_state")

def execute_console_command(command: str) -> dict:
    return send("execute_console_command", {"command": command})

def navigate_map(row: int, col: int) -> dict:
    return send("navigate_map", {"row": row, "col": col})

def act_and_wait(action: str, **params: Any) -> dict:
    return send("act_and_wait", {"action": action, **params})

def start_run(character: str = "Ironclad", ascension: int = 0,
              seed: str | None = None, fight: str | None = None,
              godmode: bool = False) -> dict:
    p: dict[str, Any] = {"character": character, "ascension": ascension}
    if seed:
        p["seed"] = seed
    if fight:
        p["fight"] = fight
    if godmode:
        p["godmode"] = True
    return send("start_run", p)
