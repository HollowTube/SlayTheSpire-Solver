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
    CardPiles,
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


def _wait_settled(
    exclude: tuple[str, ...] = ("LOADING",),
    timeout: float = 10.0,
    delay: float = 0.4,
) -> str:
    """Poll until the screen is out of `exclude` and stable for two consecutive reads.

    A screen-name change alone is not proof the underlying scene has finished
    loading/tearing down — e.g. after start_run() the screen can already report
    a non-MAIN_MENU value while the async intro Event Room is still resolving.
    Issuing a console command (win/room/fight) into that window is what throws
    a NullReferenceException in NCombatUi.Enable() and permanently wedges the
    game's CombatManager for the rest of the process — see README. Requiring
    the same (screen, action-count) reading twice in a row is a much stronger
    settle signal than a screen-name check or a fixed sleep.

    `exclude` is matched case-insensitively: STS2 mixes SCREAMING_SNAKE_CASE
    top-level screen names (MAIN_MENU) with PascalCase room/scene type names
    for sub-states (MENU_NLogoAnimation, MENU_NCombatRoom).
    """
    deadline = time.monotonic() + timeout
    exclude_upper = tuple(k.upper() for k in exclude)
    prev_key: tuple[str, int] | None = None
    while time.monotonic() < deadline:
        s = _screen()
        if any(k in s.upper() for k in exclude_upper):
            prev_key = None
            time.sleep(delay)
            continue
        actions = bc.get_available_actions()
        action_list = actions.get("actions", []) if isinstance(actions, dict) else []
        key = (s, len(action_list))
        if key == prev_key:
            return s
        prev_key = key
        time.sleep(delay)
    return _screen()


def _stable_piles(
    retries: int = 30, delay: float = 0.35, required_matches: int = 4
) -> CardPiles:
    """Poll get_card_piles() until `required_matches` consecutive reads agree.

    Console commands and the fresh-combat starting-hand draw are async; reading
    piles immediately after a screen transition can catch mid-draw state and
    miss cards that arrive later (the CINDER stray-Defend bug, where
    set_hand()'s old single-snapshot read left starting-deck cards mixed into
    what should have been a clean test hand, producing a false
    RNG-divergence failure). A *single* repeated match isn't enough evidence:
    live testing showed STS2's opening-hand draw arriving in bursts spread
    over 1-2s with quiet gaps close to a second, so two reads 0.25s apart can
    agree by coincidence mid-draw. Requiring several consecutive agreements
    spaced further apart is a stronger settle signal, matching the existing
    "reads agree" idiom already used by `integtests/cards/conftest.py`'s
    `_stable_combat_state()` but tuned for this longer, burstier race.
    """

    def _key(piles: CardPiles) -> tuple:
        return tuple(
            tuple((c.name, c.upgraded) for c in pile.cards)
            for pile in (
                piles.hand,
                piles.draw_pile,
                piles.discard_pile,
                piles.exhaust_pile,
            )
        )

    piles = parse_card_piles(bc.get_card_piles())
    prev_key = _key(piles)
    streak = 1
    for _ in range(retries):
        if streak >= required_matches:
            return piles
        time.sleep(delay)
        piles = parse_card_piles(bc.get_card_piles())
        key = _key(piles)
        if key == prev_key:
            streak += 1
        else:
            prev_key = key
            streak = 1
    return piles


class CombatWedgedError(RuntimeError):
    """The game's CombatManager is permanently stuck and needs a full relaunch.

    STS2 can throw a NullReferenceException in NCombatUi.Enable() if a
    `fight`/`win`/`room` console command races an async screen transition
    (see README). Once that happens the UI is stuck on MENU_NCombatRoom with
    no functioning `dismiss` and `fight` keeps failing — there is no
    in-session recovery.
    """


# Symptom the report documented for the unrecoverable NCombatUi wedge: `fight`
# leaves the game stuck on this pre-combat overlay instead of entering combat.
# Matched case-insensitively — see _wait_settled's docstring on STS2 mixing
# SCREAMING_SNAKE_CASE and PascalCase for these room/scene sub-state names.
_WEDGE_SCREEN = "MENU_NCombatRoom".upper()


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
        Works from any game state. Uses in_combat field (not screen name) for detection
        because STS2's MENU_NCombatRoom screen (room entry overlay) is not actual combat.

        Raises CombatWedgedError instead of the generic timeout RuntimeError if
        `fight` leaves the game on the known-unrecoverable MENU_NCombatRoom
        wedge, so the failure is unambiguous about needing a full relaunch.
        """
        # Start a run if there is none yet. Settle first (not just a single
        # _screen() read) — a fresh game process can transiently report a
        # boot splash/attract-loop screen (e.g. MENU_NLogoAnimation) before
        # settling into MAIN_MENU, which a single read can mistake for "we're
        # mid-run, skip start_run()".
        screen = _wait_settled(exclude=("LOADING", "LOGO", "ANIMATION"), timeout=20.0)
        if "MAIN_MENU" in screen or "GAME_OVER" in screen:
            bc.start_run(character="Ironclad", ascension=0)
            _wait_settled(exclude=("MAIN_MENU", "GAME_OVER", "LOADING"), timeout=20.0)

        # Death overlay: player HP is 0; heal so fight initialises cleanly
        if _screen() == "OVERLAY":
            _console("heal 999")
            _wait_settled(timeout=8.0)

        # Win any active combat. Check in_combat rather than screen name —
        # MENU_NCombatRoom contains the word "Combat" but is not actual combat.
        if bc.get_combat_state().get("in_combat"):
            _console("win")
            _wait_settled(timeout=10.0)

        if _screen() != "MAP":
            _console("room MAP")
            _wait_settled(timeout=10.0)

        _console(f"fight {self.FIGHT_ID}")

        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if bc.get_combat_state().get("in_combat"):
                return
            time.sleep(0.4)

        stuck_screen = _screen()
        if _WEDGE_SCREEN in stuck_screen.upper():
            raise CombatWedgedError(
                f"fight {self.FIGHT_ID} left the game stuck on {_WEDGE_SCREEN} "
                "instead of entering combat — this is the known NCombatUi.Enable() "
                "wedge (see README's live-test section). It cannot be recovered "
                "in-session; fully relaunch the game before retrying."
            )
        raise RuntimeError(
            f"fight {self.FIGHT_ID} did not reach combat in {timeout}s "
            f"(screen: {stuck_screen}, in_combat: {bc.get_combat_state().get('in_combat')})"
        )

    def set_hand(self, *cards: CardName | str) -> None:
        piles = _stable_piles()
        for hcard in piles.hand.cards:
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
        piles = _stable_piles()
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

    def set_discard_pile(self, *cards: CardName | str) -> None:
        """Replace the discard pile with the given cards.

        Drains the existing discard into hand (via `draw_discard 99` if
        available, otherwise one-by-one removal), then adds the desired cards
        via `card X discard`.
        """
        piles = _stable_piles()
        discard_before = list(piles.discard_pile.cards)
        for c in discard_before:
            cid = _to_console_id(c.name)
            _console(f"remove_card {cid} discard")
        time.sleep(0.2)
        for card in cards:
            console_id = CARD_STS2_ID.get(card, card)
            _console(f"card {console_id} discard")

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
