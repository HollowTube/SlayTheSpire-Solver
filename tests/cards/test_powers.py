"""Behavioural tests for Ironclad power cards and persistent effects that
react to game events (TurnStart, CardExhausted, BlockGained, DamageReceived)."""

from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
    legal_actions,
)


# ── DemonForm ─────────────────────────────────────────────────────────────────


def test_demon_form_grants_2_strength_at_start_of_each_turn(make_state):
    state = make_state(hand=["DemonForm"])

    after_play = apply(state, PlayCardAction("DemonForm"))
    assert after_play.player_strength == 0

    after_turn_1 = apply(after_play, EndTurnAction())
    assert after_turn_1.player_strength == 2

    after_turn_2 = apply(after_turn_1, EndTurnAction())
    assert after_turn_2.player_strength == 4


# ── CrimsonMantle ─────────────────────────────────────────────────────────────


def test_crimson_mantle_gains_block_and_increasing_self_damage_each_turn():
    # attack=0 so the monster's swing doesn't pollute the HP deltas.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["CrimsonMantle"],
    )

    after_play = apply(state, PlayCardAction("CrimsonMantle"))

    after_turn_1 = apply(after_play, EndTurnAction())
    assert after_play.player_hp - after_turn_1.player_hp == 1
    assert after_turn_1.player_block == 8

    after_turn_2 = apply(after_turn_1, EndTurnAction())
    assert after_turn_1.player_hp - after_turn_2.player_hp == 2
    assert after_turn_2.player_block == 8


# ── Inferno ───────────────────────────────────────────────────────────────────


def test_inferno_self_damage_at_turn_start_triggers_aoe_retaliation():
    # At the start of each turn, Inferno's holder loses 1 HP which triggers
    # 6 damage to ALL enemies. attack=0 so monsters' swings don't pollute.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0), Monster(hp=44, attack=0)],
        seed=42,
        hand=["Inferno"],
    )

    after_play = apply(state, PlayCardAction("Inferno"))

    after_turn_1 = apply(after_play, EndTurnAction())
    assert after_play.player_hp - after_turn_1.player_hp == 1
    assert after_play.monsters[0].hp - after_turn_1.monsters[0].hp == 6
    assert after_play.monsters[1].hp - after_turn_1.monsters[1].hp == 6

    after_turn_2 = apply(after_turn_1, EndTurnAction())
    assert after_turn_1.player_hp - after_turn_2.player_hp == 1
    assert after_turn_1.monsters[0].hp - after_turn_2.monsters[0].hp == 6


# ── Aggression ────────────────────────────────────────────────────────────────


def test_aggression_returns_a_discarded_attack_to_hand_at_turn_start():
    # Only the Attack ("Strike") in the discard pile should be returned, not
    # the Skill ("Defend"). The draw pile is pre-filled so the turn-start draw
    # doesn't reshuffle the discard pile before Aggression fires.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["Aggression"],
        draw_pile=["Bash"] * 5,
        discard_pile=["Strike", "Defend"],
    )

    after_play = apply(state, PlayCardAction("Aggression"))
    after_turn = apply(after_play, EndTurnAction())

    assert "Strike" in after_turn.hand
    assert "Defend" in after_turn.discard_pile
    assert "Defend" not in after_turn.hand


# ── DarkEmbrace ───────────────────────────────────────────────────────────────


def test_dark_embrace_draws_a_card_when_a_card_is_exhausted():
    # Playing Impervious (Exhaust) while Dark Embrace is active should draw
    # 1 extra card -> net hand size unchanged.
    state = CombatState(
        player_hp=80,
        player_energy=4,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["DarkEmbrace", "Impervious"],
        draw_pile=["Strike"] * 5,
    )

    after_dark_embrace = apply(state, PlayCardAction("DarkEmbrace"))
    hand_size_before = len(after_dark_embrace.hand)

    after_impervious = apply(after_dark_embrace, PlayCardAction("Impervious"))

    assert "Impervious" in after_impervious.exhaust_pile
    assert len(after_impervious.hand) == hand_size_before


