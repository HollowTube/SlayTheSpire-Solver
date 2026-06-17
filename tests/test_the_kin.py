"""Behavioural tests for The Kin (Overgrowth boss: Kin Priest + 2x Kin Follower),
including Status::Frail (-25% Block) and Status::Minion (flee-on-leader-death)."""

from sts_sim import CombatState, Monster, apply


# ── Status::Frail ─────────────────────────────────────────────────────────────


def test_frail_reduces_block_gained_by_25_percent():
    """Frail(1) reduces a Defend (5 block) to 3 (5*0.75=3.75, floor=3)."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, attack=0)],
        seed=42,
        hand=["Defend"],
        player_statuses=[("Frail", 1)],
    )
    after = apply(state, "PlayCard:Defend")
    assert after.player_block == 3


def test_frail_reduces_block_gained_on_gain_block_scaled_too():
    """Block modifiers apply to GainBlockScaled ops (Evil Eye)."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, attack=0)],
        seed=42,
        hand=["Evil Eye"],
        player_statuses=[("Frail", 1)],
    )
    after = apply(state, "PlayCard:Evil Eye")
    assert after.player_block == 6  # 8 * 0.75 = 6.0, floor = 6


def test_frail_is_binary_debuff_multiple_stacks_dont_compound():
    """Multiple Frail stacks apply the modifier once (binary debuff)."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, attack=0)],
        seed=42,
        hand=["Defend"],
        player_statuses=[("Frail", 3)],
    )
    after = apply(state, "PlayCard:Defend")
    assert after.player_block == 3  # still 3, not 5 * 0.75^3


def test_frail_decays_at_end_of_monster_turn():
    """Frail on a monster is removed by tick_debuffs after its turn."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, name="Kin Follower", statuses=[("Frail", 1)])],
        seed=42,
        hand=[],
    )
    assert "Frail" in state.monsters[0].statuses
    after = apply(state, "EndTurn")
    assert "Frail" not in after.monsters[0].statuses


def test_frail_on_player_decays_at_end_of_player_turn():
    """Player Frail decays at end of player turn (via tick_debuffs)."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=50, attack=0)],
        seed=42,
        hand=[],
        player_statuses=[("Frail", 1)],
    )
    assert "Frail" in state.player_statuses
    after = apply(state, "EndTurn")
    assert "Frail" not in after.player_statuses


# ── Status::Minion ────────────────────────────────────────────────────────────


def test_minion_acts_normally_when_leader_is_alive():
    """Follower with Minion still acts when Kin Priest is alive."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[
            Monster(hp=190, name="Kin Priest"),
            Monster(
                hp=59,
                name="Kin Follower",
                intent="Quick Slash",
                statuses=[("Minion", 0)],
            ),
            Monster(
                hp=59,
                name="Kin Follower",
                intent="Power Dance",
                statuses=[("Minion", 0)],
            ),
        ],
        seed=42,
        hand=[],
    )
    assert state.monsters[1].intent == "Quick Slash"
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp >= 5


def test_minion_skips_turn_when_leader_is_dead():
    """Follower flees (skips turn) once Kin Priest's HP <= 0."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[
            Monster(hp=0, name="Kin Priest"),
            Monster(
                hp=59,
                name="Kin Follower",
                intent="Quick Slash",
                statuses=[("Minion", 0)],
            ),
            Monster(
                hp=59,
                name="Kin Follower",
                intent="Power Dance",
                statuses=[("Minion", 0)],
            ),
        ],
        seed=42,
        hand=[],
    )
    after = apply(state, "EndTurn")
    # Both followers should have skipped since priest is dead: no damage.
    assert after.player_hp == state.player_hp == 80


def test_only_specific_leader_death_triggers_minion_flee():
    """A minion doesn't flee if a non-leader monster dies."""
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[
            Monster(hp=190, name="Kin Priest"),
            Monster(
                hp=0,
                name="Kin Follower",
                intent="Quick Slash",
                statuses=[("Minion", 0)],
            ),
            Monster(
                hp=59,
                name="Kin Follower",
                intent="Power Dance",
                statuses=[("Minion", 0)],
            ),
        ],
        seed=42,
        hand=[],
    )
    after = apply(state, "EndTurn")
    # Follower 2 is alive, leader is alive, so at least some damage happened.
    assert after.player_hp < state.player_hp


