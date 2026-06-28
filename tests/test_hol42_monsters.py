"""Behavioural tests for HOL-42 monsters: Snapping Jaxfruit and four Ruby
Raiders — all pure combinations of existing EffectOps with no new engine
surface (DealDamage, GainBlock, ApplyStatusToSelf(Strength))."""

from sts_sim import CombatState, EndTurnAction, Monster, apply


def _jaxfruit(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=31, name="Snapping Jaxfruit")],
        seed=seed,
        hand=hand or [],
    )


def test_snapping_jaxfruit_opens_with_energy_orb():
    state = _jaxfruit()
    assert state.monsters[0].intent == "Energy Orb"


def test_energy_orb_deals_3_damage():
    state = _jaxfruit()
    after = apply(state, EndTurnAction())
    assert state.player_hp - after.player_hp == 3


def test_energy_orb_grants_2_strength():
    state = _jaxfruit()
    after = apply(state, EndTurnAction())
    assert after.monsters[0].strength == 2


def test_jaxfruit_repeats_energy_orb_forever():
    state = _jaxfruit()
    after1 = apply(state, EndTurnAction())
    assert after1.monsters[0].intent == "Energy Orb"
    after2 = apply(after1, EndTurnAction())
    assert after2.monsters[0].intent == "Energy Orb"


# ── Axe Ruby Raider ─────────────────────────────────────────────────────────


def _axe_raider(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=20, name="Axe Ruby Raider")],
        seed=seed,
        hand=hand or [],
    )


def test_axe_raider_opens_with_swing_1():
    state = _axe_raider()
    assert state.monsters[0].intent == "Swing 1"


def test_swing_1_deals_5_damage_and_gains_5_block():
    state = _axe_raider()
    after = apply(state, EndTurnAction())
    assert state.player_hp - after.player_hp == 5
    assert after.monsters[0].block == 5


def test_axe_cycle_swing_1_to_swing_2_to_big_swing():
    state = _axe_raider()
    after1 = apply(state, EndTurnAction())
    assert after1.monsters[0].intent == "Swing 2"
    after2 = apply(after1, EndTurnAction())
    assert after2.monsters[0].intent == "Big Swing"
    after3 = apply(after2, EndTurnAction())
    assert after3.monsters[0].intent == "Swing 1"


def test_big_swing_deals_12_damage():
    state = _axe_raider()
    after_swing1 = apply(state, EndTurnAction())
    after_swing2 = apply(after_swing1, EndTurnAction())
    assert after_swing2.monsters[0].intent == "Big Swing"
    after_big = apply(after_swing2, EndTurnAction())
    assert after_swing2.player_hp - after_big.player_hp == 12


# ── Assassin Ruby Raider ─────────────────────────────────────────────────────


def _assassin_raider(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=18, name="Assassin Ruby Raider")],
        seed=seed,
        hand=hand or [],
    )


def test_assassin_raider_opens_with_killshot():
    state = _assassin_raider()
    assert state.monsters[0].intent == "Killshot"


def test_killshot_deals_11_damage():
    state = _assassin_raider()
    after = apply(state, EndTurnAction())
    assert state.player_hp - after.player_hp == 11


def test_assassin_repeats_killshot_forever():
    state = _assassin_raider()
    after1 = apply(state, EndTurnAction())
    assert after1.monsters[0].intent == "Killshot"
    after2 = apply(after1, EndTurnAction())
    assert after2.monsters[0].intent == "Killshot"


# ── Brute Ruby Raider ────────────────────────────────────────────────────────


def _brute_raider(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=30, name="Brute Ruby Raider")],
        seed=seed,
        hand=hand or [],
    )


def test_brute_raider_opens_with_beat():
    state = _brute_raider()
    assert state.monsters[0].intent == "Beat"


def test_beat_deals_7_damage():
    state = _brute_raider()
    after = apply(state, EndTurnAction())
    assert state.player_hp - after.player_hp == 7


def test_brute_cycle_beat_to_roar():
    state = _brute_raider()
    after = apply(state, EndTurnAction())
    assert after.monsters[0].intent == "Roar"
    after_roar = apply(after, EndTurnAction())
    assert after_roar.monsters[0].intent == "Beat"


def test_roar_grants_3_strength():
    state = _brute_raider()
    after_beat = apply(state, EndTurnAction())
    after_roar = apply(after_beat, EndTurnAction())
    assert after_roar.monsters[0].strength == 3


# ── Crossbow Ruby Raider ─────────────────────────────────────────────────────


def _crossbow_raider(seed=42, hand=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=18, name="Crossbow Ruby Raider")],
        seed=seed,
        hand=hand or [],
    )


def test_crossbow_raider_opens_with_reload():
    state = _crossbow_raider()
    assert state.monsters[0].intent == "Reload"


def test_reload_gains_3_block():
    state = _crossbow_raider()
    after = apply(state, EndTurnAction())
    assert after.monsters[0].block == 3


def test_crossbow_alternates_reload_to_fire():
    state = _crossbow_raider()
    after = apply(state, EndTurnAction())
    assert after.monsters[0].intent == "Fire"


def test_fire_deals_14_damage():
    state = _crossbow_raider()
    after_reload = apply(state, EndTurnAction())
    assert after_reload.monsters[0].intent == "Fire"
    after_fire = apply(after_reload, EndTurnAction())
    assert after_reload.player_hp - after_fire.player_hp == 14


def test_crossbow_fire_returns_to_reload():
    state = _crossbow_raider()
    after_reload = apply(state, EndTurnAction())
    after_fire = apply(after_reload, EndTurnAction())
    assert after_fire.monsters[0].intent == "Reload"
