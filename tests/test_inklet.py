"""Behavioural tests for the Inklet monster (Overgrowth normal enemy, attacks
in groups of three) and the Slippery status it starts with."""

from sts_sim import CombatState, Monster, apply


def _inklet(seed=42, hand=None, statuses=()):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=15, name="Inklet", statuses=list(statuses))],
        seed=seed,
        hand=hand or [],
    )


# ── Move pool ────────────────────────────────────────────────────────────────


def test_inklet_opens_with_jab():
    state = _inklet()
    assert state.monsters[0].intent == "Jab"


def test_inklet_jab_deals_3_damage():
    state = _inklet()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 3


def test_inklet_windup_punch_deals_2_damage_three_times():
    # With seed 42, Inklet's post-Jab roll lands on Windup Punch.
    state = _inklet()
    after_jab = apply(state, "EndTurn")
    assert after_jab.monsters[0].intent == "Windup Punch"

    after_windup = apply(after_jab, "EndTurn")
    assert after_jab.player_hp - after_windup.player_hp == 6


def test_inklet_piercing_gaze_deals_10_damage():
    # Stepping through seed 42's pattern: Jab, Windup Punch, Jab, Windup
    # Punch, Jab, Piercing Gaze.
    state = _inklet()
    after_jab_1 = apply(state, "EndTurn")
    after_windup_1 = apply(after_jab_1, "EndTurn")
    after_jab_2 = apply(after_windup_1, "EndTurn")
    after_windup_2 = apply(after_jab_2, "EndTurn")
    after_jab_3 = apply(after_windup_2, "EndTurn")
    assert after_jab_3.monsters[0].intent == "Piercing Gaze"

    after_gaze = apply(after_jab_3, "EndTurn")
    assert after_jab_3.player_hp - after_gaze.player_hp == 10


def test_inklet_returns_to_jab_after_windup_punch_or_piercing_gaze():
    state = _inklet()
    after_jab = apply(state, "EndTurn")
    assert after_jab.monsters[0].intent == "Windup Punch"

    after_windup = apply(after_jab, "EndTurn")
    assert after_windup.monsters[0].intent == "Jab"


# ── Slippery ─────────────────────────────────────────────────────────────────


def test_slippery_caps_hp_loss_to_1_then_is_consumed():
    state = _inklet(hand=["Strike", "Strike"], statuses=[("Slippery", 1)])
    assert "Slippery" in state.monsters[0].statuses

    after_first = apply(apply(state, "PlayCard:Strike"), "SelectTarget:Monster:0")
    # Strike deals 6, but Slippery caps the first hit's HP loss to 1.
    assert state.monsters[0].hp - after_first.monsters[0].hp == 1
    assert "Slippery" not in after_first.monsters[0].statuses

    after_second = apply(
        apply(after_first, "PlayCard:Strike"), "SelectTarget:Monster:0"
    )
    # Slippery has been consumed - the second Strike deals its full 6 damage.
    assert after_first.monsters[0].hp - after_second.monsters[0].hp == 6


def test_slippery_with_multiple_stacks_caps_each_hit_until_exhausted():
    state = _inklet(hand=["Strike", "Strike", "Strike"], statuses=[("Slippery", 2)])

    after_first = apply(apply(state, "PlayCard:Strike"), "SelectTarget:Monster:0")
    assert state.monsters[0].hp - after_first.monsters[0].hp == 1

    after_second = apply(
        apply(after_first, "PlayCard:Strike"), "SelectTarget:Monster:0"
    )
    assert after_first.monsters[0].hp - after_second.monsters[0].hp == 1
    assert "Slippery" not in after_second.monsters[0].statuses

    after_third = apply(
        apply(after_second, "PlayCard:Strike"), "SelectTarget:Monster:0"
    )
    assert after_second.monsters[0].hp - after_third.monsters[0].hp == 6