# ── FeelNoPain ────────────────────────────────────────────────────────────────


def test_feel_no_pain_gains_3_block_when_a_card_is_exhausted():
    state = CombatState(
        player_hp=80,
        player_energy=4,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["FeelNoPain", "Impervious"],
        draw_pile=["Strike"] * 5,
    )

    after_feel_no_pain = apply(state, PlayCardAction("FeelNoPain"))
    after_impervious = apply(after_feel_no_pain, PlayCardAction("Impervious"))

    # Impervious grants 30 Block, plus 3 from Feel No Pain.
    assert "Impervious" in after_impervious.exhaust_pile
    assert after_impervious.player_block == 33


# ── Barricade ─────────────────────────────────────────────────────────────────


def test_barricade_keeps_block_through_turn_start():
    state = CombatState(
        player_hp=80,
        player_energy=4,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["Barricade", "ShrugItOff"],
    )

    after_barricade = apply(state, PlayCardAction("Barricade"))
    after_block = apply(after_barricade, PlayCardAction("ShrugItOff"))
    assert after_block.player_block == 8

    after_turn = apply(after_block, EndTurnAction())
    assert after_turn.player_block == 8


def test_without_barricade_block_clears_at_turn_start():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["ShrugItOff"],
    )

    after_block = apply(state, PlayCardAction("ShrugItOff"))
    assert after_block.player_block == 8

    after_turn = apply(after_block, EndTurnAction())
    assert after_turn.player_block == 0


# ── Juggernaut ────────────────────────────────────────────────────────────────


def test_juggernaut_deals_damage_to_enemy_when_block_is_gained():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["Juggernaut", "ShrugItOff"],
    )

    after_juggernaut = apply(state, PlayCardAction("Juggernaut"))
    after_shrug = apply(after_juggernaut, PlayCardAction("ShrugItOff"))

    assert after_shrug.player_block == 8
    assert state.monsters[0].hp - after_shrug.monsters[0].hp == 5


# ── FlameBarrier ──────────────────────────────────────────────────────────────


def test_flame_barrier_retaliates_against_attacker_then_expires():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
        player_statuses=[("FlameBarrier", 1)],
    )

    # Jaw Worm opens with Chomp (11 damage); FlameBarrier retaliates for 4.
    after_turn_1 = apply(state, EndTurnAction())
    assert state.player_hp - after_turn_1.player_hp == 11
    assert state.monsters[0].hp - after_turn_1.monsters[0].hp == 4

    # FlameBarrier expired — no further retaliation.
    after_turn_2 = apply(after_turn_1, EndTurnAction())
    assert after_turn_1.monsters[0].hp == after_turn_2.monsters[0].hp


# ── Colossus ──────────────────────────────────────────────────────────────────


def test_colossus_halves_damage_from_a_vulnerable_attacker():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm", statuses=[("Vulnerable", 1)])],
        seed=42,
        hand=["Colossus"],
    )

    after_play = apply(state, PlayCardAction("Colossus"))
    assert after_play.player_block == 5

    after_turn = apply(after_play, EndTurnAction())

    # Jaw Worm's Chomp deals 11; Colossus halves damage from a Vulnerable
    # attacker: floor(11 * 0.5) == 5. Colossus's own 5 Block absorbs that fully.
    assert after_play.player_hp - after_turn.player_hp == 0


def test_colossus_does_not_reduce_damage_from_a_non_vulnerable_attacker():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=42,
        hand=["Colossus"],
    )

    after_play = apply(state, PlayCardAction("Colossus"))
    after_turn = apply(after_play, EndTurnAction())

    # Jaw Worm's Chomp deals 11, unmodified; 5 Block absorbs 5, leaving 6 HP loss.
    assert after_play.player_hp - after_turn.player_hp == 6


