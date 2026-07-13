"""Shared fixtures for live-game integration tests.

These tests require STS2 running with the bridge mod:
    pytest integtests/ -q

The Windows host IP is auto-detected via the WSL default gateway.
Override with STS2_BRIDGE_HOST if needed.
"""

import re
import time

import pytest

from sts_sim import bridge_client as bc


def pytest_configure(config):
    config.addinivalue_line("markers", "live: requires STS2 running with bridge mod")


def pytest_collection_modifyitems(config, items):
    if not bc.is_connected():
        skip = pytest.mark.skip(reason="bridge not reachable (STS2 not running)")
        for item in items:
            item.add_marker(skip)


# ---------------------------------------------------------------------------
# Low-level helpers
# ---------------------------------------------------------------------------


def _to_console_id(camel_name: str) -> str:
    snake = re.sub(r"(?<=[a-z])(?=[A-Z])", "_", camel_name)
    return snake.upper()


def _console(cmd: str) -> dict:
    r = bc.execute_console_command(cmd)
    time.sleep(0.25)
    return r


def _combat() -> dict:
    raw = bc._payload(bc.get_combat_state())
    return {
        "hand": raw.get("players", [{}])[0].get("hand", []),
        "enemies": raw.get("enemies", []),
        "player": raw.get("players", [{}])[0],
    }


def _screen() -> str:
    return bc._payload(bc.get_screen()).get("screen", "UNKNOWN")


# ---------------------------------------------------------------------------
# CombatFixture
# ---------------------------------------------------------------------------


class CombatFixture:
    """Set up deterministic in-combat states and verify card effects.

    Keeps enemy HP well above the damage being tested so the enemy never dies
    mid-test — avoiding the REWARD screen entirely.
    """

    # Monster used as a test dummy — high enough HP that 8 damage (Bash) never kills it.
    FIGHT_ID = "NIBBITS_WEAK"

    def setup_fight(self, timeout: int = 15) -> None:
        """Start a fresh combat via the dev console.

        Recipe: win any active fight → room MAP → fight <encounter>.
        Works from COMBAT, REWARD, MAP, EVENT, or CARD_SELECTION.
        """
        # Start a run if there is none yet
        screen = _screen()
        if "MAIN_MENU" in screen or "GAME_OVER" in screen:
            bc.start_run(character="Ironclad", ascension=0)
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                s = _screen()
                if not any(k in s for k in ("MAIN_MENU", "GAME_OVER", "LOADING")):
                    break
                time.sleep(0.5)

        # Death overlay: player HP is 0; heal so fight initialises cleanly
        if _screen() == "OVERLAY":
            _console("heal 999")
            time.sleep(0.5)

        # Win any active combat to reach REWARD
        if "COMBAT" in _screen() and "LOADING" not in _screen():
            _console("win")
            time.sleep(1.5)

        # Navigate to MAP (works from REWARD, OVERLAY, EVENT, CARD_SELECTION, etc.)
        if _screen() != "MAP":
            _console("room MAP")
            time.sleep(1.5)

        _console(f"fight {self.FIGHT_ID}")
        time.sleep(1.0)  # let the game start transitioning before polling

        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            s = _screen()
            if "COMBAT" in s and "LOADING" not in s:
                return
            time.sleep(0.4)
        raise RuntimeError(f"fight {self.FIGHT_ID} did not reach combat in {timeout}s (screen: {_screen()})")

    def set_hand(self, *card_ids: str) -> None:
        state = _combat()
        for card in state["hand"]:
            cid = _to_console_id(card["name"])
            _console(f"remove_card {cid} hand")
        time.sleep(0.2)
        for card_id in card_ids:
            _console(f"card {card_id} hand")

    def enemy_hp(self, idx: int = 0) -> int:
        state = _combat()
        enemies = state["enemies"]
        return enemies[idx]["hp"] if idx < len(enemies) else -1

    def player_block(self) -> int:
        return _combat()["player"].get("block", 0)

    def has_power(self, power_name: str, target: str = "enemy", idx: int = 0) -> int:
        state = _combat()
        if target == "enemy":
            powers = (
                state["enemies"][idx].get("powers", [])
                if idx < len(state["enemies"])
                else []
            )
        else:
            powers = state["player"].get("powers", [])
        for p in powers:
            if power_name.lower() in p.get("name", "").lower():
                return p.get("amount", 0)
        return 0

    def play(self, keyword: str) -> bool:
        avail = bc._payload(bc.get_available_actions())
        actions = avail.get("actions", [])
        act = next(
            (a for a in actions if keyword.lower() in a.get("card_name", "").lower()),
            None,
        )
        if act is None:
            return False
        bc.play_card(act["card_index"], act.get("target_index", -1))
        time.sleep(0.5)
        return True


@pytest.fixture
def fix():
    return CombatFixture()
