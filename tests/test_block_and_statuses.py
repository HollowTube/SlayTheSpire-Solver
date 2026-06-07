from sts_sim import CombatState, apply


def make_state(hand):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monster_hp=44,
        monster_attack=6,
        seed=42,
        hand=list(hand),
    )


# Per the Slay the Spire wiki, base (un-upgraded) Defend grants 5 block.
DEFEND_BLOCK = 5


def test_playing_defend_grants_the_documented_block_without_asking_for_a_target():
    state = make_state(hand=["Defend"])

    resolved = apply(state, "PlayCard:Defend")

    assert resolved.player_block == state.player_block + DEFEND_BLOCK
    assert resolved.pending is None


def test_block_fully_absorbs_an_attack_no_larger_than_it():
    # Defend grants 5 block; the toy monster hits for 6, so block alone
    # isn't enough here — use two Defends to get 10 block, comfortably
    # absorbing the 6 damage attack.
    state = make_state(hand=["Defend", "Defend"])
    once = apply(state, "PlayCard:Defend")
    blocked = apply(once, "PlayCard:Defend")

    next_state = apply(blocked, "EndTurn")

    assert next_state.player_hp == blocked.player_hp


def test_block_smaller_than_the_attack_only_partially_absorbs_it():
    # Defend grants 5 block; the toy monster hits for 6, so 1 damage gets through.
    state = make_state(hand=["Defend"])
    blocked = apply(state, "PlayCard:Defend")

    next_state = apply(blocked, "EndTurn")

    assert next_state.player_hp == blocked.player_hp - 1


def test_block_does_not_carry_over_into_the_next_turn():
    state = make_state(hand=["Defend"])
    blocked = apply(state, "PlayCard:Defend")

    after_one_turn = apply(blocked, "EndTurn")

    assert after_one_turn.player_block == 0


def test_playing_bash_against_the_monster_leaves_it_vulnerable():
    state = make_state(hand=["Bash"])
    awaiting_target = apply(state, "PlayCard:Bash")

    resolved = apply(awaiting_target, "SelectTarget:Monster")

    assert "Vulnerable" in resolved.monster_statuses


# Per the Slay the Spire wiki, Vulnerable increases damage taken from attacks
# by 50%, rounded down: floor(STRIKE_DAMAGE * 1.5) == 9.
VULNERABLE_STRIKE_DAMAGE = 9


def test_a_vulnerable_monster_takes_amplified_damage_from_a_subsequent_strike():
    # Bash (8 dmg, applies Vulnerable) then Strike — the amplification must
    # come from the event-bus modifier pipeline reacting to Vulnerable, not a
    # hardcoded `if has_vulnerable` branch in the damage calculation.
    state = make_state(hand=["Bash", "Strike"])
    vulnerable = apply(apply(state, "PlayCard:Bash"), "SelectTarget:Monster")
    hp_before_strike = vulnerable.monster_hp

    awaiting_target = apply(vulnerable, "PlayCard:Strike")
    resolved = apply(awaiting_target, "SelectTarget:Monster")

    assert resolved.monster_hp == hp_before_strike - VULNERABLE_STRIKE_DAMAGE