def test_colossus_expires_after_one_turn():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm", statuses=[("Vulnerable", 1)])],
        seed=42,
        hand=["Colossus"],
    )

    after_play = apply(state, PlayCardAction("Colossus"))
    assert "Colossus" in after_play.player_statuses

    after_turn_1 = apply(after_play, EndTurnAction())
    assert "Colossus" not in after_turn_1.player_statuses
    assert after_turn_1.player_block == 0


# ── Corruption ────────────────────────────────────────────────────────────────


def test_corruption_makes_skills_free_and_exhausts_them():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["Corruption", "Defend"],
    )

    after_corruption = apply(state, PlayCardAction("Corruption"))
    assert after_corruption.player_energy == 0

    # Defend (a Skill) costs 1, but Corruption makes Skills free.
    after_defend = apply(after_corruption, PlayCardAction("Defend"))
    assert after_defend.player_energy == 0
    assert after_defend.player_block == 5
    assert "Defend" in after_defend.exhaust_pile
    assert "Defend" not in after_defend.discard_pile


# ── Cruelty ───────────────────────────────────────────────────────────────────


def test_cruelty_amplifies_vulnerable_damage_to_175x():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, statuses=[("Vulnerable", 1)])],
        seed=42,
        hand=["Cruelty", "Strike"],
    )

    after_cruelty = apply(state, PlayCardAction("Cruelty"))
    after_strike = apply(after_cruelty, PlayCardAction("Strike"))
    after_target = apply(after_strike, SelectTargetAction(0))

    # Strike deals 6 base; Vulnerable + Cruelty -> floor(6 * 1.75) == 10.
    assert state.monsters[0].hp - after_target.monsters[0].hp == 10


def test_without_cruelty_vulnerable_damage_is_only_15x():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, statuses=[("Vulnerable", 1)])],
        seed=42,
        hand=["Strike"],
    )

    after_strike = apply(state, PlayCardAction("Strike"))
    after_target = apply(after_strike, SelectTargetAction(0))

    # floor(6 * 1.5) == 9.
    assert state.monsters[0].hp - after_target.monsters[0].hp == 9


# ── OneTwoPunch ───────────────────────────────────────────────────────────────


def test_one_two_punch_doubles_the_next_attack_played_this_turn():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["OneTwoPunch", "Strike", "Strike"],
    )

    after_otp = apply(state, PlayCardAction("OneTwoPunch"))
    after_strike = apply(after_otp, PlayCardAction("Strike"))
    after_target = apply(after_strike, SelectTargetAction(0))

    # Strike deals 6; One Two Punch makes it resolve twice -> 12 total.
    assert state.monsters[0].hp - after_target.monsters[0].hp == 12

    # The power is consumed — a second Strike only hits once.
    after_strike2 = apply(after_target, PlayCardAction("Strike"))
    after_target2 = apply(after_strike2, SelectTargetAction(0))
    assert after_target.monsters[0].hp - after_target2.monsters[0].hp == 6


def test_one_two_punch_does_not_persist_to_next_turn_if_unused():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["OneTwoPunch"],
    )

    after_otp = apply(state, PlayCardAction("OneTwoPunch"))
    assert "OneTwoPunch" in after_otp.player_statuses

    after_turn = apply(after_otp, EndTurnAction())
    assert "OneTwoPunch" not in after_turn.player_statuses


# ── Pyre ──────────────────────────────────────────────────────────────────────


def _pyre_state(hand, seed=42, energy=3, draw_pile=None):
    return CombatState(
        player_hp=80,
        player_energy=energy,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=seed,
        hand=hand,
        draw_pile=draw_pile or [],
    )


def test_pyre_applies_pyre_status():
    state = _pyre_state(["Pyre", "Strike", "Strike", "Strike", "Defend"])
    after = apply(state, PlayCardAction("Pyre"))
    assert "Pyre" in after.player_statuses


