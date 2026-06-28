"""Tests for reconstructing an arbitrary mid-fight `CombatState` from its
constituent parts (HP/block/statuses/piles/turn/monster move-history) — the
foundation an external analysis server needs to reconstruct a snapshot of a
real, in-progress fight rather than only ever a fresh turn-0 combat."""

from sts_sim import CombatState, EndTurnAction, Monster, apply, legal_actions


def test_reconstructing_player_block_and_statuses():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
        player_block=5,
        player_statuses=[("Vulnerable", 2), ("Strength", 3)],
    )

    assert state.player_block == 5
    assert state.player_statuses.count("Vulnerable") == 2
    assert state.player_strength == 3
    assert "PlayCard:Strike" in legal_actions(state)


def test_reconstructing_turn_and_card_piles():
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6, name="Jaw Worm")],
        seed=42,
        hand=["Strike"],
        turn=3,
        draw_pile=["Defend", "Defend"],
        discard_pile=["Bash"],
        exhaust_pile=["Slimed"],
    )

    assert state.turn == 3
    assert state.draw_pile == ["Defend", "Defend"]
    assert state.discard_pile == ["Bash"]
    assert state.exhaust_pile == ["Slimed"]


def test_reconstructing_a_monster_mid_cycle_continues_its_real_ai_cycle():
    # Nibbit's fixed cycle is Butt -> Hesitant Slice -> Hiss -> Butt -> ...
    # Reconstruct it as if it had just resolved "Hesitant Slice" and is now
    # telegraphing "Hiss" — ending the turn should advance to "Butt", proving
    # the AI continues the real cycle rather than restarting from the opener.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[
            Monster(hp=44, name="Nibbit", intent="Hiss", last_move="Hesitant Slice")
        ],
        seed=42,
        hand=[],
    )

    assert state.monsters[0].intent == "Hiss"

    after = apply(state, EndTurnAction())

    assert after.monsters[0].intent == "Butt"


def test_reconstructing_a_monster_with_strength_affects_its_next_attack():
    # Jaw Worm's Chomp is a flat 11 damage; with +5 Strength reconstructed
    # onto the monster, ending the turn should deal 16.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[
            Monster(
                hp=44,
                name="Jaw Worm",
                intent="Chomp",
                last_move="Bellow",
                statuses=[("Strength", 5)],
            )
        ],
        seed=42,
        hand=[],
    )

    assert state.monsters[0].strength == 5

    after = apply(state, EndTurnAction())

    assert state.player_hp - after.player_hp == 16
