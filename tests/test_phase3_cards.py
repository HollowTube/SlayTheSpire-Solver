"""Behavioural tests for Phase 3 Ironclad cards: damage/block scaled off
game state (DynamicVar-style)."""

from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
)


def make_state(hand=("Strike",), seed=42, player_block=0, **kwargs):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=seed,
        hand=list(hand),
        player_block=player_block,
        **kwargs,
    )


# ── BodySlam ─────────────────────────────────────────────────────────────────


def test_body_slam_deals_damage_equal_to_current_block():
    state = make_state(hand=["BodySlam"], player_block=12)

    awaiting_target = apply(state, PlayCardAction("BodySlam"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 12


# ── PerfectedStrike ──────────────────────────────────────────────────────────


def test_perfected_strike_deals_6_plus_2_per_card_containing_strike_in_deck():
    # PerfectedStrike counts ALL cards containing "Strike" in the deck,
    # including itself (now in the discard pile after being played): 1
    # Strike in hand + 2 Strikes in the draw pile + PerfectedStrike itself
    # = 4 total -> 6 + 2*4 = 14 damage.
    state = make_state(
        hand=["PerfectedStrike", "Strike"],
        draw_pile=["Strike", "Strike"],
    )

    awaiting_target = apply(state, PlayCardAction("PerfectedStrike"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 14


def test_perfected_strike_counts_cards_with_strike_as_a_substring():
    # TwinStrike contains "Strike" even though its name isn't exactly
    # "Strike" - it counts the same as a plain Strike: 1 TwinStrike in hand +
    # 2 Strikes in the draw pile + PerfectedStrike itself = 4 total ->
    # 6 + 2*4 = 14 damage.
    state = make_state(
        hand=["PerfectedStrike", "TwinStrike"],
        draw_pile=["Strike", "Strike"],
    )

    awaiting_target = apply(state, PlayCardAction("PerfectedStrike"))
    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 14


# ── AshenStrike ──────────────────────────────────────────────────────────────


def test_ashen_strike_deals_6_plus_3_per_card_in_exhaust_pile():
    # 2 cards already in exhaust pile -> 6 + 3*2 = 12 damage.
    state = make_state(
        hand=["AshenStrike"],
        exhaust_pile=["Tremble", "Impervious"],
    )

    awaiting_target = apply(state, PlayCardAction("AshenStrike"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 12


# ── Bully ────────────────────────────────────────────────────────────────────


def test_bully_deals_4_plus_2_per_vulnerable_stack_on_target():
    # Target has 3 stacks of Vulnerable -> base damage 4 + 2*3 = 10, then
    # the target's own Vulnerable amplifies it by 1.5x -> 15.
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


# ── Conflagration ────────────────────────────────────────────────────────────


def test_conflagration_deals_8_plus_2_per_attack_played_this_turn_to_all_enemies():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6), Monster(hp=44, attack=6)],
        seed=42,
        hand=["Strike", "Strike", "Conflagration"],
    )

    # Two Strikes played first -> Conflagration sees attacks_played_this_turn == 2.
    after_strike_1 = apply(
        apply(state, PlayCardAction("Strike")), SelectTargetAction(0)
    )
    after_strike_2 = apply(
        apply(after_strike_1, PlayCardAction("Strike")), SelectTargetAction(0)
    )

    resolved = apply(after_strike_2, PlayCardAction("Conflagration"))

    # 8 + 2*2 = 12 damage to each enemy (Conflagration is non-targeted/AoE).
    assert after_strike_2.monsters[0].hp - resolved.monsters[0].hp == 12
    assert after_strike_2.monsters[1].hp - resolved.monsters[1].hp == 12


# ── TearAsunder ──────────────────────────────────────────────────────────────


def test_tear_asunder_hits_once_per_extra_time_player_was_damaged():
    # Player hasn't been damaged yet -> 1 + 0 = 1 hit of 5 damage.
    state = make_state(hand=["TearAsunder"])

    awaiting_target = apply(state, PlayCardAction("TearAsunder"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 5


def test_tear_asunder_hits_twice_after_player_was_damaged_once():
    # Monster attacks once during EndTurn -> player_times_damaged_this_combat
    # becomes 1 -> TearAsunder hits 1 + 1 = 2 times for 5 each = 10.
    state = make_state(hand=["TearAsunder", "Strike", "Strike"])
    after_monster_turn = apply(state, EndTurnAction())
    assert after_monster_turn.player_hp < state.player_hp

    awaiting_target = apply(after_monster_turn, PlayCardAction("TearAsunder"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert after_monster_turn.monsters[0].hp - resolved.monsters[0].hp == 10


# ── Spite ────────────────────────────────────────────────────────────────────


def test_spite_hits_once_when_no_hp_lost_this_turn():
    state = make_state(hand=["Spite"])

    awaiting_target = apply(state, PlayCardAction("Spite"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 5


def test_spite_hits_twice_when_hp_lost_this_turn():
    state = make_state(hand=["Spite", "Strike", "Strike"])
    after_monster_turn = apply(state, EndTurnAction())
    assert after_monster_turn.player_hp < state.player_hp

    awaiting_target = apply(after_monster_turn, PlayCardAction("Spite"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert after_monster_turn.monsters[0].hp - resolved.monsters[0].hp == 10


# ── Dismantle ────────────────────────────────────────────────────────────────


def test_dismantle_deals_8_damage_once_without_vulnerable():
    state = make_state(hand=["Dismantle"])

    awaiting_target = apply(state, PlayCardAction("Dismantle"))
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert state.monsters[0].hp - resolved.monsters[0].hp == 8


def test_dismantle_hits_twice_when_target_has_vulnerable():
    # Each 8-damage hit is amplified 1.5x by the target's Vulnerable
    # (floor(8*1.5) = 12), hit twice -> 24.
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


# ── MoltenFist ───────────────────────────────────────────────────────────────


def test_molten_fist_doubles_targets_vulnerable_then_deals_10_damage():
    # Target starts with 2 Vulnerable stacks -> doubled to 4 *before* the
    # 10-damage hit, which is then amplified 1.5x by Vulnerable
    # (floor(10*1.5) = 15).
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
    # MoltenFist Exhausts after being played.
    assert "MoltenFist" in resolved.exhaust_pile


# ── Dominate ─────────────────────────────────────────────────────────────────


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
