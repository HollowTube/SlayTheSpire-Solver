"""Behavioural tests for Phase 4 Wave 4 Ironclad cards: Corruption's cost/pile
overrides, Cruelty's damage-modifier generalization, Mangle's persistent
Strength debuff, and One Two Punch's turn-scoped double-play."""

from sts_sim import CombatState, Monster, apply


# ── Corruption ───────────────────────────────────────────────────────────────


def test_corruption_makes_skills_free_and_exhausts_them():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["Corruption", "Defend"],
    )

    after_corruption = apply(state, "PlayCard:Corruption")
    # Corruption costs 3, spending all the player's energy.
    assert after_corruption.player_energy == 0

    # Defend (a Skill) costs 1, but Corruption makes Skills free.
    after_defend = apply(after_corruption, "PlayCard:Defend")
    assert after_defend.player_energy == 0
    assert after_defend.player_block == 5
    assert "Defend" in after_defend.exhaust_pile
    assert "Defend" not in after_defend.discard_pile


# ── Cruelty ──────────────────────────────────────────────────────────────────


def test_cruelty_amplifies_vulnerable_damage_to_175x():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, statuses=[("Vulnerable", 1)])],
        seed=42,
        hand=["Cruelty", "Strike"],
    )

    after_cruelty = apply(state, "PlayCard:Cruelty")
    after_strike = apply(after_cruelty, "PlayCard:Strike")
    after_target = apply(after_strike, "SelectTarget:Monster:0")

    # Strike deals 6 base damage; Vulnerable + Cruelty -> floor(6 * 1.75) == 10.
    assert state.monsters[0].hp - after_target.monsters[0].hp == 10


def test_without_cruelty_vulnerable_damage_is_only_15x():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, statuses=[("Vulnerable", 1)])],
        seed=42,
        hand=["Strike"],
    )

    after_strike = apply(state, "PlayCard:Strike")
    after_target = apply(after_strike, "SelectTarget:Monster:0")

    # floor(6 * 1.5) == 9.
    assert state.monsters[0].hp - after_target.monsters[0].hp == 9


# ── Mangle ───────────────────────────────────────────────────────────────────


def test_mangle_deals_damage_and_applies_persistent_minus_10_strength():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=42,
        hand=["Mangle"],
    )

    after_play = apply(state, "PlayCard:Mangle")
    after_target = apply(after_play, "SelectTarget:Monster:0")

    assert state.monsters[0].hp - after_target.monsters[0].hp == 15
    assert after_target.monsters[0].strength == -10

    # Jaw Worm opens with Chomp (11 damage); -10 Strength reduces it to 1.
    after_turn = apply(after_target, "EndTurn")
    assert after_target.player_hp - after_turn.player_hp == 1
    # The debuff isn't removed by the turn's tick_debuffs - it persists.
    assert after_turn.monsters[0].strength == -10


# ── One Two Punch ────────────────────────────────────────────────────────────


def test_one_two_punch_doubles_the_next_attack_played_this_turn():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["OneTwoPunch", "Strike", "Strike"],
    )

    after_otp = apply(state, "PlayCard:OneTwoPunch")
    after_strike = apply(after_otp, "PlayCard:Strike")
    after_target = apply(after_strike, "SelectTarget:Monster:0")

    # Strike deals 6; One Two Punch makes it resolve twice -> 12 total.
    assert state.monsters[0].hp - after_target.monsters[0].hp == 12

    # The power is consumed - a second Strike only hits once.
    after_strike2 = apply(after_target, "PlayCard:Strike")
    after_target2 = apply(after_strike2, "SelectTarget:Monster:0")
    assert after_target.monsters[0].hp - after_target2.monsters[0].hp == 6


def test_one_two_punch_does_not_persist_to_next_turn_if_unused():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["OneTwoPunch"],
    )

    after_otp = apply(state, "PlayCard:OneTwoPunch")
    assert "OneTwoPunch" in after_otp.player_statuses

    after_turn = apply(after_otp, "EndTurn")
    assert "OneTwoPunch" not in after_turn.player_statuses
