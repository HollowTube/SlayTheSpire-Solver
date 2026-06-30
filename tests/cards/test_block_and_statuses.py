"""Behavioural tests for the block system and status effects: Vulnerable,
Weak, Strength, Enrage, and their interactions with attacks and turns."""

import pytest
from sts_sim import (
    CombatState,
    EndTurnAction,
    Monster,
    PlayCardAction,
    SelectTargetAction,
    apply,
)


# Per the Slay the Spire wiki, base (un-upgraded) Defend grants 5 block.
DEFEND_BLOCK = 5


def test_playing_defend_grants_the_documented_block_without_asking_for_a_target(
    make_state,
):
    state = make_state(hand=["Defend"])

    resolved = apply(state, PlayCardAction("Defend"))

    assert resolved.player_block == state.player_block + DEFEND_BLOCK
    assert resolved.pending is None


def test_block_fully_absorbs_an_attack_no_larger_than_it(make_state):
    # Defend grants 5 block; the toy monster hits for 6. Use two Defends
    # (10 block total) to comfortably absorb the 6 damage attack.
    state = make_state(hand=["Defend", "Defend"])
    once = apply(state, PlayCardAction("Defend"))
    blocked = apply(once, PlayCardAction("Defend"))

    next_state = apply(blocked, EndTurnAction())

    assert next_state.player_hp == blocked.player_hp


def test_block_smaller_than_the_attack_only_partially_absorbs_it(make_state):
    # Defend grants 5 block; the toy monster hits for 6, so 1 damage gets through.
    state = make_state(hand=["Defend"])
    blocked = apply(state, PlayCardAction("Defend"))

    next_state = apply(blocked, EndTurnAction())

    assert next_state.player_hp == blocked.player_hp - 1


def test_block_does_not_carry_over_into_the_next_turn(make_state):
    state = make_state(hand=["Defend"])
    blocked = apply(state, PlayCardAction("Defend"))

    after_one_turn = apply(blocked, EndTurnAction())

    assert after_one_turn.player_block == 0


def test_playing_bash_against_the_monster_leaves_it_vulnerable(make_state):
    state = make_state(hand=["Bash"])
    awaiting_target = apply(state, PlayCardAction("Bash"))

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert "Vulnerable" in resolved.monsters[0].statuses


def test_monster_vulnerable_decays_by_one_per_turn(make_state):
    # Bash applies 2 Vulnerable stacks. After one EndTurn one stack decays,
    # after a second EndTurn the last stack decays.
    state = make_state(hand=["Bash"])
    with_vulnerable = apply(apply(state, PlayCardAction("Bash")), SelectTargetAction(0))
    assert with_vulnerable.monsters[0].statuses.count("Vulnerable") == 2

    after_one_turn = apply(with_vulnerable, EndTurnAction())
    assert after_one_turn.monsters[0].statuses.count("Vulnerable") == 1

    after_two_turns = apply(after_one_turn, EndTurnAction())
    assert after_two_turns.monsters[0].statuses.count("Vulnerable") == 0


def test_player_vulnerable_from_skull_bash_decays_by_one_per_turn():
    # Gremlin Nob's Skull Bash applies 2 Vulnerable. The tick fires at the
    # start of the player's turn, so after Skull Bash the player enters the
    # next turn with 1 (2 applied, 1 ticked). One more turn: 1 → 0.
    for seed in range(200):
        state = _gremlin_nob(seed)
        after_bellow = apply(state, EndTurnAction())
        if after_bellow.monsters[0].intent == "Skull Bash":
            after_skull_bash = apply(after_bellow, EndTurnAction())
            assert after_skull_bash.player_statuses.count("Vulnerable") == 1
            after_next_turn = apply(after_skull_bash, EndTurnAction())
            assert after_next_turn.player_statuses.count("Vulnerable") == 0
            return
    pytest.fail("Could not find a seed where Skull Bash fires as the second move")


# Per the Slay the Spire wiki, Vulnerable increases damage taken by 50%,
# rounded down: floor(STRIKE_DAMAGE * 1.5) == 9.
VULNERABLE_STRIKE_DAMAGE = 9

# Per the Slay the Spire wiki, base (un-upgraded) Iron Wave deals 5 damage
# and grants 5 block.
IRON_WAVE_DAMAGE = 5
IRON_WAVE_BLOCK = 5


def test_playing_iron_wave_deals_damage_and_grants_block_from_a_single_card(make_state):
    state = make_state(hand=["Iron Wave"])
    awaiting_target = apply(state, PlayCardAction("Iron Wave"))

    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert resolved.monsters[0].hp == state.monsters[0].hp - IRON_WAVE_DAMAGE
    assert resolved.player_block == state.player_block + IRON_WAVE_BLOCK


# Per the Slay the Spire wiki, base (un-upgraded) Inflame grants 2 Strength.
INFLAME_STRENGTH = 2
STRIKE_DAMAGE_WITH_INFLAME = 6 + INFLAME_STRENGTH


