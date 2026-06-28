"""Behavioural tests for HOL-47: Flyconid (elite) — weighted-random elite
monster with Vulnerable Spores, Frail Spores, and Smash."""

from sts_sim import CombatState, EndTurnAction, Monster, apply


def _flyconid(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=49, name="Flyconid")],
        seed=seed,
        hand=hand or [],
    )


def test_opens_with_frail_spores():
    state = _flyconid()
    assert state.monsters[0].intent == "Frail Spores"


def test_frail_spores_deals_8_damage_and_applies_frail():
    state = _flyconid()
    after = apply(state, EndTurnAction())
    assert state.player_hp - after.player_hp == 8, "expected 8 damage from Frail Spores"
    assert "Frail" in after.player_statuses, "expected Frail to be applied"


def test_smash_deals_11_damage():
    state = _flyconid()
    for _ in range(10):
        current = apply(state, EndTurnAction())
        if current.monsters[0].intent == "Smash":
            after = apply(current, EndTurnAction())
            expected = 16 if "Vulnerable" in current.player_statuses else 11
            assert current.player_hp - after.player_hp == expected, (
                f"expected {expected} damage from Smash"
            )
            return
        state = current
    assert False, "Smash never rolled in 10 attempts"


def test_vulnerable_spores_applies_vulnerable():
    state = _flyconid()
    for _ in range(10):
        current = apply(state, EndTurnAction())
        if current.monsters[0].intent == "Vulnerable Spores":
            after = apply(current, EndTurnAction())
            assert after.player_statuses.count("Vulnerable") >= 1, (
                "expected Vulnerable to be applied"
            )
            assert current.player_hp == after.player_hp, (
                "Vulnerable Spores deals no damage"
            )
            return
        state = current
    assert False, "Vulnerable Spores never rolled in 10 attempts"


def test_no_move_repeats_consecutively():
    state = _flyconid()
    moves = []
    for _ in range(15):
        after = apply(state, EndTurnAction())
        moves.append(after.monsters[0].last_move)
        state = after
    for i in range(1, len(moves)):
        assert moves[i] != moves[i - 1], f"{moves[i]} repeated at turns {i - 1}→{i}"
