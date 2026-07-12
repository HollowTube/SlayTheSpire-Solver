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

    def setup_fight(self, timeout: int = 90) -> None:
        bc.start_run(character="Ironclad", ascension=0)
        time.sleep(2.5)

        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            screen = _screen()
            if "COMBAT" in screen and "LOADING" not in screen:
                return
            if "LOADING" in screen or "TRANSITION" in screen:
                time.sleep(0.8)
                continue
            if screen == "EVENT":
                avail = bc._payload(bc.get_available_actions())
                actions = avail.get("actions", [])
                opts = [a for a in actions if a.get("action") == "event_option"]
                proceed = next(
                    (a for a in opts if "proceed" in a.get("label", "").lower()), None
                )
                if proceed is not None:
                    bc.make_event_choice(proceed["choice_index"])
                    time.sleep(1.5)
                elif opts:
                    _bad = ("card", "curse")
                    safe = next(
                        (
                            a
                            for a in opts
                            if "proceed" not in a.get("label", "").lower()
                            and not any(k in a.get("label", "").lower() for k in _bad)
                        ),
                        opts[1] if len(opts) > 1 else opts[0],
                    )
                    bc.make_event_choice(safe["choice_index"])
                    time.sleep(1.0)
                else:
                    time.sleep(0.5)
                continue
            if "REWARD" in screen:
                bc.reward_proceed()
                time.sleep(1.0)
                continue
            if "CARD_SELECTION" in screen or ("CARD" in screen and "SELECT" in screen):
                bc.card_skip()
                time.sleep(1.0)
                if "CARD" in _screen():
                    bc.card_confirm()
                    time.sleep(0.8)
                continue
            if "TREASURE" in screen:
                bc.treasure_proceed()
                time.sleep(1.0)
                continue
            if "REST" in screen:
                bc.rest_site_choice("rest")
                time.sleep(0.8)
                bc.rest_site_proceed()
                time.sleep(0.8)
                continue
            if "SHOP" in screen or "MERCHANT" in screen:
                bc.shop_proceed()
                time.sleep(0.8)
                continue
            if screen == "MAP":
                map_state = bc._payload(bc.get_map_state())
                nodes = map_state.get("nodes", [])
                target = next(
                    (n for n in nodes if n.get("available") and n.get("type") == "Monster"),
                    None,
                ) or next((n for n in nodes if n.get("available")), None)
                if target:
                    bc.navigate_map(target["row"], target["col"])
                    time.sleep(2.5)
                else:
                    _console("fight JAW_WORM")
                    time.sleep(2)
                continue
            if "GAME_OVER" in screen or "MAIN_MENU" in screen:
                bc.start_run(character="Ironclad", ascension=0)
                time.sleep(2.5)
                continue
            time.sleep(0.5)
        raise RuntimeError(f"Could not reach combat in {timeout}s (last screen: {_screen()})")

    def _ensure_in_combat(self) -> None:
        screen = _screen()
        if "COMBAT" not in screen or "LOADING" in screen:
            self.setup_fight()

    def set_hand(self, *card_ids: str) -> None:
        state = _combat()
        for card in state["hand"]:
            cid = _to_console_id(card["name"])
            _console(f"remove_card {cid} hand")
        time.sleep(0.2)
        for card_id in card_ids:
            _console(f"card {card_id} hand")

    def pin_enemy_hp(self, target: int, idx: int = 0) -> None:
        self._ensure_in_combat()
        state = _combat()
        enemies = state["enemies"]
        if idx >= len(enemies) or enemies[idx]["hp"] <= target:
            self.setup_fight()
            state = _combat()
            enemies = state["enemies"]
        cur = enemies[idx]["hp"]
        if cur > target:
            _console(f"damage {cur - target} {idx + 1}")

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


@pytest.fixture(scope="module")
def fix():
    """CombatFixture scoped to the test module — one run per file."""
    f = CombatFixture()
    f.setup_fight()
    return f
