"""Behavioural tests for Slithering Strangler (Overgrowth elite) and
Status::Constrict (persistent end-of-turn unblockable self-damage)."""

from sts_sim import CombatState, Monster, apply


# ── Status::Constrict ─────────────────────────────────────────────────────────


def test_constrict_deals_unblockable_end_of_turn_damage():
    """Constrict(3) deals exactly 3 unblockable damage at EndTurn."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, attack=0)],
        seed=42,
        hand=[],
        player_statuses=[("Constrict", 3)],
    )
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 3


def test_constrict_damage_ignores_block():
    """Constrict damage bypasses block entirely."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, attack=0)],
        seed=42,
        hand=["Defend"],
        player_block=50,
        player_statuses=[("Constrict", 5)],
    )
    after = apply(state, "EndTurn")
    # 5 Constrict damage, bypassing 50 block
    assert state.player_hp - after.player_hp == 5
    # Block from Defend should still be there (or be gone from EndTurn reset)
    # — Constrict hit first, then block resets.


def test_constrict_does_not_decay():
    """Constrict persists across turns (no decay)."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, attack=0)],
        seed=42,
        hand=[],
        player_statuses=[("Constrict", 3)],
    )
    a1 = apply(state, "EndTurn")
    # Constrict deals damage but doesn't decay
    assert a1.player_hp == 77
    assert "Constrict" in a1.player_statuses

    a2 = apply(a1, "EndTurn")
    assert a2.player_hp == 74
    assert "Constrict" in a2.player_statuses


def test_constrict_stacks_accumulate():
    """Multiple Constrict applications sum their stack counts."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, attack=0)],
        seed=42,
        hand=[],
        player_statuses=[("Constrict", 3), ("Constrict", 2)],
    )
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 5  # 3 + 2 = 5


def test_no_constrict_no_damage():
    """Without Constrict, EndTurn deals no self-damage."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, attack=0)],
        seed=42,
        hand=[],
    )
    after = apply(state, "EndTurn")
    assert after.player_hp == 80  # no constrict, no damage


# ── Slithering Strangler move pool ───────────────────────────────────────────


def _strangler(seed=42):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=54, name="Slithering Strangler")],
        seed=seed,
        hand=[],
    )


def test_strangler_opens_with_constrict():
    state = _strangler()
    assert state.monsters[0].intent == "Constrict"


def test_constrict_applies_3_stacks_to_player():
    state = _strangler()
    after = apply(state, "EndTurn")
    # EndTurn deals damage from Constrict immediately (after monster turn).
    # 3 + 0 = 3 damage from Constrict at end of turn.
    assert state.player_hp - after.player_hp == 3


def test_strangler_after_constrict_random_branch():
    """After Constrict, the next move is Thwack or Lash (random)."""
    state = _strangler()
    a1 = apply(state, "EndTurn")  # Constrict
    assert a1.monsters[0].intent in {"Thwack", "Lash"}


def test_thwack_deals_7_damage_and_grants_5_block():
    """Thwack: 7 damage to player, 5 block to self."""
    # seed=1 gives Constrict → Thwack
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=54, name="Slithering Strangler")],
        seed=1,
        hand=[],
    )
    a1 = apply(state, "EndTurn")  # Constrict
    assert a1.monsters[0].intent == "Thwack"
    after = apply(a1, "EndTurn")
    # Thwack deals 7 + Constrict 3 end-of-turn = 10
    assert a1.player_hp - after.player_hp == 10
    assert after.monsters[0].block >= 5


def test_lash_deals_12_damage():
    """Lash: 12 damage to player."""
    # seed=0 gives Constrict → Lash
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=54, name="Slithering Strangler")],
        seed=0,
        hand=[],
    )
    a1 = apply(state, "EndTurn")  # Constrict
    assert a1.monsters[0].intent == "Lash"
    after = apply(a1, "EndTurn")
    # Lash deals 12 + Constrict 3 = 15
    assert a1.player_hp - after.player_hp == 15


def test_cycle_returns_to_constrict():
    """Both Thwack and Lash lead back to Constrict."""
    state = _strangler()
    a1 = apply(state, "EndTurn")  # Constrict
    a2 = apply(a1, "EndTurn")  # Thwack or Lash → back to Constrict
    assert a2.monsters[0].intent == "Constrict"
