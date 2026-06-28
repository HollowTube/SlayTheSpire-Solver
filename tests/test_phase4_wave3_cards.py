"""Behavioural tests for Phase 4 Wave 3 Ironclad cards: GameEvent::BlockGained
and GameEvent::DamageReceived, plus the persistent powers that react to them
(Juggernaut, FlameBarrier) and Colossus's cross-side damage modifier."""

from sts_sim import CombatState, EndTurnAction, Monster, PlayCardAction, apply


# ── Juggernaut ───────────────────────────────────────────────────────────────


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
    after_turn_1 = apply(state, EndTurnAction())
    assert state.player_hp - after_turn_1.player_hp == 11
    assert state.monsters[0].hp - after_turn_1.monsters[0].hp == 4

    # FlameBarrier has expired (it lasted only the one turn) - no further
    # retaliation on the next attack.
    after_turn_2 = apply(after_turn_1, EndTurnAction())
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

    after_play = apply(state, PlayCardAction("Colossus"))
    assert after_play.player_block == 5

    after_turn = apply(after_play, EndTurnAction())

    # Jaw Worm's Chomp deals 11; Colossus halves damage from a Vulnerable
    # attacker, rounded down: floor(11 * 0.5) == 5. Colossus's own 5 Block
    # then absorbs that fully, so the player takes no HP loss.
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

    # Jaw Worm's Chomp deals 11, unmodified (Jaw Worm has no Vulnerable);
    # Colossus's 5 Block absorbs 5 of it, leaving 6 HP loss.
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

    # Colossus only lasts for the turn it's played - it decays away during
    # the end-of-turn debuff tick, alongside Block being cleared.
    after_turn_1 = apply(after_play, EndTurnAction())
    assert "Colossus" not in after_turn_1.player_statuses
    assert after_turn_1.player_block == 0
