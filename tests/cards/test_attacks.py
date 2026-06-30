"""Behavioural tests for Ironclad attack cards: cards whose primary effect
is dealing damage to one or more enemies."""

from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
    legal_actions,
)


# ── Bludgeon ──────────────────────────────────────────────────────────────────


def test_bludgeon_deals_32_damage(make_state):
    state = make_state(hand=["Bludgeon"])

    awaiting_target = apply(state, PlayCardAction("Bludgeon"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 32


# ── TwinStrike ────────────────────────────────────────────────────────────────


def test_twin_strike_deals_5_damage_twice(make_state):
    state = make_state(hand=["TwinStrike"])

    awaiting_target = apply(state, PlayCardAction("TwinStrike"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 10


# ── Break ─────────────────────────────────────────────────────────────────────


def test_break_deals_20_damage_and_applies_5_vulnerable(make_state):
    state = make_state(hand=["Break"])

    awaiting_target = apply(state, PlayCardAction("Break"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 20
    assert resolved.monsters[0].statuses.count("Vulnerable") == 5


# ── Uppercut ──────────────────────────────────────────────────────────────────


def test_uppercut_deals_13_damage_and_applies_weak_and_vulnerable(make_state):
    state = make_state(hand=["Uppercut"])

    awaiting_target = apply(state, PlayCardAction("Uppercut"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 13
    assert "Weak" in resolved.monsters[0].statuses
    assert "Vulnerable" in resolved.monsters[0].statuses


# ── Hemokinesis ───────────────────────────────────────────────────────────────


def test_hemokinesis_loses_2_hp_and_deals_15_damage(make_state):
    state = make_state(hand=["Hemokinesis"])

    awaiting_target = apply(state, PlayCardAction("Hemokinesis"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.player_hp - resolved.player_hp == 2
    assert state.monsters[0].hp - resolved.monsters[0].hp == 15


# ── Cinder ────────────────────────────────────────────────────────────────────


def test_cinder_deals_18_damage_and_exhausts_a_random_hand_card(make_state):
    # Cinder costs 2, deals 18 damage to a chosen enemy, then exhausts a
    # random card from hand.
    state = make_state(hand=["Cinder", "Defend"])

    awaiting_target = apply(state, PlayCardAction("Cinder"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 18
    assert resolved.hand == []
    assert "Defend" in resolved.exhaust_pile
    assert "Cinder" in resolved.discard_pile


# ── Thrash ────────────────────────────────────────────────────────────────────


def test_thrash_deals_8_damage_and_exhausts_a_random_attack_from_hand(make_state):
    # Thrash costs 1, deals 4 damage twice (8 total), then exhausts a random
    # Attack card from hand.
    state = make_state(hand=["Thrash", "Strike", "Defend"])

    awaiting_target = apply(state, PlayCardAction("Thrash"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 8
    assert "Strike" in resolved.exhaust_pile
    assert "Defend" in resolved.hand
    assert "Thrash" in resolved.discard_pile


# ── Headbutt ──────────────────────────────────────────────────────────────────


def test_headbutt_deals_9_damage_and_returns_a_discarded_card_to_top_of_draw(
    make_state,
):
    # Headbutt costs 1, deals 9 damage, then returns a card from the discard
    # pile to the top of the draw pile.
    state = make_state(
        hand=["Headbutt"],
        draw_pile=["Strike"],
        discard_pile=["Iron Wave"],
    )

    awaiting_target = apply(state, PlayCardAction("Headbutt"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 9
    assert len(resolved.discard_pile) == 1
    assert sorted(resolved.discard_pile + [resolved.draw_pile[-1]]) == [
        "Headbutt",
        "Iron Wave",
    ]
    assert resolved.draw_pile[0] == "Strike"


# ── FiendFire ─────────────────────────────────────────────────────────────────


def test_fiend_fire_deals_7_per_card_in_hand_and_exhausts_hand_then_itself(make_state):
    # FiendFire costs 2, deals 7 damage per remaining card in hand, then
    # exhausts every card in hand (including itself).
    state = make_state(hand=["FiendFire", "Strike", "Defend"])

    awaiting_target = apply(state, PlayCardAction("FiendFire"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    # 2 cards remaining in hand (Strike, Defend) -> 2 * 7 = 14 damage.
    assert state.monsters[0].hp - resolved.monsters[0].hp == 14
    assert resolved.hand == []
    assert "Strike" in resolved.exhaust_pile
    assert "Defend" in resolved.exhaust_pile
    assert "FiendFire" in resolved.exhaust_pile


# ── InfernalBlade ─────────────────────────────────────────────────────────────


def test_infernal_blade_adds_a_random_attack_to_hand_and_exhausts(make_state):
    # InfernalBlade costs 1, Exhausts, and adds a random Attack from the pool.
    state = make_state(hand=["InfernalBlade"])

    resolved = apply(state, PlayCardAction("InfernalBlade"))

    assert len(resolved.hand) == 1
    assert "InfernalBlade" in resolved.exhaust_pile


# ── BodySlam ──────────────────────────────────────────────────────────────────


def test_body_slam_deals_damage_equal_to_current_block(make_state):
    state = make_state(hand=["BodySlam"], player_block=12)

    awaiting_target = apply(state, PlayCardAction("BodySlam"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 12


# ── PerfectedStrike ───────────────────────────────────────────────────────────


def test_perfected_strike_deals_6_plus_2_per_card_containing_strike_in_deck(make_state):
    # PerfectedStrike counts ALL cards containing "Strike": 1 in hand + 2 in
    # draw pile + PerfectedStrike itself = 4 total -> 6 + 2*4 = 14 damage.
    state = make_state(
        hand=["PerfectedStrike", "Strike"],
        draw_pile=["Strike", "Strike"],
    )

    awaiting_target = apply(state, PlayCardAction("PerfectedStrike"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 14


def test_perfected_strike_counts_cards_with_strike_as_a_substring(make_state):
    # TwinStrike counts because its name contains "Strike".
    state = make_state(
        hand=["PerfectedStrike", "TwinStrike"],
        draw_pile=["Strike", "Strike"],
    )

    awaiting_target = apply(state, PlayCardAction("PerfectedStrike"))
    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 14


# ── AshenStrike ───────────────────────────────────────────────────────────────


def test_ashen_strike_deals_6_plus_3_per_card_in_exhaust_pile(make_state):
    # 2 cards already in exhaust pile -> 6 + 3*2 = 12 damage.
    state = make_state(
        hand=["AshenStrike"],
        exhaust_pile=["Tremble", "Impervious"],
    )

    awaiting_target = apply(state, PlayCardAction("AshenStrike"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 12


# ── Bully ─────────────────────────────────────────────────────────────────────


def test_bully_deals_4_plus_2_per_vulnerable_stack_on_target():
    # Target has 3 stacks of Vulnerable -> base 4 + 2*3 = 10, then amplified
    # 1.5x by Vulnerable -> 15.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6, statuses=[("Vulnerable", 3)])],
        seed=42,
        hand=["Bully"],
    )

    awaiting_target = apply(state, PlayCardAction("Bully"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 15


# ── Conflagration ─────────────────────────────────────────────────────────────


def test_conflagration_deals_8_plus_2_per_attack_played_this_turn_to_all_enemies():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6), Monster(hp=44, attack=6)],
        seed=42,
        hand=["Strike", "Strike", "Conflagration"],
    )

    after_strike_1 = apply(
        apply(state, PlayCardAction("Strike")), SelectTargetAction(0)
    )
    after_strike_2 = apply(
        apply(after_strike_1, PlayCardAction("Strike")), SelectTargetAction(0)
    )

    resolved = apply(after_strike_2, PlayCardAction("Conflagration"))

    # 8 + 2*2 = 12 damage to each enemy.
    assert after_strike_2.monsters[0].hp - resolved.monsters[0].hp == 12
    assert after_strike_2.monsters[1].hp - resolved.monsters[1].hp == 12


# ── TearAsunder ───────────────────────────────────────────────────────────────


def test_tear_asunder_hits_once_per_extra_time_player_was_damaged(make_state):
    # Player hasn't been damaged yet -> 1 + 0 = 1 hit of 5 damage.
    state = make_state(hand=["TearAsunder"])

    awaiting_target = apply(state, PlayCardAction("TearAsunder"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 5


def test_tear_asunder_hits_twice_after_player_was_damaged_once(make_state):
    # Monster attacks once during EndTurn -> player_times_damaged_this_combat
    # becomes 1 -> TearAsunder hits 1 + 1 = 2 times for 5 each = 10.
    state = make_state(hand=["TearAsunder", "Strike", "Strike"])
    after_monster_turn = apply(state, EndTurnAction())
    assert after_monster_turn.player_hp < state.player_hp

    awaiting_target = apply(after_monster_turn, PlayCardAction("TearAsunder"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert after_monster_turn.monsters[0].hp - resolved.monsters[0].hp == 10


# ── Spite ─────────────────────────────────────────────────────────────────────


def test_spite_hits_once_when_no_hp_lost_this_turn(make_state):
    state = make_state(hand=["Spite"])

    awaiting_target = apply(state, PlayCardAction("Spite"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 5


def test_spite_hits_twice_when_hp_lost_this_turn(make_state):
    state = make_state(hand=["Spite", "Strike", "Strike"])
    after_monster_turn = apply(state, EndTurnAction())
    assert after_monster_turn.player_hp < state.player_hp

    awaiting_target = apply(after_monster_turn, PlayCardAction("Spite"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert after_monster_turn.monsters[0].hp - resolved.monsters[0].hp == 10


# ── Dismantle ─────────────────────────────────────────────────────────────────


def test_dismantle_deals_8_damage_once_without_vulnerable(make_state):
    state = make_state(hand=["Dismantle"])

    awaiting_target = apply(state, PlayCardAction("Dismantle"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 8


def test_dismantle_hits_twice_when_target_has_vulnerable():
    # Each 8-damage hit is amplified 1.5x by Vulnerable (floor(8*1.5) = 12),
    # hit twice -> 24.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6, statuses=[("Vulnerable", 1)])],
        seed=42,
        hand=["Dismantle"],
    )

    awaiting_target = apply(state, PlayCardAction("Dismantle"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 24


# ── MoltenFist ────────────────────────────────────────────────────────────────


def test_molten_fist_doubles_targets_vulnerable_then_deals_10_damage():
    # Target starts with 2 Vulnerable stacks -> doubled to 4 before the
    # 10-damage hit, which is then amplified 1.5x -> floor(10*1.5) = 15.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6, statuses=[("Vulnerable", 2)])],
        seed=42,
        hand=["MoltenFist"],
    )

    awaiting_target = apply(state, PlayCardAction("MoltenFist"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 15
    assert resolved.monsters[0].statuses.count("Vulnerable") == 4
    assert "MoltenFist" in resolved.exhaust_pile


# ── Mangle ────────────────────────────────────────────────────────────────────


def test_mangle_deals_damage_and_applies_persistent_minus_10_strength():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=42,
        hand=["Mangle"],
    )

    after_play = apply(state, PlayCardAction("Mangle"))
    after_target = apply(after_play, SelectTargetAction(0))

    assert state.monsters[0].hp - after_target.monsters[0].hp == 15
    assert after_target.monsters[0].strength == -10

    # Jaw Worm opens with Chomp (11 damage); -10 Strength reduces it to 1.
    after_turn = apply(after_target, EndTurnAction())
    assert after_target.player_hp - after_turn.player_hp == 1
    assert after_turn.monsters[0].strength == -10


# ── FightMe! ──────────────────────────────────────────────────────────────────


def _jaw_worm_state(hand, seed=42, energy=3, draw_pile=None):
    return CombatState(
        player_hp=80,
        player_energy=energy,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=seed,
        hand=hand,
        draw_pile=draw_pile or [],
    )


def test_fight_me_deals_5_damage_twice_and_grants_strength():
    state = _jaw_worm_state(["FightMe!", "Strike", "Strike", "Strike", "Defend"])
    after = apply(apply(state, PlayCardAction("FightMe!")), SelectTargetAction(0))
    assert after.monsters[0].hp == 44 - 5 - 5
    assert after.monsters[0].strength == 1
    assert after.player_strength == 3


# ── Anger ─────────────────────────────────────────────────────────────────────


def test_anger_deals_6_damage():
    state = _jaw_worm_state(["Anger", "Strike", "Strike", "Strike", "Defend"])
    after = apply(apply(state, PlayCardAction("Anger")), SelectTargetAction(0))
    assert after.monsters[0].hp == 44 - 6


def test_anger_adds_copy_to_discard():
    state = _jaw_worm_state(["Anger", "Strike", "Strike", "Strike", "Defend"])
    after = apply(apply(state, PlayCardAction("Anger")), SelectTargetAction(0))
    assert after.discard_pile.count("Anger") == 2  # played copy + added copy


def test_anger_costs_0():
    state = _jaw_worm_state(["Anger", "Strike", "Strike", "Strike", "Strike"])
    assert legal_actions(state).count("PlayCard:Anger") == 1


# ── Stomp ─────────────────────────────────────────────────────────────────────


def test_stomp_deals_12_damage_to_all_enemies():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm"), Monster(hp=30, name="Gremlin Nob")],
        hand=["Stomp", "Strike", "Strike", "Strike", "Defend"],
        seed=42,
    )
    after = apply(state, PlayCardAction("Stomp"))
    assert after.monsters[0].hp == 44 - 12
    assert after.monsters[1].hp == 30 - 12


def test_stomp_costs_1_less_per_attack_played():
    state = _jaw_worm_state(["Strike", "Stomp", "Strike", "Strike", "Defend"], energy=2)
    after_strike = apply(apply(state, PlayCardAction("Strike")), SelectTargetAction(0))
    legal = legal_actions(after_strike)
    assert "PlayCard:Stomp" in legal


# ── Breakthrough ──────────────────────────────────────────────────────────────


def test_breakthrough_costs_1_hp_and_hits_all_enemies_for_9_without_a_target(
    make_state,
):
    state = make_state(
        hand=["Breakthrough"],
        monsters=[Monster(hp=44, attack=6), Monster(hp=30, attack=5)],
    )
    resolved = apply(state, PlayCardAction("Breakthrough"))
    assert resolved.pending is None
    assert resolved.player_hp == state.player_hp - 1
    assert resolved.monsters[0].hp == state.monsters[0].hp - 9
    assert resolved.monsters[1].hp == state.monsters[1].hp - 9


# ── Setup Strike ──────────────────────────────────────────────────────────────


def test_setup_strike_deals_7_and_grants_2_strength_this_turn_only(make_state):
    state = make_state(hand=["Setup Strike", "Strike"])
    after_setup = apply(
        apply(state, PlayCardAction("Setup Strike")), SelectTargetAction(0)
    )
    assert after_setup.monsters[0].hp == state.monsters[0].hp - 7
    assert "StrengthThisTurn" in after_setup.player_statuses

    after_strike = apply(
        apply(after_setup, PlayCardAction("Strike")), SelectTargetAction(0)
    )
    # Strike (6) + 2 from StrengthThisTurn = 8.
    assert after_strike.monsters[0].hp == after_setup.monsters[0].hp - 8

    after_turn = apply(after_strike, EndTurnAction())
    assert "StrengthThisTurn" not in after_turn.player_statuses


# ── Unrelenting ───────────────────────────────────────────────────────────────


def test_unrelenting_deals_12_and_makes_next_attack_free(make_state):
    state = make_state(hand=["Unrelenting", "Strike"], player_energy=1)
    after_unrelenting = apply(
        apply(state, PlayCardAction("Unrelenting")), SelectTargetAction(0)
    )
    assert after_unrelenting.monsters[0].hp == state.monsters[0].hp - 12
    assert "FreeAttack" in after_unrelenting.player_statuses
    assert after_unrelenting.player_energy == 0

    # Strike normally costs 1, but FreeAttack makes it cost 0.
    assert "PlayCard:Strike" in legal_actions(after_unrelenting)
    after_strike = apply(
        apply(after_unrelenting, PlayCardAction("Strike")), SelectTargetAction(0)
    )
    assert after_strike.player_energy == 0
    assert "FreeAttack" not in after_strike.player_statuses
