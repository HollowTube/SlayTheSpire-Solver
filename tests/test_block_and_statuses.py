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


# Per the Slay the Spire wiki, base (un-upgraded) Iron Wave deals 5 damage and
# grants 5 block — a single targeted card whose effect pipeline both deals
# damage (which needs the SelectTarget protocol) and grants block (which
# doesn't), proving the pipeline composes ops of different shapes in one card.
IRON_WAVE_DAMAGE = 5
IRON_WAVE_BLOCK = 5


def test_playing_iron_wave_deals_damage_and_grants_block_from_a_single_card():
    state = make_state(hand=["Iron Wave"])
    awaiting_target = apply(state, "PlayCard:Iron Wave")

    resolved = apply(awaiting_target, "SelectTarget:Monster")

    assert resolved.monster_hp == state.monster_hp - IRON_WAVE_DAMAGE
    assert resolved.player_block == state.player_block + IRON_WAVE_BLOCK


# Per the Slay the Spire wiki, base (un-upgraded) Inflame grants 2 Strength
# (a permanent, stacking buff — unlike Vulnerable it never expires on its
# own), and Strength adds its stack count to each attack's damage output.
INFLAME_STRENGTH = 2
STRIKE_DAMAGE_WITH_INFLAME = 6 + INFLAME_STRENGTH


def test_playing_inflame_grants_the_player_strength_without_asking_for_a_target():
    state = make_state(hand=["Inflame"])

    resolved = apply(state, "PlayCard:Inflame")

    assert "Strength" in resolved.player_statuses
    assert resolved.pending is None


def test_strength_increases_the_damage_dealt_by_a_subsequent_strike():
    # Strength must amplify damage through the event-bus modifier pipeline
    # reacting to the player's own Strength stacks — not a hardcoded
    # `if has_strength` branch in the damage calculation, mirroring how
    # Vulnerable amplifies damage taken from the target side.
    state = make_state(hand=["Inflame", "Strike"])
    strengthened = apply(state, "PlayCard:Inflame")

    awaiting_target = apply(strengthened, "PlayCard:Strike")
    resolved = apply(awaiting_target, "SelectTarget:Monster")

    assert resolved.monster_hp == strengthened.monster_hp - STRIKE_DAMAGE_WITH_INFLAME


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
