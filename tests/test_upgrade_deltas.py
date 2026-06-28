"""Behavioural tests for HOL-53: the `UpgradeDelta` mechanism. Tracer cards
(Strike, Defend, Bash, Barricade) resolve their upgraded values at
`upgrade_level 1` via the `"+"` suffix through `apply`, while their base
(`upgrade_level 0`) behaviour is unchanged."""

from sts_sim import CombatState, Monster, PlayCardAction, SelectTargetAction, apply


def make_state(hand, seed=42, player_energy=3):
    return CombatState(
        player_hp=80,
        player_energy=player_energy,
        monsters=[Monster(hp=44, attack=0)],
        seed=seed,
        hand=list(hand),
    )


# ── Strike / Strike+ ─────────────────────────────────────────────────────────


def test_strike_deals_6_damage():
    state = make_state(hand=["Strike"])

    resolved = apply(apply(state, PlayCardAction("Strike")), SelectTargetAction(0))

    assert 44 - resolved.monsters[0].hp == 6


def test_strike_plus_deals_9_damage():
    state = make_state(hand=["Strike+"])

    resolved = apply(apply(state, PlayCardAction("Strike+")), SelectTargetAction(0))

    assert 44 - resolved.monsters[0].hp == 9


# ── Defend / Defend+ ─────────────────────────────────────────────────────────


def test_defend_grants_5_block():
    state = make_state(hand=["Defend"])

    resolved = apply(state, PlayCardAction("Defend"))

    assert resolved.player_block == 5


def test_defend_plus_grants_8_block():
    state = make_state(hand=["Defend+"])

    resolved = apply(state, PlayCardAction("Defend+"))

    assert resolved.player_block == 8


# ── Bash / Bash+ ─────────────────────────────────────────────────────────────


def test_bash_deals_8_damage_and_applies_2_vulnerable():
    state = make_state(hand=["Bash"])

    resolved = apply(apply(state, PlayCardAction("Bash")), SelectTargetAction(0))

    assert 44 - resolved.monsters[0].hp == 8
    assert resolved.monsters[0].statuses.count("Vulnerable") == 2


def test_bash_plus_deals_10_damage_and_applies_3_vulnerable():
    state = make_state(hand=["Bash+"])

    resolved = apply(apply(state, PlayCardAction("Bash+")), SelectTargetAction(0))

    assert 44 - resolved.monsters[0].hp == 10
    assert resolved.monsters[0].statuses.count("Vulnerable") == 3


# ── Barricade / Barricade+ ───────────────────────────────────────────────────


def test_barricade_costs_3_energy():
    state = make_state(hand=["Barricade"], player_energy=3)

    resolved = apply(state, PlayCardAction("Barricade"))

    assert resolved.player_energy == 0


def test_barricade_plus_costs_2_energy():
    state = make_state(hand=["Barricade+"], player_energy=3)

    resolved = apply(state, PlayCardAction("Barricade+"))

    assert resolved.player_energy == 1