def test_playing_inflame_grants_the_player_strength_without_asking_for_a_target(
    make_state,
):
    state = make_state(hand=["Inflame"])

    resolved = apply(state, PlayCardAction("Inflame"))

    assert "Strength" in resolved.player_statuses
    assert resolved.pending is None


def test_strength_increases_the_damage_dealt_by_a_subsequent_strike(make_state):
    state = make_state(hand=["Inflame", "Strike"])
    strengthened = apply(state, PlayCardAction("Inflame"))

    awaiting_target = apply(strengthened, PlayCardAction("Strike"))
    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert (
        resolved.monsters[0].hp
        == strengthened.monsters[0].hp - STRIKE_DAMAGE_WITH_INFLAME
    )


def test_a_vulnerable_monster_takes_amplified_damage_from_a_subsequent_strike(
    make_state,
):
    # Bash (8 dmg, applies Vulnerable) then Strike — amplification from Vulnerable.
    state = make_state(hand=["Bash", "Strike"])
    vulnerable = apply(apply(state, PlayCardAction("Bash")), SelectTargetAction(0))
    hp_before_strike = vulnerable.monsters[0].hp

    awaiting_target = apply(vulnerable, PlayCardAction("Strike"))
    resolved = apply(awaiting_target, SelectTargetAction(0))

    assert resolved.monsters[0].hp == hp_before_strike - VULNERABLE_STRIKE_DAMAGE


def _gremlin_nob(seed, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=85, attack=0, name="Gremlin Nob")],
        seed=seed,
        hand=hand or [],
        deck=deck,
    )


def test_gremlin_nob_skull_bash_applies_two_vulnerable_to_the_player():
    for seed in range(200):
        state = _gremlin_nob(seed)
        after_bellow = apply(state, EndTurnAction())
        if after_bellow.monsters[0].intent == "Skull Bash":
            after_skull_bash = apply(after_bellow, EndTurnAction())
            assert after_skull_bash.player_statuses.count("Vulnerable") == 1
            after_next = apply(after_skull_bash, EndTurnAction())
            assert after_next.player_statuses.count("Vulnerable") == 0
            return
    pytest.fail("Could not find a seed where Skull Bash fires as the second move")


def test_gremlin_nob_bellow_grants_enrage_not_strength():
    state = _gremlin_nob(seed=42)
    after_bellow = apply(state, EndTurnAction())
    assert "Enrage" in after_bellow.monsters[0].statuses
    assert "Strength" not in after_bellow.monsters[0].statuses


def test_playing_a_skill_card_when_monster_has_enrage_gives_monster_strength():
    # Enrage(2): each time the player plays a Skill, the monster gains 2 Strength.
    state = _gremlin_nob(seed=42, deck=["Defend"] * 5)
    after_bellow = apply(state, EndTurnAction())
    assert "Enrage" in after_bellow.monsters[0].statuses

    after_defend = apply(after_bellow, PlayCardAction("Defend"))
    assert "Strength" in after_defend.monsters[0].statuses


def test_playing_an_attack_card_when_monster_has_enrage_does_not_trigger_it():
    # Only Skill cards trigger Enrage — Attacks must not.
    state = _gremlin_nob(seed=42, deck=["Strike"] * 5)
    after_bellow = apply(state, EndTurnAction())
    assert "Enrage" in after_bellow.monsters[0].statuses

    awaiting_target = apply(after_bellow, PlayCardAction("Strike"))
    after_strike = apply(awaiting_target, SelectTargetAction(0))
    assert "Strength" not in after_strike.monsters[0].statuses


def test_player_vulnerable_ticks_after_monster_attacks_not_before():
    # Vulnerable ticks at the START of the player's turn (after the monster
    # has already attacked). Skull Bash applies 2 → player enters next turn
    # with 1 (one ticked). Rush then fires with that 1 stack: floor(14*1.5)=21.
    for seed in range(200):
        state = _gremlin_nob(seed)
        after_bellow = apply(state, EndTurnAction())
        if after_bellow.monsters[0].intent != "Skull Bash":
            continue
        after_skull_bash = apply(after_bellow, EndTurnAction())
        assert after_skull_bash.player_statuses.count("Vulnerable") == 1
        if after_skull_bash.monsters[0].intent != "Rush":
            continue
        hp_before = after_skull_bash.player_hp
        after_rush = apply(after_skull_bash, EndTurnAction())
        damage = hp_before - after_rush.player_hp
        assert damage == 21, (
            f"Rush into 1 Vulnerable should deal 21 (floor(14*1.5)), "
            f"got {damage} — Vulnerable ticked before monster attacked"
        )
        return
    pytest.fail("Could not find seed with Bellow→Skull Bash→Rush sequence")


def test_weak_on_monster_reduces_its_attack_damage():
    # Placeholder until a card/move that applies Weak is added.
    pass
