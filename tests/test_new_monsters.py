"""Behavioural tests for Nibbit and Fuzzy Wurm Crawler."""
import pytest
from sts_sim import CombatState, apply


def _nibbit(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=0,
        seed=seed,
        hand=hand or [],
        deck=deck,
        monster_name="Nibbit",
    )


def _fwc(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=56,
        monster_attack=0,
        seed=seed,
        hand=hand or [],
        deck=deck,
        monster_name="Fuzzy Wurm Crawler",
    )


# ── Nibbit ────────────────────────────────────────────────────────────────────

def test_nibbit_opens_with_butt():
    state = _nibbit()
    assert state.monster_intent == "Butt"


def test_nibbit_butt_deals_12_damage():
    state = _nibbit()
    assert state.monster_intent == "Butt"
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 12


def test_nibbit_cycles_butt_hesitant_slice_hiss_and_repeats():
    state = _nibbit()
    assert state.monster_intent == "Butt"

    after_butt = apply(state, "EndTurn")
    assert after_butt.monster_intent == "Hesitant Slice"

    after_hesitant = apply(after_butt, "EndTurn")
    assert after_hesitant.monster_intent == "Hiss"

    after_hiss = apply(after_hesitant, "EndTurn")
    assert after_hiss.monster_intent == "Butt"


def test_nibbit_hesitant_slice_deals_damage_and_gains_block():
    state = _nibbit()
    after_butt = apply(state, "EndTurn")
    assert after_butt.monster_intent == "Hesitant Slice"

    hp_before = after_butt.player_hp
    block_before = after_butt.monster_block
    after_hesitant = apply(after_butt, "EndTurn")

    assert hp_before - after_hesitant.player_hp == 6
    assert after_hesitant.monster_block == block_before + 5


def test_nibbit_hiss_grants_two_strength():
    state = _nibbit()
    after_butt = apply(state, "EndTurn")
    after_hesitant = apply(after_butt, "EndTurn")
    assert after_hesitant.monster_intent == "Hiss"
    strength_before = after_hesitant.monster_strength

    after_hiss = apply(after_hesitant, "EndTurn")

    assert after_hiss.monster_strength == strength_before + 2


def test_nibbit_hiss_strength_amplifies_subsequent_butt():
    # After one Hiss (2 Str), the next Butt deals 12+2=14.
    state = _nibbit()
    after_butt = apply(state, "EndTurn")       # Butt (12 dmg)
    after_hesitant = apply(after_butt, "EndTurn")  # Hesitant Slice
    after_hiss = apply(after_hesitant, "EndTurn")   # Hiss (+2 Str)
    assert after_hiss.monster_intent == "Butt"

    hp_before = after_hiss.player_hp
    after_second_butt = apply(after_hiss, "EndTurn")
    assert hp_before - after_second_butt.player_hp == 14


# ── Fuzzy Wurm Crawler ────────────────────────────────────────────────────────

def test_fwc_opens_with_acid_goop():
    state = _fwc()
    assert state.monster_intent == "Acid Goop"


def test_fwc_acid_goop_deals_4_damage():
    state = _fwc()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 4


def test_fwc_alternates_acid_goop_and_inhale():
    state = _fwc()
    assert state.monster_intent == "Acid Goop"

    after_goop = apply(state, "EndTurn")
    assert after_goop.monster_intent == "Inhale"

    after_inhale = apply(after_goop, "EndTurn")
    assert after_inhale.monster_intent == "Acid Goop"

    after_goop2 = apply(after_inhale, "EndTurn")
    assert after_goop2.monster_intent == "Inhale"


def test_fwc_inhale_grants_seven_strength():
    state = _fwc()
    after_goop = apply(state, "EndTurn")
    assert after_goop.monster_intent == "Inhale"
    strength_before = after_goop.monster_strength

    after_inhale = apply(after_goop, "EndTurn")

    assert after_inhale.monster_strength == strength_before + 7


def test_fwc_acid_goop_scales_with_accumulated_strength():
    # After one Inhale (7 Str), Acid Goop deals 4+7=11.
    state = _fwc()
    after_goop = apply(state, "EndTurn")    # Acid Goop (4 dmg)
    after_inhale = apply(after_goop, "EndTurn")  # Inhale (+7 Str)
    assert after_inhale.monster_intent == "Acid Goop"
    assert after_inhale.monster_strength == 7

    hp_before = after_inhale.player_hp
    after_goop2 = apply(after_inhale, "EndTurn")
    assert hp_before - after_goop2.player_hp == 11
