"""Behavioural tests for Phase 4 Wave 3 Ironclad cards: GameEvent::BlockGained
and GameEvent::DamageReceived, plus the persistent powers that react to them
(Juggernaut, FlameBarrier) and Colossus's cross-side damage modifier."""

from sts_sim import CombatState, Monster, apply


# ── Juggernaut ───────────────────────────────────────────────────────────────


def test_juggernaut_deals_damage_to_enemy_when_block_is_gained():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0)],
        seed=42,
        hand=["Juggernaut", "ShrugItOff"],
    )

    after_juggernaut = apply(state, "PlayCard:Juggernaut")
    after_shrug = apply(after_juggernaut, "PlayCard:ShrugItOff")

    assert after_shrug.player_block == 8
    assert state.monsters[0].hp - after_shrug.monsters[0].hp == 5


# ── FlameBarrier ─────────────────────────────────────────────────────────────


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
    after_turn_1 = apply(state, "EndTurn")
    assert state.player_hp - after_turn_1.player_hp == 11
    assert state.monsters[0].hp - after_turn_1.monsters[0].hp == 4

    # FlameBarrier has expired (it lasted only the one turn) - no further
    # retaliation on the next attack.
    after_turn_2 = apply(after_turn_1, "EndTurn")
    assert after_turn_1.monsters[0].hp == after_turn_2.monsters[0].hp


# ── Colossus ─────────────────────────────────────────────────────────────────


def test_colossus_halves_damage_from_a_vulnerable_attacker():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm", statuses=[("Vulnerable", 1)])],
        seed=42,
        hand=["Colossus"],
    )

    after_play = apply(state, "PlayCard:Colossus")
    after_turn = apply(after_play, "EndTurn")

    # Jaw Worm's Chomp deals 11; Colossus halves damage from a Vulnerable
    # attacker, rounded down: floor(11 * 0.5) == 5.
    assert after_play.player_hp - after_turn.player_hp == 5


def test_colossus_does_not_reduce_damage_from_a_non_vulnerable_attacker():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, name="Jaw Worm")],
        seed=42,
        hand=["Colossus"],
    )

    after_play = apply(state, "PlayCard:Colossus")
    after_turn = apply(after_play, "EndTurn")

    assert after_play.player_hp - after_turn.player_hp == 11
