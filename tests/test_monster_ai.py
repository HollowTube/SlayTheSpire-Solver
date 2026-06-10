from sts_sim import CombatState, Monster, apply


def make_state(monster_name):
    # monster_attack is meaningless for an AI-driven monster (its move pool
    # decides what it does each turn) — passed as 0 to satisfy the
    # constructor's existing required parameter.
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, name=monster_name)],
        seed=42,
        hand=[],
    )


# Per the Slay the Spire wiki, Thrash deals 7 damage and grants 5 block.
THRASH_DAMAGE = 7
THRASH_BLOCK = 5

# Per the Slay the Spire wiki, Bellow grants 3 Strength and 6 block.
BELLOW_STRENGTH = 3
BELLOW_BLOCK = 6


def test_a_jaw_worm_telegraphs_chomp_as_its_opening_move():
    # Per the Slay the Spire wiki, Jaw Worm always opens combat with Chomp.
    state = make_state("Jaw Worm")

    assert state.monsters[0].intent == "Chomp"


# Per the Slay the Spire wiki, Chomp deals 11 damage.
CHOMP_DAMAGE = 11


def test_a_jaw_worms_intent_sequence_follows_its_documented_pattern_and_constraints():
    # Per the Slay the Spire wiki: Jaw Worm opens with Chomp, then rolls from
    # {Bellow 45%, Thrash 30%, Chomp 25%} each turn — but can't repeat Bellow
    # or Chomp, and can't Thrash a 3rd consecutive time. Run many turns under
    # a fixed seed (player HP padded so the fight never ends mid-run) and
    # check every telegraphed intent obeys the documented pool and streak
    # constraints — a property the *sequence* must hold regardless of which
    # exact moves the RNG happens to roll.
    state = CombatState(
        player_hp=100_000,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, name="Jaw Worm")],
        seed=1,
        hand=[],
    )

    intents = [state.monsters[0].intent]
    for _ in range(200):
        state = apply(state, "EndTurn")
        intents.append(state.monsters[0].intent)

    assert intents[0] == "Chomp"
    assert all(intent in {"Chomp", "Thrash", "Bellow"} for intent in intents)

    streak = 1
    for previous, current in zip(intents, intents[1:]):
        streak = streak + 1 if current == previous else 1
        allowed = 2 if current == "Thrash" else 1
        assert streak <= allowed, f"{current} ran {streak} times in a row"


def test_ending_the_turn_resolves_the_telegraphed_chomp_against_the_player():
    state = make_state("Jaw Worm")
    assert state.monsters[0].intent == "Chomp"

    next_state = apply(state, "EndTurn")

    assert next_state.player_hp == state.player_hp - CHOMP_DAMAGE


def test_thrash_deals_damage_to_the_player_and_grants_the_jaw_worm_block():
    # seed=6 happens to roll Chomp then Thrash as the worm's first two moves
    # — found by sampling seeds for this exact sequence, since the AI is
    # genuinely RNG-driven and there's no way to force a move from Python.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, name="Jaw Worm")],
        seed=6,
        hand=[],
    )
    after_chomp = apply(state, "EndTurn")
    assert after_chomp.monsters[0].intent == "Thrash"

    after_thrash = apply(after_chomp, "EndTurn")

    assert after_thrash.player_hp == after_chomp.player_hp - THRASH_DAMAGE
    assert after_thrash.monsters[0].block == THRASH_BLOCK


def test_bellow_grants_the_jaw_worm_strength_and_block_without_attacking():
    # seed=0 rolls Chomp then Bellow as the worm's first two moves.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, name="Jaw Worm")],
        seed=0,
        hand=[],
    )
    after_chomp = apply(state, "EndTurn")
    assert after_chomp.monsters[0].intent == "Bellow"

    after_bellow = apply(after_chomp, "EndTurn")

    assert after_bellow.player_hp == after_chomp.player_hp
    assert "Strength" in after_bellow.monsters[0].statuses
    assert after_bellow.monsters[0].block == BELLOW_BLOCK


def test_the_jaw_worms_block_absorbs_the_players_subsequent_attack():
    # Block must work symmetrically — the wiki documents it reducing incoming
    # damage from anyone, not just attacks against the player. seed=0's
    # Bellow grants 6 block; the player's 6-damage Strike should be fully
    # absorbed rather than touching monster_hp.
    state = CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, name="Jaw Worm")],
        seed=0,
        hand=["Strike"],
    )
    after_chomp = apply(state, "EndTurn")
    after_bellow = apply(after_chomp, "EndTurn")
    assert after_bellow.monsters[0].block == BELLOW_BLOCK

    awaiting_target = apply(after_bellow, "PlayCard:Strike")
    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert resolved.monsters[0].hp == after_bellow.monsters[0].hp
    assert resolved.monsters[0].block == BELLOW_BLOCK - 6


def test_the_jaw_worms_strength_amplifies_its_own_subsequent_attack():
    # seed=2 rolls Chomp, Bellow, Chomp — letting us watch the +3 Strength
    # from Bellow amplify the worm's very next Chomp. Strength must flow
    # through the same generic event-bus modifier pipeline that already
    # amplifies the *player's* damage — symmetrically, with no special-casing
    # for which side holds the buff.
    state = CombatState(
        player_hp=10_000,
        player_energy=3,
        monsters=[Monster(hp=44, attack=0, name="Jaw Worm")],
        seed=2,
        hand=[],
    )
    after_chomp = apply(state, "EndTurn")
    after_bellow = apply(after_chomp, "EndTurn")
    assert after_bellow.monsters[0].intent == "Chomp"
    hp_before_strengthened_chomp = after_bellow.player_hp

    after_strengthened_chomp = apply(after_bellow, "EndTurn")

    assert after_strengthened_chomp.player_hp == hp_before_strengthened_chomp - (
        CHOMP_DAMAGE + BELLOW_STRENGTH
    )
