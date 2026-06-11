"""Behavioural tests for Nibbit and Fuzzy Wurm Crawler."""

from sts_sim import CombatState, Monster, apply


def _nibbit(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, name="Nibbit")],
        seed=seed,
        hand=hand or [],
        deck=deck,
    )


def _fwc(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=56, attack=0, name="Fuzzy Wurm Crawler")],
        seed=seed,
        hand=hand or [],
        deck=deck,
    )


def _byrdonis(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=84, attack=0, name="Byrdonis")],
        seed=seed,
        hand=hand or [],
        deck=deck,
    )


# ── Nibbit ────────────────────────────────────────────────────────────────────


def test_nibbit_opens_with_butt():
    state = _nibbit()
    assert state.monsters[0].intent == "Butt"


def test_nibbit_butt_deals_12_damage():
    state = _nibbit()
    assert state.monsters[0].intent == "Butt"
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 12


def test_nibbit_cycles_butt_hesitant_slice_hiss_and_repeats():
    state = _nibbit()
    assert state.monsters[0].intent == "Butt"

    after_butt = apply(state, "EndTurn")
    assert after_butt.monsters[0].intent == "Hesitant Slice"

    after_hesitant = apply(after_butt, "EndTurn")
    assert after_hesitant.monsters[0].intent == "Hiss"

    after_hiss = apply(after_hesitant, "EndTurn")
    assert after_hiss.monsters[0].intent == "Butt"


def test_nibbit_hesitant_slice_deals_damage_and_gains_block():
    state = _nibbit()
    after_butt = apply(state, "EndTurn")
    assert after_butt.monsters[0].intent == "Hesitant Slice"

    hp_before = after_butt.player_hp
    block_before = after_butt.monsters[0].block
    after_hesitant = apply(after_butt, "EndTurn")

    assert hp_before - after_hesitant.player_hp == 6
    assert after_hesitant.monsters[0].block == block_before + 5


def test_nibbit_hiss_grants_two_strength():
    state = _nibbit()
    after_butt = apply(state, "EndTurn")
    after_hesitant = apply(after_butt, "EndTurn")
    assert after_hesitant.monsters[0].intent == "Hiss"
    strength_before = after_hesitant.monsters[0].strength

    after_hiss = apply(after_hesitant, "EndTurn")

    assert after_hiss.monsters[0].strength == strength_before + 2


def test_nibbit_hiss_strength_amplifies_subsequent_butt():
    # After one Hiss (2 Str), the next Butt deals 12+2=14.
    state = _nibbit()
    after_butt = apply(state, "EndTurn")  # Butt (12 dmg)
    after_hesitant = apply(after_butt, "EndTurn")  # Hesitant Slice
    after_hiss = apply(after_hesitant, "EndTurn")  # Hiss (+2 Str)
    assert after_hiss.monsters[0].intent == "Butt"

    hp_before = after_hiss.player_hp
    after_second_butt = apply(after_hiss, "EndTurn")
    assert hp_before - after_second_butt.player_hp == 14


# ── Fuzzy Wurm Crawler ────────────────────────────────────────────────────────


def test_fwc_opens_with_acid_goop():
    state = _fwc()
    assert state.monsters[0].intent == "Acid Goop"


def test_fwc_acid_goop_deals_4_damage():
    state = _fwc()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 4


def test_fwc_alternates_acid_goop_and_inhale():
    state = _fwc()
    assert state.monsters[0].intent == "Acid Goop"

    after_goop = apply(state, "EndTurn")
    assert after_goop.monsters[0].intent == "Inhale"

    after_inhale = apply(after_goop, "EndTurn")
    assert after_inhale.monsters[0].intent == "Acid Goop"

    after_goop2 = apply(after_inhale, "EndTurn")
    assert after_goop2.monsters[0].intent == "Inhale"


def test_fwc_inhale_grants_seven_strength():
    state = _fwc()
    after_goop = apply(state, "EndTurn")
    assert after_goop.monsters[0].intent == "Inhale"
    strength_before = after_goop.monsters[0].strength

    after_inhale = apply(after_goop, "EndTurn")

    assert after_inhale.monsters[0].strength == strength_before + 7


def test_fwc_acid_goop_scales_with_accumulated_strength():
    # After one Inhale (7 Str), Acid Goop deals 4+7=11.
    state = _fwc()
    after_goop = apply(state, "EndTurn")  # Acid Goop (4 dmg)
    after_inhale = apply(after_goop, "EndTurn")  # Inhale (+7 Str)
    assert after_inhale.monsters[0].intent == "Acid Goop"
    assert after_inhale.monsters[0].strength == 7

    hp_before = after_inhale.player_hp
    after_goop2 = apply(after_inhale, "EndTurn")
    assert hp_before - after_goop2.player_hp == 11


# ── Byrdonis ──────────────────────────────────────────────────────────────────


def test_byrdonis_opens_with_swoop():
    state = _byrdonis()
    assert state.monsters[0].intent == "Swoop"


def test_byrdonis_swoop_deals_17_damage():
    state = _byrdonis()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 17


def test_byrdonis_alternates_swoop_and_peck():
    state = _byrdonis()
    assert state.monsters[0].intent == "Swoop"

    after_swoop = apply(state, "EndTurn")
    assert after_swoop.monsters[0].intent == "Peck"

    after_peck = apply(after_swoop, "EndTurn")
    assert after_peck.monsters[0].intent == "Swoop"


def test_byrdonis_gains_one_strength_every_turn():
    # Territorial 1: +1 Strength at the end of every Byrdonis turn,
    # regardless of which move it used.
    state = _byrdonis()
    assert state.monsters[0].strength == 0

    after_swoop = apply(state, "EndTurn")
    assert after_swoop.monsters[0].strength == 1

    after_peck = apply(after_swoop, "EndTurn")
    assert after_peck.monsters[0].strength == 2


def test_byrdonis_strength_amplifies_subsequent_moves():
    # After one Swoop (+1 Str), the next Peck deals (3+1)*3=12.
    state = _byrdonis()
    after_swoop = apply(state, "EndTurn")  # Swoop (17 dmg, +1 Str)
    assert after_swoop.monsters[0].intent == "Peck"
    assert after_swoop.monsters[0].strength == 1

    hp_before = after_swoop.player_hp
    after_peck = apply(after_swoop, "EndTurn")
    assert hp_before - after_peck.player_hp == 12
