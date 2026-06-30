"""Behavioural tests for the UpgradeDelta mechanism: tracer cards (Strike,
Defend, Bash, Barricade) and a batch of Wave A upgraded cards confirm that
the `"+"` suffix resolves upgraded values through `apply`."""

from sts_sim import (
    CombatState,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
)


def _state(hand, seed=42, player_energy=3, monsters=None, **kwargs):
    return CombatState(
        player_hp=80,
        player_energy=player_energy,
        monsters=monsters or [Monster(hp=99, attack=0)],
        seed=seed,
        hand=list(hand),
        **kwargs,
    )


# ── Strike / Strike+ ─────────────────────────────────────────────────────────


def test_strike_deals_6_damage():
    state = _state(hand=["Strike"], monsters=[Monster(hp=44, attack=0)])

    resolved = apply(apply(state, PlayCardAction("Strike")), SelectTargetAction(0))

    assert 44 - resolved.monsters[0].hp == 6


def test_strike_plus_deals_9_damage():
    state = _state(hand=["Strike+"], monsters=[Monster(hp=44, attack=0)])

    resolved = apply(apply(state, PlayCardAction("Strike+")), SelectTargetAction(0))

    assert 44 - resolved.monsters[0].hp == 9


# ── Defend / Defend+ ─────────────────────────────────────────────────────────


def test_defend_grants_5_block():
    state = _state(hand=["Defend"], monsters=[Monster(hp=44, attack=0)])

    resolved = apply(state, PlayCardAction("Defend"))

    assert resolved.player_block == 5


def test_defend_plus_grants_8_block():
    state = _state(hand=["Defend+"], monsters=[Monster(hp=44, attack=0)])

    resolved = apply(state, PlayCardAction("Defend+"))

    assert resolved.player_block == 8


# ── Bash / Bash+ ─────────────────────────────────────────────────────────────


def test_bash_deals_8_damage_and_applies_2_vulnerable():
    state = _state(hand=["Bash"], monsters=[Monster(hp=44, attack=0)])

    resolved = apply(apply(state, PlayCardAction("Bash")), SelectTargetAction(0))

    assert 44 - resolved.monsters[0].hp == 8
    assert resolved.monsters[0].statuses.count("Vulnerable") == 2


def test_bash_plus_deals_10_damage_and_applies_3_vulnerable():
    state = _state(hand=["Bash+"], monsters=[Monster(hp=44, attack=0)])

    resolved = apply(apply(state, PlayCardAction("Bash+")), SelectTargetAction(0))

    assert 44 - resolved.monsters[0].hp == 10
    assert resolved.monsters[0].statuses.count("Vulnerable") == 3


# ── Barricade / Barricade+ ───────────────────────────────────────────────────


def test_barricade_costs_3_energy():
    state = _state(
        hand=["Barricade"], monsters=[Monster(hp=44, attack=0)], player_energy=3
    )

    resolved = apply(state, PlayCardAction("Barricade"))

    assert resolved.player_energy == 0


def test_barricade_plus_costs_2_energy():
    state = _state(
        hand=["Barricade+"], monsters=[Monster(hp=44, attack=0)], player_energy=3
    )

    resolved = apply(state, PlayCardAction("Barricade+"))

    assert resolved.player_energy == 1


# ── Wave A upgrades ───────────────────────────────────────────────────────────


def test_aggression_plus_still_installs_aggression_status():
    state = _state(hand=["Aggression+"], player_energy=1)

    resolved = apply(state, PlayCardAction("Aggression+"))

    assert "Aggression" in resolved.player_statuses
    assert "Aggression+" in resolved.exhaust_pile