# ── Kin Priest move pool / fixed cycle ────────────────────────────────────────


def _kin_priest(seed=42):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=190, name="Kin Priest")],
        seed=seed,
        hand=[],
    )


def test_kin_priest_opens_with_orb_of_frailty():
    state = _kin_priest()
    assert state.monsters[0].intent == "Orb of Frailty"


def test_orb_of_frailty_deals_8_damage():
    state = _kin_priest()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 8


def test_orb_of_weakness_deals_8_damage():
    state = _kin_priest()
    a1 = apply(state, "EndTurn")  # Orb of Frailty
    assert a1.monsters[0].intent == "Orb of Weakness"
    a2 = apply(a1, "EndTurn")
    assert a1.player_hp - a2.player_hp == 8


def test_soul_beam_deals_3_damage_three_times():
    state = _kin_priest()
    a1 = apply(state, "EndTurn")  # Orb of Frailty
    a2 = apply(a1, "EndTurn")  # Orb of Weakness
    assert a2.monsters[0].intent == "Soul Beam"
    a3 = apply(a2, "EndTurn")
    assert a2.player_hp - a3.player_hp == 9


def test_dark_ritual_grants_2_strength():
    state = _kin_priest()
    a1 = apply(state, "EndTurn")  # Orb of Frailty
    a2 = apply(a1, "EndTurn")  # Orb of Weakness
    a3 = apply(a2, "EndTurn")  # Soul Beam
    assert a3.monsters[0].intent == "Dark Ritual"
    a4 = apply(a3, "EndTurn")
    assert a4.monsters[0].strength == 2


def test_kin_priest_cycle_repeats():
    """After Dark Ritual, the cycle returns to Orb of Frailty."""
    state = _kin_priest()
    a1 = apply(state, "EndTurn")  # Orb of Frailty
    a2 = apply(a1, "EndTurn")  # Orb of Weakness
    a3 = apply(a2, "EndTurn")  # Soul Beam
    a4 = apply(a3, "EndTurn")  # Dark Ritual
    assert a4.monsters[0].intent == "Orb of Frailty"


# ── Kin Follower move pool / fixed cycle ──────────────────────────────────────


def _kin_follower(seed=42, intent=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=59, name="Kin Follower", intent=intent)],
        seed=seed,
        hand=[],
    )


def test_kin_follower_opens_with_quick_slash():
    state = _kin_follower()
    assert state.monsters[0].intent == "Quick Slash"


def test_quick_slash_deals_5_damage():
    state = _kin_follower()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 5


def test_boomerang_deals_2_damage_twice():
    state = _kin_follower()
    a1 = apply(state, "EndTurn")  # Quick Slash
    assert a1.monsters[0].intent == "Boomerang"
    a2 = apply(a1, "EndTurn")
    assert a1.player_hp - a2.player_hp == 4


def test_power_dance_grants_2_strength():
    state = _kin_follower()
    a1 = apply(state, "EndTurn")  # Quick Slash
    a2 = apply(a1, "EndTurn")  # Boomerang
    assert a2.monsters[0].intent == "Power Dance"
    a3 = apply(a2, "EndTurn")
    assert a3.monsters[0].strength == 2


def test_kin_follower_cycle_repeats():
    """After Power Dance, the cycle returns to Quick Slash."""
    state = _kin_follower()
    a1 = apply(state, "EndTurn")  # Quick Slash
    a2 = apply(a1, "EndTurn")  # Boomerang
    a3 = apply(a2, "EndTurn")  # Power Dance
    assert a3.monsters[0].intent == "Quick Slash"
