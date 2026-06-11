"""Behavioural tests for Phase 3 Ironclad cards: damage/block scaled off
game state (DynamicVar-style)."""

from sts_sim import CombatState, Monster, apply


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

    awaiting_target = apply(state, "PlayCard:BodySlam")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 12


# ── PerfectedStrike ──────────────────────────────────────────────────────────


def test_perfected_strike_deals_6_plus_2_per_strike_in_deck():
    # 1 Strike in hand (PerfectedStrike doesn't count itself) + 2 Strikes in
    # the draw pile = 3 Strikes total -> 6 + 2*3 = 12 damage.
    state = make_state(
        hand=["PerfectedStrike", "Strike"],
        draw_pile=["Strike", "Strike"],
    )

    awaiting_target = apply(state, "PlayCard:PerfectedStrike")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 12


# ── AshenStrike ──────────────────────────────────────────────────────────────


def test_ashen_strike_deals_6_plus_3_per_card_in_exhaust_pile():
    # 2 cards already in exhaust pile -> 6 + 3*2 = 12 damage.
    state = make_state(
        hand=["AshenStrike"],
        exhaust_pile=["Tremble", "Impervious"],
    )

    awaiting_target = apply(state, "PlayCard:AshenStrike")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

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

    awaiting_target = apply(state, "PlayCard:Bully")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

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
    after_strike_1 = apply(apply(state, "PlayCard:Strike"), "SelectTarget:Monster:0")
    after_strike_2 = apply(apply(after_strike_1, "PlayCard:Strike"), "SelectTarget:Monster:0")

    resolved = apply(after_strike_2, "PlayCard:Conflagration")

    # 8 + 2*2 = 12 damage to each enemy (Conflagration is non-targeted/AoE).
    assert after_strike_2.monsters[0].hp - resolved.monsters[0].hp == 12
    assert after_strike_2.monsters[1].hp - resolved.monsters[1].hp == 12


# ── TearAsunder ──────────────────────────────────────────────────────────────


def test_tear_asunder_hits_once_per_extra_time_player_was_damaged():
    # Player hasn't been damaged yet -> 1 + 0 = 1 hit of 6 damage.
    state = make_state(hand=["TearAsunder"])

    awaiting_target = apply(state, "PlayCard:TearAsunder")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 6


def test_tear_asunder_hits_twice_after_player_was_damaged_once():
    # Monster attacks once during EndTurn -> player_times_damaged_this_combat
    # becomes 1 -> TearAsunder hits 1 + 1 = 2 times for 6 each = 12.
    state = make_state(hand=["TearAsunder", "Strike", "Strike"])
    after_monster_turn = apply(state, "EndTurn")
    assert after_monster_turn.player_hp < state.player_hp

    awaiting_target = apply(after_monster_turn, "PlayCard:TearAsunder")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert after_monster_turn.monsters[0].hp - resolved.monsters[0].hp == 12


# ── Spite ────────────────────────────────────────────────────────────────────


def test_spite_hits_once_when_no_hp_lost_this_turn():
    state = make_state(hand=["Spite"])

    awaiting_target = apply(state, "PlayCard:Spite")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 3


def test_spite_hits_twice_when_hp_lost_this_turn():
    state = make_state(hand=["Spite", "Strike", "Strike"])
    after_monster_turn = apply(state, "EndTurn")
    assert after_monster_turn.player_hp < state.player_hp

    awaiting_target = apply(after_monster_turn, "PlayCard:Spite")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert after_monster_turn.monsters[0].hp - resolved.monsters[0].hp == 6


# ── Dismantle ────────────────────────────────────────────────────────────────


def test_dismantle_deals_8_damage_once_without_vulnerable():
    state = make_state(hand=["Dismantle"])

    awaiting_target = apply(state, "PlayCard:Dismantle")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

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

    awaiting_target = apply(state, "PlayCard:Dismantle")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

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

    awaiting_target = apply(state, "PlayCard:MoltenFist")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - resolved.monsters[0].hp == 15
    assert resolved.monsters[0].statuses.count("Vulnerable") == 4


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

    awaiting_target = apply(state, "PlayCard:Dominate")
    assert awaiting_target.pending == "SelectTarget"

    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert resolved.monsters[0].statuses.count("Vulnerable") == 2
    assert resolved.player_strength == 2
    assert "Dominate" in resolved.exhaust_pile
