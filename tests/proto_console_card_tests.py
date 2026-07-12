"""PROTOTYPE — answers: "Can we build a usable Python wrapper around the bridge
client to set up deterministic in-combat states and verify card effects?"

Run with (requires STS2 running with bridge mod):
    STS2_BRIDGE_HOST=172.26.176.1 python tests/proto_console_card_tests.py

The wrapper assumes you're already in combat (COMBAT_PLAYER_TURN screen). To
get there: start a run, skip Neow, navigate to any monster node on the map.
The fixture handles everything from there.

Delete or absorb when the question is answered.
"""

import re
import sys
import time

from sts_sim import bridge_client as bc


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _to_console_id(camel_name: str) -> str:
    """CamelCase card name  →  SCREAMING_SNAKE_CASE console ID.

    Bridge returns hand card names in CamelCase (e.g. 'DefendIronclad'),
    but the dev console expects SCREAMING_SNAKE_CASE ('DEFEND_IRONCLAD').
    """
    # Insert underscore before each uppercase letter that follows a lowercase one
    snake = re.sub(r"(?<=[a-z])(?=[A-Z])", "_", camel_name)
    return snake.upper()


def _console(cmd: str) -> dict:
    r = bc.execute_console_command(cmd)
    time.sleep(0.25)
    return r


def _combat() -> dict:
    """Return a simple view of the current combat state."""
    raw = bc._payload(bc.get_combat_state())
    return {
        "hand": raw.get("players", [{}])[0].get("hand", []),
        "enemies": raw.get("enemies", []),
        "player": raw.get("players", [{}])[0],
    }


def _screen() -> str:
    return bc._payload(bc.get_screen()).get("screen", "UNKNOWN")


# ---------------------------------------------------------------------------
# CombatFixture  — the main wrapper
# ---------------------------------------------------------------------------


class CombatFixture:
    """Set up deterministic in-combat states and verify card effects.

    Assumes a run is already in COMBAT_PLAYER_TURN. Call setup_fight() to
    start a fresh run and navigate to the first monster room automatically.
    """

    # ── Lifecycle ────────────────────────────────────────────────────────────

    def setup_fight(self, timeout: int = 30) -> None:
        """Start a clean run (no godmode) and navigate to the first monster node.

        Handles Neow: picks "Proceed" (no blessing) automatically.
        """
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
                # Neow — always pick "Proceed" (no blessing) to avoid mandatory
                # card-selection screens that the bridge can't confirm through
                avail = bc._payload(bc.get_available_actions())
                actions = avail.get("actions", [])
                proceed_idx = next(
                    (
                        a["choice_index"]
                        for a in actions
                        if a.get("action") == "event_option"
                        and "proceed" in a.get("label", "").lower()
                    ),
                    None,
                )
                if proceed_idx is not None:
                    bc.make_event_choice(proceed_idx)
                    time.sleep(1.5)
                else:
                    # Proceed option not yet visible — try event_proceed directly
                    bc.execute_action("event_proceed")
                    time.sleep(1.5)
                continue
            if "REWARD" in screen:
                bc.reward_proceed()
                time.sleep(1.0)
                continue
            if "CARD_SELECTION" in screen or ("CARD" in screen and "SELECT" in screen):
                bc.card_skip()
                time.sleep(1.0)
                # If still on card selection, try confirm
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
                # Navigate to the first available monster node
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
                # Run ended — start a new one
                bc.start_run(character="Ironclad", ascension=0)
                time.sleep(2.5)
                continue
            time.sleep(0.5)
        raise RuntimeError(f"Could not reach combat in {timeout}s (last screen: {_screen()})")

    def _ensure_in_combat(self) -> None:
        """Start a fresh run if we're not currently in combat."""
        screen = _screen()
        if "COMBAT" not in screen or "LOADING" in screen:
            self.setup_fight()

    def assert_in_combat(self) -> None:
        screen = _screen()
        if "COMBAT" not in screen or "LOADING" in screen:
            raise RuntimeError(f"Not in combat (screen={screen}). Call setup_fight() first.")

    # ── State control ────────────────────────────────────────────────────────

    def set_hand(self, *card_ids: str) -> None:
        """Clear the current hand and add exactly the given cards.

        Args:
            card_ids: SCREAMING_SNAKE_CASE console IDs, e.g. 'STRIKE_IRONCLAD'.
        """
        state = _combat()
        for card in state["hand"]:
            cid = _to_console_id(card["name"])
            _console(f"remove_card {cid} hand")
        # Brief pause for state to settle after removals
        time.sleep(0.2)
        for card_id in card_ids:
            _console(f"card {card_id} hand")

    def pin_enemy_hp(self, target: int, idx: int = 0) -> None:
        """Damage enemy[idx] down to exactly target HP.

        If enemy HP is at or below target (or no enemy exists), starts a fresh
        run via setup_fight() to get a new enemy, then pins.

        Design: keeps target well above the damage being tested so the enemy
        never dies during the test — avoiding the REWARD screen entirely.
        """
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

    def set_energy(self, amount: int) -> None:
        """Set player energy to the given amount."""
        bc.manipulate_state({"energy": amount})
        time.sleep(0.2)

    # ── Queries ──────────────────────────────────────────────────────────────

    def enemy_hp(self, idx: int = 0) -> int:
        state = _combat()
        enemies = state["enemies"]
        return enemies[idx]["hp"] if idx < len(enemies) else -1

    def player_block(self) -> int:
        return _combat()["player"].get("block", 0)

    def player_energy(self) -> int:
        return _combat()["player"].get("energy", -1)

    def enemy_powers(self, idx: int = 0) -> list[dict]:
        state = _combat()
        enemies = state["enemies"]
        return enemies[idx].get("powers", []) if idx < len(enemies) else []

    def hand_names(self) -> list[str]:
        return [c["name"] for c in _combat()["hand"]]

    def has_power(self, power_name: str, target: str = "enemy", idx: int = 0) -> int:
        """Return the stack count of power_name on target ('enemy' or 'player').

        Returns 0 if the power is absent.
        """
        state = _combat()
        if target == "enemy":
            powers = state["enemies"][idx].get("powers", []) if idx < len(state["enemies"]) else []
        else:
            powers = state["player"].get("powers", [])
        for p in powers:
            if power_name.lower() in p.get("name", "").lower():
                return p.get("amount", 0)
        return 0

    # ── Actions ──────────────────────────────────────────────────────────────

    def play(self, keyword: str) -> bool:
        """Play the first available card whose name contains keyword (case-insensitive)."""
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

    # ── Assertions (raise AssertionError on failure) ─────────────────────────

    def assert_damage(self, expected: int, hp_before: int, idx: int = 0) -> None:
        actual = hp_before - self.enemy_hp(idx)
        if actual != expected:
            raise AssertionError(f"Expected {expected} damage, dealt {actual}")

    def assert_block(self, expected: int) -> None:
        actual = self.player_block()
        if actual != expected:
            raise AssertionError(f"Expected {expected} block, got {actual}")

    def assert_power(
        self, power_name: str, expected_stacks: int, target: str = "enemy", idx: int = 0
    ) -> None:
        actual = self.has_power(power_name, target, idx)
        if actual != expected_stacks:
            raise AssertionError(
                f"Expected {power_name} x{expected_stacks} on {target}[{idx}], got {actual}"
            )


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