def test_ashen_strike_plus_deals_6_plus_4_per_card_in_exhaust_pile():
    # 2 cards already in exhaust pile -> 6 + 4*2 = 14 damage.
    state = _state(hand=["AshenStrike+"], exhaust_pile=["Tremble", "Impervious"])

    after_play = apply(state, PlayCardAction("AshenStrike+"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 14


def test_bloodletting_plus_gains_3_energy():
    state = _state(hand=["Bloodletting+"], player_energy=1)

    resolved = apply(state, PlayCardAction("Bloodletting+"))

    assert resolved.player_energy == 1 + 3
    assert resolved.player_hp == state.player_hp - 3


def test_blood_wall_plus_grants_20_block():
    state = _state(hand=["BloodWall+"], player_energy=2)

    resolved = apply(state, PlayCardAction("BloodWall+"))

    assert resolved.player_block == 20
    assert resolved.player_hp == state.player_hp - 2


def test_bludgeon_plus_deals_42_damage():
    state = _state(hand=["Bludgeon+"], player_energy=3)

    after_play = apply(state, PlayCardAction("Bludgeon+"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 42


def test_body_slam_plus_costs_0_energy():
    state = _state(hand=["BodySlam+"], player_energy=1, player_block=12)

    after_play = apply(state, PlayCardAction("BodySlam+"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert resolved.player_energy == 1
    assert state.monsters[0].hp - resolved.monsters[0].hp == 12


def test_break_plus_deals_30_damage_and_applies_7_vulnerable():
    state = _state(hand=["Break+"])

    after_play = apply(state, PlayCardAction("Break+"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 30
    assert resolved.monsters[0].statuses.count("Vulnerable") == 7


def test_breakthrough_plus_deals_13_damage_to_all_enemies():
    state = _state(
        hand=["Breakthrough+"],
        monsters=[Monster(hp=99, attack=0), Monster(hp=99, attack=0)],
    )

    resolved = apply(state, PlayCardAction("Breakthrough+"))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 13
    assert state.monsters[1].hp - resolved.monsters[1].hp == 13


def test_bully_plus_deals_4_plus_3_per_vulnerable_stack_on_target():
    # Target has 3 stacks of Vulnerable -> base 4 + 3*3 = 13, amplified 1.5x
    # -> floor(13*1.5) = 19.
    state = _state(
        hand=["Bully+"],
        monsters=[Monster(hp=99, attack=0, statuses=[("Vulnerable", 3)])],
        player_energy=0,
    )

    after_play = apply(state, PlayCardAction("Bully+"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 19


def test_burning_pact_plus_exhausts_a_card_and_draws_3():
    state = _state(
        hand=["BurningPact+", "Strike"],
        draw_pile=["Defend", "Defend", "Defend", "Defend"],
        player_energy=1,
    )

    resolved = apply(state, PlayCardAction("BurningPact+"))

    assert len(resolved.exhaust_pile) == 1
    assert len(resolved.hand) == 3


def test_cinder_plus_deals_24_damage():
    state = _state(hand=["Cinder+"], player_energy=2)

    after_play = apply(state, PlayCardAction("Cinder+"))
    resolved = apply(after_play, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 24


def test_colossus_plus_grants_8_block():
    state = _state(hand=["Colossus+"], player_energy=1)

    resolved = apply(state, PlayCardAction("Colossus+"))

    assert resolved.player_block == 8


def test_conflagration_plus_deals_9_plus_3_per_attack_played_this_turn():
    state = _state(
        hand=["Strike", "Strike", "Conflagration+"],
        monsters=[Monster(hp=99, attack=0), Monster(hp=99, attack=0)],
        player_energy=3,
    )

    after_strike_1 = apply(
        apply(state, PlayCardAction("Strike")), SelectTargetAction(0)
    )
    after_strike_2 = apply(
        apply(after_strike_1, PlayCardAction("Strike")), SelectTargetAction(0)
    )

    resolved = apply(after_strike_2, PlayCardAction("Conflagration+"))

    # 9 + 3*2 = 15 damage to each enemy.
    assert after_strike_2.monsters[0].hp - resolved.monsters[0].hp == 15
    assert after_strike_2.monsters[1].hp - resolved.monsters[1].hp == 15
