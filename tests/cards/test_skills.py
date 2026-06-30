"""Behavioural tests for Ironclad skill cards: non-damage, non-power cards
covering block, draw, hand manipulation, healing, and self-damage utilities."""

from sts_sim import (
    CombatState,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
)


# ── ShrugItOff ────────────────────────────────────────────────────────────────


def test_shrug_it_off_gains_8_block_and_draws_1():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=42,
        hand=["ShrugItOff"],
        draw_pile=["Strike"],
    )

    resolved = apply(state, PlayCardAction("ShrugItOff"))

    assert resolved.player_block == state.player_block + 8
    assert "Strike" in resolved.hand


# ── Taunt ─────────────────────────────────────────────────────────────────────


def test_taunt_gains_7_block_and_applies_vulnerable_to_target(make_state):
    state = make_state(hand=["Taunt"])

    awaiting_target = apply(state, PlayCardAction("Taunt"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert resolved.player_block == state.player_block + 7
    assert "Vulnerable" in resolved.monsters[0].statuses


# ── Bloodletting ──────────────────────────────────────────────────────────────


def test_bloodletting_loses_3_hp_and_gains_2_energy(make_state):
    state = make_state(hand=["Bloodletting"])

    resolved = apply(state, PlayCardAction("Bloodletting"))

    assert state.player_hp - resolved.player_hp == 3
    assert resolved.player_energy == state.player_energy + 2


# ── BloodWall ─────────────────────────────────────────────────────────────────


def test_blood_wall_loses_2_hp_and_gains_16_block(make_state):
    state = make_state(hand=["BloodWall"])

    resolved = apply(state, PlayCardAction("BloodWall"))

    assert state.player_hp - resolved.player_hp == 2
    assert resolved.player_block == state.player_block + 16


# ── Offering ──────────────────────────────────────────────────────────────────


def test_offering_loses_6_hp_gains_2_energy_draws_3_and_exhausts():
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


# ── Tremble ───────────────────────────────────────────────────────────────────


def test_tremble_applies_3_vulnerable_and_exhausts(make_state):
    state = make_state(hand=["Tremble"])

    awaiting_target = apply(state, PlayCardAction("Tremble"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert resolved.monsters[0].statuses.count("Vulnerable") == 3
    assert "Tremble" not in resolved.discard_pile
    assert "Tremble" in resolved.exhaust_pile


# ── Impervious ────────────────────────────────────────────────────────────────


def test_impervious_grants_30_block_and_exhausts(make_state):
    state = make_state(hand=["Impervious"])

    resolved = apply(state, PlayCardAction("Impervious"))

    assert resolved.player_block == state.player_block + 30
    assert "Impervious" not in resolved.discard_pile
    assert "Impervious" in resolved.exhaust_pile


# ── NotYet ────────────────────────────────────────────────────────────────────


def test_not_yet_heals_10_and_exhausts():
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


# ── TrueGrit ──────────────────────────────────────────────────────────────────


def test_true_grit_gains_7_block_and_exhausts_a_random_hand_card(make_state):
    state = make_state(hand=["TrueGrit", "Defend"])

    resolved = apply(state, PlayCardAction("TrueGrit"))

    assert resolved.player_block == state.player_block + 7
    assert resolved.hand == []
    assert "Defend" in resolved.exhaust_pile
    assert "TrueGrit" in resolved.discard_pile


# ── BurningPact ───────────────────────────────────────────────────────────────


def test_burning_pact_exhausts_a_random_hand_card_and_draws_2(make_state):
    state = make_state(
        hand=["BurningPact", "Defend"],
        draw_pile=["Strike", "Strike"],
    )

    resolved = apply(state, PlayCardAction("BurningPact"))

    assert "Defend" in resolved.exhaust_pile
    assert "BurningPact" in resolved.discard_pile
    assert resolved.hand.count("Strike") == 2


# ── SecondWind ────────────────────────────────────────────────────────────────


def test_second_wind_exhausts_non_attacks_and_gains_5_block_each(make_state):
    # For each non-Attack card in hand, exhaust it and gain 5 block.
    state = make_state(hand=["SecondWind", "Defend", "Rage", "Strike"])

    resolved = apply(state, PlayCardAction("SecondWind"))

    assert resolved.player_block == state.player_block + 10
    assert "Defend" in resolved.exhaust_pile
    assert "Rage" in resolved.exhaust_pile
    assert "Strike" in resolved.hand
    assert "SecondWind" in resolved.discard_pile


# ── Dominate ──────────────────────────────────────────────────────────────────


def test_dominate_applies_vulnerable_then_gains_strength_equal_to_resulting_stacks():
    # Target starts with 1 Vulnerable stack -> Dominate applies one more
    # (total 2), then the player gains 2 Strength.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6, statuses=[("Vulnerable", 1)])],
        seed=42,
        hand=["Dominate"],
    )

    awaiting_target = apply(state, PlayCardAction("Dominate"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert resolved.monsters[0].statuses.count("Vulnerable") == 2
    assert resolved.player_strength == 2
    assert "Dominate" in resolved.exhaust_pile


# ── Evil Eye ──────────────────────────────────────────────────────────────────


def test_evil_eye_grants_8_block_or_16_if_exhausted_a_card_this_turn(make_state):
    state = make_state(hand=["Evil Eye"])
    after_no_exhaust = apply(state, PlayCardAction("Evil Eye"))
    assert after_no_exhaust.player_block == 8

    state_with_exhaust = make_state(hand=["Evil Eye", "MoltenFist"])
    after_exhaust = apply(
        apply(
            apply(state_with_exhaust, PlayCardAction("MoltenFist")),
            SelectTargetAction(0),
        ),
        PlayCardAction("Evil Eye"),
    )
    assert after_exhaust.player_block == 16


# ── Forgotten Ritual ─────────────────────────────────────────────────────────


def test_forgotten_ritual_grants_3_energy_and_exhausts(make_state):
    state = make_state(hand=["Forgotten Ritual"], player_energy=1)
    resolved = apply(state, PlayCardAction("Forgotten Ritual"))
    assert resolved.player_energy == 1 + 3
    assert "Forgotten Ritual" in resolved.exhaust_pile
    assert "Forgotten Ritual" not in resolved.hand
