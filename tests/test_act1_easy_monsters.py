"""Behavioural tests for the remaining Act 1 "easy pool" monsters: the
Shrinker Beetle and the four Slime variants (Twig Slime S/M, Leaf Slime S/M)."""

from sts_sim import CombatState, Monster, apply


def _twig_slime_s(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=11, attack=0, name="Twig Slime (S)")],
        seed=seed,
        hand=hand or [],
        deck=deck,
    )


def _shrinker_beetle(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=38, attack=0, name="Shrinker Beetle")],
        seed=seed,
        hand=hand or [],
        deck=deck,
    )


def _leaf_slime_s(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=13, attack=0, name="Leaf Slime (S)")],
        seed=seed,
        hand=hand or [],
        deck=deck,
    )


def _leaf_slime_m(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=33, attack=0, name="Leaf Slime (M)")],
        seed=seed,
        hand=hand or [],
        deck=deck,
    )


def _twig_slime_m(seed=0, hand=None, deck=None):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=27, attack=0, name="Twig Slime (M)")],
        seed=seed,
        hand=hand or [],
        deck=deck,
    )


# ── Twig Slime (S) ──────────────────────────────────────────────────────────


def test_twig_slime_s_opens_with_tackle():
    state = _twig_slime_s()
    assert state.monsters[0].intent == "Tackle"


def test_twig_slime_s_tackle_deals_4_damage():
    state = _twig_slime_s()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 4


def test_twig_slime_s_repeats_tackle_forever():
    state = _twig_slime_s()
    after = apply(state, "EndTurn")
    assert after.monsters[0].intent == "Tackle"
    after2 = apply(after, "EndTurn")
    assert after2.monsters[0].intent == "Tackle"


# ── Shrinker Beetle ──────────────────────────────────────────────────────────


def test_shrinker_beetle_opens_with_shrink():
    state = _shrinker_beetle()
    assert state.monsters[0].intent == "Shrink"


def test_shrinker_beetle_shrink_applies_shrink_to_the_player_without_damage():
    state = _shrinker_beetle()
    after = apply(state, "EndTurn")
    assert after.player_hp == state.player_hp
    assert "Shrink" in after.player_statuses


def test_shrinker_beetle_shrink_reduces_the_players_subsequent_attack_damage():
    # Shrink reduces the player's outgoing damage by 30%, rounded down:
    # floor(6 * 0.7) = 4.
    state = _shrinker_beetle(hand=["Strike"])
    after_shrink = apply(state, "EndTurn")
    assert "Shrink" in after_shrink.player_statuses

    awaiting_target = apply(after_shrink, "PlayCard:Strike")
    resolved = apply(awaiting_target, "SelectTarget:Monster:0")

    assert after_shrink.monsters[0].hp - resolved.monsters[0].hp == 4


def test_shrinker_beetle_shrink_does_not_decay():
    state = _shrinker_beetle(hand=["Strike"] * 5)
    after_shrink = apply(state, "EndTurn")
    assert "Shrink" in after_shrink.player_statuses

    after_another_turn = apply(after_shrink, "EndTurn")
    assert "Shrink" in after_another_turn.player_statuses


def test_shrinker_beetle_alternates_chomp_and_stomp_after_shrink():
    state = _shrinker_beetle()
    assert state.monsters[0].intent == "Shrink"

    after_shrink = apply(state, "EndTurn")
    assert after_shrink.monsters[0].intent == "Chomp"

    after_chomp = apply(after_shrink, "EndTurn")
    assert after_chomp.monsters[0].intent == "Stomp"

    after_stomp = apply(after_chomp, "EndTurn")
    assert after_stomp.monsters[0].intent == "Chomp"


def test_shrinker_beetle_chomp_deals_7_and_stomp_deals_13():
    state = _shrinker_beetle()
    after_shrink = apply(state, "EndTurn")
    assert after_shrink.monsters[0].intent == "Chomp"

    hp_before_chomp = after_shrink.player_hp
    after_chomp = apply(after_shrink, "EndTurn")
    assert hp_before_chomp - after_chomp.player_hp == 7

    assert after_chomp.monsters[0].intent == "Stomp"
    hp_before_stomp = after_chomp.player_hp
    after_stomp = apply(after_chomp, "EndTurn")
    assert hp_before_stomp - after_stomp.player_hp == 13


# ── Leaf Slime (S) ───────────────────────────────────────────────────────────


def test_leaf_slime_s_opens_with_tackle():
    state = _leaf_slime_s()
    assert state.monsters[0].intent == "Tackle"


def test_leaf_slime_s_tackle_deals_3_damage():
    state = _leaf_slime_s()
    after = apply(state, "EndTurn")
    assert state.player_hp - after.player_hp == 3