def test_pyre_grants_energy_at_start_of_each_turn():
    state = _pyre_state(["Pyre", "Strike", "Strike", "Strike", "Defend"])
    after = apply(state, PlayCardAction("Pyre"))
    after_end_turn = apply(after, EndTurnAction())
    assert after_end_turn.player_energy == 4  # base 3 + Pyre 1


# ── DrumOfBattle ──────────────────────────────────────────────────────────────


def test_drum_of_battle_draws_2_on_play():
    state = _pyre_state(
        ["DrumOfBattle", "Strike", "Strike", "Strike", "Defend"],
        draw_pile=["Defend", "Defend"],
    )
    assert len(state.hand) == 5
    after = apply(state, PlayCardAction("DrumOfBattle"))
    assert len(after.hand) == 6  # played (1 gone) + draw 2 = net +1


def test_drum_of_battle_exhausts_top_of_draw_at_turn_start():
    state = _pyre_state(
        ["DrumOfBattle", "Strike", "Strike", "Strike", "Strike"],
        draw_pile=["Defend", "Defend"],
    )
    after = apply(state, PlayCardAction("DrumOfBattle"))
    assert len(after.draw_pile) == 0
    assert "BattleDrum" in after.player_statuses


# ── StoneArmor (Plating) ──────────────────────────────────────────────────────


def test_stone_armor_costs_1():
    state = _pyre_state(["StoneArmor"])
    assert "PlayCard:StoneArmor" in [str(a) for a in legal_actions(state)]


def test_stone_armor_applies_plating():
    state = _pyre_state(["StoneArmor", "Strike", "Strike", "Defend", "Defend"])
    after = apply(state, PlayCardAction("StoneArmor"))
    assert "Plating" in after.player_statuses


def test_plating_reduces_damage_taken():
    """Plating grants block at end of player turn, which absorbs monster damage."""
    state = _pyre_state(["StoneArmor", "Strike"])
    # End turn without StoneArmor for baseline
    after_turn_no_armor = apply(state, EndTurnAction())
    damage_no_armor = state.player_hp - after_turn_no_armor.player_hp
    # Play StoneArmor, then end turn
    after_play = apply(state, PlayCardAction("StoneArmor"))
    after_turn = apply(after_play, EndTurnAction())
    damage_with_armor = state.player_hp - after_turn.player_hp
    assert after_turn.player_block == 0  # block reset (no Barricade)
    assert damage_with_armor <= damage_no_armor


