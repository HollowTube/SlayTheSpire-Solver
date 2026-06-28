"""Behavioural tests for Phase 4 Wave 1 Ironclad cards: GameEvent::TurnStart
and the persistent powers that react to it."""

from sts_sim import CombatState, EndTurnAction, Monster, PlayCardAction, apply


def make_state(hand=("Strike",), seed=42, **kwargs):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=6)],
        seed=seed,
        hand=list(hand),
        **kwargs,
    )


# ── DemonForm ────────────────────────────────────────────────────────────────


def test_demon_form_grants_2_strength_at_start_of_each_turn():
    state = make_state(hand=["DemonForm"])

    after_play = apply(state, PlayCardAction("DemonForm"))
    assert after_play.player_strength == 0

    after_turn_1 = apply(after_play, EndTurnAction())
    assert after_turn_1.player_strength == 2

    after_turn_2 = apply(after_turn_1, EndTurnAction())
    assert after_turn_2.player_strength == 4


# ── CrimsonMantle ────────────────────────────────────────────────────────────


def test_crimson_mantle_gains_block_and_increasing_self_damage_each_turn():
    # Chosen-not-wiki numbers: 8 block + self-damage starting at 1, +1/turn.
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


# ── Inferno ──────────────────────────────────────────────────────────────────


def test_inferno_self_damage_at_turn_start_triggers_aoe_retaliation():
    # At the start of each turn, Inferno's holder loses 1 unblockable HP;
    # that HP loss in turn triggers 6 damage to ALL enemies. attack=0 so the
    # monsters' swings don't pollute the HP deltas.
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

    # The effect repeats identically every turn (no escalation).
    after_turn_2 = apply(after_turn_1, EndTurnAction())
    assert after_turn_1.player_hp - after_turn_2.player_hp == 1
    assert after_turn_1.monsters[0].hp - after_turn_2.monsters[0].hp == 6


# ── Aggression ───────────────────────────────────────────────────────────────


def test_aggression_returns_a_discarded_attack_to_hand_at_turn_start():
    # Only one Attack ("Strike") is in the discard pile alongside a non-Attack
    # ("Defend") -> Aggression must return the Attack, not the Skill. The
    # draw pile is pre-filled with the turn's opening hand so the normal
    # turn-start draw doesn't reshuffle (and thus drain) the discard pile
    # before Aggression's reaction runs.
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