def test_leaf_slime_s_alternates_tackle_and_goop_forever():
    state = _leaf_slime_s()
    assert state.monsters[0].intent == "Tackle"

    after_tackle = apply(state, "EndTurn")
    assert after_tackle.monsters[0].intent == "Goop"

    after_goop = apply(after_tackle, "EndTurn")
    assert after_goop.monsters[0].intent == "Tackle"

    after_tackle2 = apply(after_goop, "EndTurn")
    assert after_tackle2.monsters[0].intent == "Goop"


def test_leaf_slime_s_goop_deals_no_damage_and_gives_player_a_slimed_card():
    state = _leaf_slime_s()
    after_tackle = apply(state, "EndTurn")
    assert after_tackle.monsters[0].intent == "Goop"

    hp_before = after_tackle.player_hp
    after_goop = apply(after_tackle, "EndTurn")

    assert after_goop.player_hp == hp_before
    # Goop sticks a "Slimed" card into the player's discard pile; with both
    # piles otherwise empty, end-of-turn drawing immediately reshuffles it
    # into the new hand — so it shows up there rather than sitting in
    # discard_pile.
    assert "Slimed" in after_goop.hand


# ── Leaf Slime (M) ───────────────────────────────────────────────────────────


def test_leaf_slime_m_opens_with_sticky_shot():
    state = _leaf_slime_m()
    assert state.monsters[0].intent == "StickyShot"


def test_leaf_slime_m_sticky_shot_deals_no_damage_and_gives_player_two_slimed_cards():
    state = _leaf_slime_m()
    hp_before = state.player_hp

    after_sticky = apply(state, "EndTurn")

    assert after_sticky.player_hp == hp_before
    # Two "Slimed" cards land in discard, then both get reshuffled and drawn
    # into the fresh hand (same mechanism as Leaf Slime (S)'s Goop).
    assert after_sticky.hand.count("Slimed") == 2


def test_leaf_slime_m_alternates_sticky_shot_and_clump_shot_forever():
    state = _leaf_slime_m()
    assert state.monsters[0].intent == "StickyShot"

    after_sticky = apply(state, "EndTurn")
    assert after_sticky.monsters[0].intent == "ClumpShot"

    after_clump = apply(after_sticky, "EndTurn")
    assert after_clump.monsters[0].intent == "StickyShot"

    after_sticky2 = apply(after_clump, "EndTurn")
    assert after_sticky2.monsters[0].intent == "ClumpShot"


def test_leaf_slime_m_clump_shot_deals_8_damage():
    state = _leaf_slime_m()
    after_sticky = apply(state, "EndTurn")
    assert after_sticky.monsters[0].intent == "ClumpShot"

    hp_before = after_sticky.player_hp
    after_clump = apply(after_sticky, "EndTurn")

    assert hp_before - after_clump.player_hp == 8


# ── Twig Slime (M) ───────────────────────────────────────────────────────────


def test_twig_slime_m_opens_with_sticky_shot():
    state = _twig_slime_m()
    assert state.monsters[0].intent == "StickyShot"


def test_twig_slime_m_sticky_shot_deals_no_damage_and_gives_player_one_slimed_card():
    state = _twig_slime_m()
    hp_before = state.player_hp

    after_sticky = apply(state, "EndTurn")

    assert after_sticky.player_hp == hp_before
    assert after_sticky.hand.count("Slimed") == 1


def test_twig_slime_m_turn_2_is_forced_clump_shot_for_11_damage():
    # StickyShot can never repeat, so the turn after the opener is forced
    # to be ClumpShot regardless of the RNG roll.
    state = _twig_slime_m()
    after_sticky = apply(state, "EndTurn")
    assert after_sticky.monsters[0].intent == "ClumpShot"

    hp_before = after_sticky.player_hp
    after_clump = apply(after_sticky, "EndTurn")

    assert hp_before - after_clump.player_hp == 11


def test_twig_slime_m_intent_sequence_follows_its_documented_pattern_and_constraints():
    # Per the wiki: after the forced ClumpShot, Twig Slime (M) rolls
    # ClumpShot (67%) / StickyShot (33%) each turn, but StickyShot can never
    # repeat consecutively. Run many turns and check the sequence obeys the
    # documented pool and streak constraint regardless of which exact moves
    # the RNG happens to roll.
    state = CombatState(
        player_hp=100_000,
        player_energy=3,
        monsters=[Monster(hp=27, attack=0, name="Twig Slime (M)")],
        seed=1,
        hand=[],
    )

    intents = [state.monsters[0].intent]
    for _ in range(200):
        state = apply(state, "EndTurn")
        intents.append(state.monsters[0].intent)

    assert intents[0] == "StickyShot"
    assert intents[1] == "ClumpShot"
    assert all(intent in {"StickyShot", "ClumpShot"} for intent in intents)

    streak = 1
    for previous, current in zip(intents, intents[1:]):
        streak = streak + 1 if current == previous else 1
        if current == "StickyShot":
            assert streak <= 1, "StickyShot ran more than once in a row"
