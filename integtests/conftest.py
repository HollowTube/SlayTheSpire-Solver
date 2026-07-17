"""Shared fixtures for live-game integration tests.

These tests require STS2 running with the bridge mod:
    pytest integtests/ -q

The Windows host IP is auto-detected via the WSL default gateway.
Override with STS2_BRIDGE_HOST if needed.
"""

import re
import time

import pytest

from sts_sim.bridge import client as bc
from sts_sim.bridge import STATUS_MAP
from sts_sim.bridge.types import (
    AvailableActions,
    CombatSnapshot,
    parse_available_actions,
    parse_card_piles,
    parse_combat_snapshot,
)
from sts_sim.sim.names import CARD_STS2_ID, CardName


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


def _combat() -> CombatSnapshot:
    return parse_combat_snapshot(bc.get_combat_state())


def _screen() -> str:
    return bc.get_screen().get("screen", "UNKNOWN")


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

    def __init__(self, fight_id: str | None = None) -> None:
        if fight_id is not None:
            self.FIGHT_ID = fight_id

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
        raise RuntimeError(
            f"fight {self.FIGHT_ID} did not reach combat in {timeout}s (screen: {_screen()})"
        )

    def set_hand(self, *cards: CardName | str) -> None:
        snapshot = _combat()
        for hcard in snapshot.player.hand:
            cid = _to_console_id(hcard.name)
            _console(f"remove_card {cid} hand")
        time.sleep(0.2)
        for card in cards:
            console_id = CARD_STS2_ID.get(card, card)
            _console(f"card {console_id} hand")

    def set_draw_pile(self, *cards: CardName | str) -> None:
        """Replace the draw pile with the given cards.

        `remove_card X draw` is not supported by the console (only hand/deck),
        so we drain the draw pile into hand via `draw 99`, remove those cards
        from hand one-by-one, then add the desired cards to the now-empty pile.
        """
        piles = parse_card_piles(bc.get_card_piles())
        draw_before = list(piles.draw_pile.cards)
        _console("draw 99")
        time.sleep(0.5)
        for c in draw_before:
            cid = _to_console_id(c.name)
            _console(f"remove_card {cid} hand")
        time.sleep(0.2)
        for card in cards:
            console_id = CARD_STS2_ID.get(card, card)
            _console(f"card {console_id} draw")

    def upgrade_card(self, index: int = 0) -> None:
        """Upgrade the card at the given hand index via the dev console."""
        _console(f"upgrade {index}")

    def enemy_hp(self, idx: int = 0) -> int:
        snapshot = _combat()
        return snapshot.enemies[idx].hp if idx < len(snapshot.enemies) else -1

    def player_block(self) -> int:
        return _combat().player.block

    def has_power(self, sim_name: str, target: str = "enemy", idx: int = 0) -> int:
        """Return stack count of a status on the target, or 0 if absent.

        ``sim_name`` is the canonical sim status name (e.g. ``"Vulnerable"``),
        not the raw bridge class name.  Uses ``bridge.STATUS_MAP`` to translate
        whatever the bridge reports into that canonical form.
        """
        snapshot = _combat()
        if target == "enemy":
            powers = snapshot.enemies[idx].powers if idx < len(snapshot.enemies) else []
        else:
            powers = snapshot.player.powers
        for p in powers:
            if STATUS_MAP.get(p.name) == sim_name:
                return p.amount
        return 0

    def play(self, keyword: str) -> bool:
        avail: AvailableActions = parse_available_actions(bc.get_available_actions())
        act = next(
            (a for a in avail.actions if keyword.lower() in a.card_name.lower()),
            None,
        )
        if act is None:
            return False
        bc.play_card(act.card_index, act.target_index)
        time.sleep(0.5)
        return True


@pytest.fixture
def fix():
    return CombatFixture()
