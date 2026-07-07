"""TCP client for the MCPTest bridge mod (port 21337).

On WSL, the game runs on the Windows host — set STS2_BRIDGE_HOST to the
WSL gateway IP (e.g. `ip route | grep default | awk '{print $3}'`).
"""

from __future__ import annotations

import json
import os
import socket
import time
from typing import Any

_STABLE_SCREENS = {
    "COMBAT_PLAYER_TURN",
    "MAP",
    "MAIN_MENU",
    "CARD_REWARD",
    "COMBAT_REWARD",
    "REST_SITE",
    "SHOP_SCREEN",
    "EVENT",
    "CHEST",
}

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
        with socket.create_connection(
            (BRIDGE_HOST, BRIDGE_PORT), timeout=TIMEOUT
        ) as sock:
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
        return {
            "error": "Bridge not running. Is the game running with MCPTest mod loaded?"
        }
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
    """Composite: assembles screen + run + combat state in multiple calls."""
    actions_data = get_available_actions()
    screen = (
        actions_data.get("screen", "UNKNOWN")
        if not actions_data.get("error")
        else "UNKNOWN"
    )

    if screen == "UNKNOWN":
        screen_data = get_screen()
        screen = screen_data.get("screen", "UNKNOWN")

    state: dict[str, Any] = {"screen": screen}

    in_run = screen not in ("MAIN_MENU", "CHARACTER_SELECT", "UNKNOWN")
    if in_run:
        player_data = get_player_state()
        if not player_data.get("error"):
            state["player"] = player_data
        else:
            run_data = get_run_state()
            if not run_data.get("error"):
                state["run"] = {
                    k: run_data[k]
                    for k in (
                        "act",
                        "floor",
                        "hp",
                        "max_hp",
                        "gold",
                        "character",
                        "seed",
                    )
                    if k in run_data
                }

    if "COMBAT" in screen and "LOADING" not in screen:
        combat_data = get_combat_state()
        if not combat_data.get("error"):
            state["combat"] = combat_data

    if not actions_data.get("error"):
        state["available_actions"] = actions_data
    else:
        state["available_actions"] = {"actions": [], "error": "Could not fetch actions"}

    return state


def execute_console_command(command: str) -> dict:
    return send("console", {"command": command})


def navigate_map(row: int, col: int) -> dict:
    return execute_action("map_travel", row=row, col=col)


def get_screen() -> dict:
    return send("get_screen")


def play_card(card_index: int, target_index: int = -1) -> dict:
    return send("play_card", {"card_index": card_index, "target_index": target_index})


def end_turn() -> dict:
    return send("end_turn")


def execute_action(action: str, **params: Any) -> dict:
    payload: dict[str, Any] = {"action": action.strip().lower()}
    payload.update(params)
    return send("execute_action", payload)


def act_and_wait(action: str, settle_timeout: float = 5.0, **params: Any) -> dict:
    """Execute action, wait for screen to stabilize, return new full state."""
    normalized = action.strip().lower()
    if normalized == "play_card":
        action_result = play_card(
            card_index=params.get("card_index", 0),
            target_index=params.get("target_index", -1),
        )
    elif normalized == "end_turn":
        action_result = end_turn()
    else:
        action_result = execute_action(normalized, **params)

    action_error = (
        action_result.get("error") if isinstance(action_result, dict) else None
    )

    deadline = time.monotonic() + settle_timeout
    prev_screen = ""
    stable_count = 0
    while time.monotonic() < deadline:
        raw = get_screen()
        current = raw.get("screen", "UNKNOWN") if isinstance(raw, dict) else "UNKNOWN"
        if current == prev_screen and current in _STABLE_SCREENS:
            stable_count += 1
            if stable_count >= 1:
                break
        else:
            stable_count = 0
        prev_screen = current
        time.sleep(0.3)

    new_state = get_full_state()
    output: dict[str, Any] = {}
    if action_error:
        output["action_error"] = action_error
    output["action_result"] = action_result
    output.update(new_state)
    return output


def start_run(
    character: str = "Ironclad",
    ascension: int = 0,
    seed: str | None = None,
    fight: str | None = None,
    godmode: bool = False,
) -> dict:
    p: dict[str, Any] = {"character": character, "ascension": ascension}
    if seed:
        p["seed"] = seed
    if fight:
        p["fight"] = fight
    if godmode:
        p["godmode"] = True
    return send("start_run", p)
