"""Behavioural tests for Phase 1 Ironclad cards: self-damage, energy, heal,
and exhausting non-Power cards."""

from sts_sim import CombatState, Monster, PlayCardAction, SelectTargetAction, apply


def make_state(hand=("Strike",)):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=list(hand),
    )


# ── Bloodletting ────────────────────────────────────────────────────────────


def test_bloodletting_loses_3_hp_and_gains_2_energy():
    # Per the wiki, Bloodletting costs 0, deals 3 unblockable self-damage,
    # and grants 2 Energy.
    state = make_state(hand=["Bloodletting"])

    resolved = apply(state, PlayCardAction("Bloodletting"))

    assert state.player_hp - resolved.player_hp == 3
    assert resolved.player_energy == state.player_energy + 2


# ── BloodWall ───────────────────────────────────────────────────────────────


def test_blood_wall_loses_2_hp_and_gains_16_block():
    # Per the wiki, BloodWall costs 2, deals 2 unblockable self-damage, and
    # grants 16 Block.
    state = make_state(hand=["BloodWall"])

    resolved = apply(state, PlayCardAction("BloodWall"))

    assert state.player_hp - resolved.player_hp == 2
    assert resolved.player_block == state.player_block + 16


# ── Hemokinesis ─────────────────────────────────────────────────────────────


def test_hemokinesis_loses_2_hp_and_deals_15_damage():
    # Per the wiki, Hemokinesis costs 1, deals 2 unblockable self-damage, and
    # deals 15 damage to a chosen enemy.
    state = make_state(hand=["Hemokinesis"])

    awaiting_target = apply(state, PlayCardAction("Hemokinesis"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.player_hp - resolved.player_hp == 2
    assert state.monsters[0].hp - resolved.monsters[0].hp == 15


# ── Offering ────────────────────────────────────────────────────────────────


def test_offering_loses_6_hp_gains_2_energy_draws_3_and_exhausts():
    # Per the wiki, Offering costs 0, deals 6 unblockable self-damage, grants
    # 2 Energy, draws 3 cards, and Exhausts itself.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["Offering"],
        draw_pile=["Strike", "Strike", "Strike", "Strike"],
    )

    resolved = apply(state, PlayCardAction("Offering"))

    assert state.player_hp - resolved.player_hp == 6
    assert resolved.player_energy == state.player_energy + 2
    assert len(resolved.hand) == 3
    assert "Offering" not in resolved.discard_pile
    assert "Offering" in resolved.exhaust_pile


# ── Tremble ─────────────────────────────────────────────────────────────────


def test_tremble_applies_3_vulnerable_and_exhausts():
    # Per the wiki, Tremble costs 1, applies 3 stacks of Vulnerable to a
    # chosen enemy, and Exhausts.
    state = make_state(hand=["Tremble"])

    awaiting_target = apply(state, PlayCardAction("Tremble"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert resolved.monsters[0].statuses.count("Vulnerable") == 3
    assert "Tremble" not in resolved.discard_pile
    assert "Tremble" in resolved.exhaust_pile


# ── Impervious ──────────────────────────────────────────────────────────────


def test_impervious_grants_30_block_and_exhausts():
    # Per the wiki, Impervious costs 2, grants 30 Block, and Exhausts.
    state = make_state(hand=["Impervious"])

    resolved = apply(state, PlayCardAction("Impervious"))

    assert resolved.player_block == state.player_block + 30
    assert "Impervious" not in resolved.discard_pile
    assert "Impervious" in resolved.exhaust_pile


# ── NotYet ──────────────────────────────────────────────────────────────────


def test_not_yet_heals_10_and_exhausts():
    # Per the wiki, NotYet costs 2, heals 10 HP, and Exhausts.
    state = CombatState(
        player_hp=50,
        player_max_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["NotYet"],
    )

    resolved = apply(state, PlayCardAction("NotYet"))

    assert resolved.player_hp == state.player_hp + 10
    assert "NotYet" not in resolved.discard_pile
    assert "NotYet" in resolved.exhaust_pile


def test_not_yet_heal_caps_at_max_hp():
    state = CombatState(
        player_hp=75,
        player_max_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["NotYet"],
    )

    resolved = apply(state, PlayCardAction("NotYet"))

    assert resolved.player_hp == 80
