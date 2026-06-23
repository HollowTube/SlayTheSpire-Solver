"""Behavioural tests for HOL-43: Tracker Ruby Raider (Status::Frail x2) and
Mawler (once-per-combat Roar constraint)."""

from sts_sim import CombatState, Monster, apply


# ── Tracker Ruby Raider ──────────────────────────────────────────────────────


def _tracker(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=21, name="Tracker Ruby Raider")],
        seed=seed,
        hand=hand or [],
    )


def test_tracker_opens_with_track():
    state = _tracker()
    assert state.monsters[0].intent == "Track"


def test_track_applies_2_frail_no_damage():
    state = _tracker()
    after = apply(state, "EndTurn")
    assert state.player_hp == after.player_hp
    assert "Frail" in after.player_statuses


def test_track_to_hounds_transition():
    state = _tracker()
    after = apply(state, "EndTurn")
    assert after.monsters[0].intent == "Hounds"


def test_hounds_deals_8_damage():
    state = _tracker()
    after_track = apply(state, "EndTurn")
    after_hounds = apply(after_track, "EndTurn")
    assert after_track.player_hp - after_hounds.player_hp == 8


def test_tracker_repeats_hounds_forever():
    state = _tracker()
    a1 = apply(state, "EndTurn")  # Track
    a2 = apply(a1, "EndTurn")  # Hounds
    a3 = apply(a2, "EndTurn")  # Hounds
    a4 = apply(a3, "EndTurn")  # Hounds
    assert a2.monsters[0].intent == "Hounds"
    assert a3.monsters[0].intent == "Hounds"
    assert a4.monsters[0].intent == "Hounds"


def test_track_frail_reduces_block():
    """After Track's 2 Frail, a Defend (5 block) should gain only 3 block."""
    state = _tracker(hand=["Defend"])
    after_track = apply(state, "EndTurn")  # Track applies Frail(2)
    after_defend = apply(after_track, "PlayCard:Defend")
    assert after_defend.player_block == 3


# ── Mawler ───────────────────────────────────────────────────────────────────


def _mawler(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=72, name="Mawler")],
        seed=seed,
        hand=hand or [],
    )


def test_mawler_opens_with_claw():
    state = _mawler()
    assert state.monsters[0].intent == "Claw"


def test_claw_deals_8_damage():
    state = _mawler()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 8


def test_mawler_next_intent_after_claw():
    """After opening Claw, the next intent is one of three valid moves."""
    state = _mawler()
    after = apply(state, "EndTurn")
    assert after.monsters[0].intent in ("Rip and Tear", "Roar", "Claw")


def test_mawler_roar_used_only_once():
    """Roar should never appear as intent more than once across many turns."""
    state = _mawler(seed=1)
    roars_seen = 0
    s = state
    for _ in range(20):
        s = apply(s, "EndTurn")
        if s.monsters[0].intent == "Roar":
            roars_seen += 1
    assert roars_seen <= 1


def test_mawler_roar_applies_vulnerable():
    """After Roar, the player should have Vulnerable in their status list."""
    state = _mawler(seed=1)
    s = apply(state, "EndTurn")  # Claw
    while s.monsters[0].intent != "Roar":
        s = apply(s, "EndTurn")
    after_roar = apply(s, "EndTurn")
    assert "Vulnerable" in after_roar.player_statuses


def test_mawler_rip_and_tear_may_appear():
    """Rip and Tear should be a possible intent (equal-weight random)."""
    state = _mawler(seed=2)
    s = apply(state, "EndTurn")  # Claw
    seen_rip = s.monsters[0].intent == "Rip and Tear"
    for _ in range(15):
        s = apply(s, "EndTurn")
        if s.monsters[0].intent == "Rip and Tear":
            seen_rip = True
            break
    assert seen_rip


def test_mawler_never_repeats_same_intent():
    """Claw and Rip and Tear should never repeat consecutively (max_streak 1).
    Check by looking at intent (the telegraphed next move), not last_move
    (which is not exposed to Python)."""
    state = _mawler(seed=3)
    s = apply(state, "EndTurn")  # Claw (opening)
    prev_intent = None
    for _ in range(30):
        s = apply(s, "EndTurn")
        if prev_intent == s.monsters[0].intent:
            if s.monsters[0].intent in ("Claw", "Rip and Tear"):
                assert False, f"{s.monsters[0].intent} repeated consecutively"
        prev_intent = s.monsters[0].intent