PASS, FAIL = "PASS", "FAIL"


def run_test(name: str, fn) -> bool:
    print(f"\n[TEST] {name}")
    try:
        fn()
        print(f"  {PASS}")
        return True
    except AssertionError as e:
        print(f"  {FAIL}  {e}")
        return False
    except Exception as e:
        print(f"  {FAIL} (exception)  {type(e).__name__}: {e}")
        return False


def test_strike_deals_6_damage(fix: CombatFixture) -> None:
    fix.set_hand("STRIKE_IRONCLAD")
    fix.pin_enemy_hp(40)  # enemy at 40; Strike deals 6 → 34 HP, no death
    hp_before = fix.enemy_hp()
    ok = fix.play("strike")
    assert ok, "Could not find Strike in available actions"
    fix.assert_damage(6, hp_before)


def test_defend_gives_5_block(fix: CombatFixture) -> None:
    fix._ensure_in_combat()
    fix.set_hand("DEFEND_IRONCLAD")
    block_before = fix.player_block()
    ok = fix.play("defend")
    assert ok, "Could not find Defend in available actions"
    gained = fix.player_block() - block_before
    assert gained == 5, f"Expected +5 block, got +{gained}"


def test_bash_deals_8_and_applies_2_vulnerable(fix: CombatFixture) -> None:
    fix.set_hand("BASH")
    fix.pin_enemy_hp(40)  # enemy at 40; Bash deals 8 → 32 HP, no death
    hp_before = fix.enemy_hp()
    ok = fix.play("bash")
    assert ok, "Could not find Bash in available actions"
    fix.assert_damage(8, hp_before)
    fix.assert_power("VulnerablePower", 2, target="enemy")


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------


def main() -> None:
    print("Checking bridge...")
    if not bc.is_connected():
        print("  Bridge not reachable. Start STS2 with the bridge mod first.")
        sys.exit(1)
    print("  Bridge OK")

    fix = CombatFixture()

    screen = _screen()
    if "COMBAT" in screen and "LOADING" not in screen:
        state = _combat()
        godmode_powers = [p for p in state["player"].get("powers", []) if p.get("amount", 0) > 999]
        if godmode_powers:
            print(f"\n  Godmode powers detected — starting fresh run for clean numbers...")
            fix.setup_fight()
        else:
            print(f"\n  Already in combat. Screen: {screen}")
    else:
        print(f"\nNot in combat (screen={screen}). Starting a fresh run...")
        fix.setup_fight()

    print(f"  Ready. Screen: {_screen()}")

    results = []
    tests = [
        ("Strike deals 6 damage", lambda: test_strike_deals_6_damage(fix)),
        ("Defend gives 5 block", lambda: test_defend_gives_5_block(fix)),
        ("Bash deals 8 damage + 2 Vulnerable", lambda: test_bash_deals_8_and_applies_2_vulnerable(fix)),
    ]

    for name, fn in tests:
        results.append((name, run_test(name, fn)))

    print("\n── Results ──────────────────────────────────────────────")
    for name, ok in results:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}")

    passed = sum(1 for _, ok in results if ok)
    print(f"\n  {passed}/{len(results)} passed")
    sys.exit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()
