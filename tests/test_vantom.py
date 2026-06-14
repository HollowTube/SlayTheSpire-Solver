"""Behavioural tests for the Vantom monster (Overgrowth boss, fixed 4-move
cycle with no RNG, and the "Wound" status card its Dismember move deals)."""

from sts_sim import CombatState, Monster, apply


def _vantom(seed=42, hand=None, statuses=(), player_energy=3):
    return CombatState(
        player_hp=80,
        player_energy=player_energy,
        monsters=[Monster(hp=173, name="Vantom", statuses=list(statuses))],
        seed=seed,
        hand=hand or [],
    )


# ── Move pool / fixed cycle ─────────────────────────────────────────────────


def test_vantom_opens_with_ink_blot():
    state = _vantom()
    assert state.monsters[0].intent == "Ink Blot"


def test_ink_blot_deals_7_damage():
    state = _vantom()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 7


def test_inky_lance_deals_6_damage_twice():
    state = _vantom()
    after_ink_blot = apply(state, "EndTurn")
    assert after_ink_blot.monsters[0].intent == "Inky Lance"

    after_inky_lance = apply(after_ink_blot, "EndTurn")
    assert after_ink_blot.player_hp - after_inky_lance.player_hp == 12


def test_dismember_deals_27_damage_and_sticks_three_wounds_in_discard():
    state = _vantom()
    after_ink_blot = apply(state, "EndTurn")
    after_inky_lance = apply(after_ink_blot, "EndTurn")
    assert after_inky_lance.monsters[0].intent == "Dismember"

    after_dismember = apply(after_inky_lance, "EndTurn")
    assert after_inky_lance.player_hp - after_dismember.player_hp == 27
    # Three "Wound" cards land in discard, then get reshuffled and drawn into
    # the fresh hand (same mechanism as Leaf Slime's Goop/StickyShot).
    assert after_dismember.hand.count("Wound") == 3


def test_prepare_grants_2_strength_then_cycle_returns_to_ink_blot():
    state = _vantom()
    after_ink_blot = apply(state, "EndTurn")
    after_inky_lance = apply(after_ink_blot, "EndTurn")
    after_dismember = apply(after_inky_lance, "EndTurn")
    assert after_dismember.monsters[0].intent == "Prepare"

    after_prepare = apply(after_dismember, "EndTurn")
    assert after_prepare.monsters[0].strength == 2
    assert after_prepare.monsters[0].intent == "Ink Blot"


# ── Slippery x9 ──────────────────────────────────────────────────────────────


def test_slippery_caps_first_nine_hp_losses_to_1_then_tenth_hits_full():
    state = _vantom(hand=["Strike"] * 10, statuses=[("Slippery", 9)], player_energy=20)
    assert state.monsters[0].statuses == ["Slippery"]

    current = state
    for _ in range(9):
        before_hp = current.monsters[0].hp
        current = apply(apply(current, "PlayCard:Strike"), "SelectTarget:Monster:0")
        assert before_hp - current.monsters[0].hp == 1

    assert "Slippery" not in current.monsters[0].statuses

    before_hp = current.monsters[0].hp
    current = apply(apply(current, "PlayCard:Strike"), "SelectTarget:Monster:0")
    assert before_hp - current.monsters[0].hp == 6