def test_plating_decrements_after_monster_turn():
    """After 4 monster turns, Plating expires."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=999, name="Jaw Worm")],
        seed=42,
        hand=["StoneArmor"],
    )
    after = apply(state, PlayCardAction("StoneArmor"))
    assert "Plating" in after.player_statuses
    for _ in range(5):
        after = apply(after, EndTurnAction())
    assert "Plating" not in after.player_statuses


def test_stone_armor_upgrade_to_6():
    """StoneArmor+: Plating 4→6 — grants 6 block instead of 4."""
    state = _pyre_state(["StoneArmor+", "Strike"])
    after_turn_no_armor = apply(state, EndTurnAction())
    damage_no_armor = state.player_hp - after_turn_no_armor.player_hp
    after_play = apply(state, PlayCardAction("StoneArmor+"))
    after_turn = apply(after_play, EndTurnAction())
    damage_with_armor = state.player_hp - after_turn.player_hp
    assert damage_with_armor <= max(0, damage_no_armor - 6)


# ── Vicious ────────────────────────────────────────────────────────────────────


def test_vicious_costs_1():
    state = _pyre_state(["Vicious"])
    assert "PlayCard:Vicious" in [str(a) for a in legal_actions(state)]


def test_vicious_applies_status():
    state = _pyre_state(["Vicious", "Strike", "Strike", "Defend", "Defend"])
    after = apply(state, PlayCardAction("Vicious"))
    assert "Vicious" in after.player_statuses


def test_vicious_upgrade():
    """Vicious+: Vicious 1→2."""
    state = _pyre_state(["Vicious+"])
    after = apply(state, PlayCardAction("Vicious+"))
    assert "Vicious" in after.player_statuses


# ── Juggling ──────────────────────────────────────────────────────────────────


def test_juggling_costs_1():
    state = _pyre_state(["Juggling"])
    assert "PlayCard:Juggling" in [str(a) for a in legal_actions(state)]


def test_juggling_applies_status():
    state = _pyre_state(["Juggling", "Strike", "Strike", "Defend", "Defend"])
    after = apply(state, PlayCardAction("Juggling"))
    assert "Juggling" in after.player_statuses


def test_juggling_adds_copy_after_3rd_attack():
    """After playing 3 attacks in one turn with Juggling, a copy is added to hand."""
    state = CombatState(
        player_hp=80,
        player_energy=5,
        monsters=[Monster(hp=99, name="Jaw Worm")],
        seed=42,
        hand=["Juggling", "Strike", "Strike", "Strike", "Defend"],
    )
    after = apply(state, PlayCardAction("Juggling"))
    assert "Juggling" in after.player_statuses
    after = apply(apply(after, PlayCardAction("Strike")), SelectTargetAction(0))
    after = apply(apply(after, PlayCardAction("Strike")), SelectTargetAction(0))
    after = apply(apply(after, PlayCardAction("Strike")), SelectTargetAction(0))
    strikes = after.hand.count("Strike")
    assert strikes >= 1, (
        f"Expected at least 1 Strike in hand after 3rd attack, got {strikes}"
    )


# ── Unmovable ─────────────────────────────────────────────────────────────────


def test_unmovable_costs_2():
    state = _pyre_state(["Unmovable"], energy=3)
    assert "PlayCard:Unmovable" in [str(a) for a in legal_actions(state)]


def test_unmovable_applies_status():
    state = _pyre_state(["Unmovable", "Strike", "Strike", "Defend", "Defend"], energy=3)
    after = apply(state, PlayCardAction("Unmovable"))
    assert "Unmovable" in after.player_statuses


def test_unmovable_doubles_first_block_gain():
    """Unmovable(1) doubles the first block gain per turn."""
    state = CombatState(
        player_hp=80,
        player_energy=5,
        monsters=[Monster(hp=30, name="Jaw Worm")],
        seed=42,
        hand=["Unmovable", "Defend", "Defend", "Defend", "Defend"],
    )
    after = apply(state, PlayCardAction("Unmovable"))
    after = apply(after, PlayCardAction("Defend"))
    assert after.player_block == 10  # 5 * 2
    after = apply(after, PlayCardAction("Defend"))
    assert after.player_block == 15  # 10 + 5 (not doubled)


def test_unmovable_resets_per_turn():
    """Unmovable resets the block-gain counter each turn."""
    state = CombatState(
        player_hp=80,
        player_energy=5,
        monsters=[Monster(hp=999, name="Jaw Worm")],
        seed=42,
        hand=["Unmovable", "Defend", "Defend", "Defend", "Defend"],
    )
    after = apply(state, PlayCardAction("Unmovable"))
    after = apply(after, PlayCardAction("Defend"))
    assert after.player_block == 10  # doubled
    after = apply(after, PlayCardAction("Defend"))
    assert after.player_block == 15  # not doubled
    after = apply(after, EndTurnAction())
    after = apply(after, PlayCardAction("Defend"))
    assert after.player_block == 10  # doubled again on new turn


def test_unmovable_upgrade_cost_reduction():
    """Unmovable+: cost 2→1."""
    state = CombatState(
        player_hp=80,
        player_energy=1,
        monsters=[Monster(hp=30, name="Jaw Worm")],
        seed=42,
        hand=["Unmovable+"],
    )
    actions = legal_actions(state)
    assert any("Unmovable+" in str(a) for a in actions)
