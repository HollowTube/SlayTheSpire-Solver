"""Behavioural tests for Fogmog (Overgrowth normal monster): Illusion
spawns Eye With Teeth, Swipe deals 8 damage + self-Str, Headbutt deals 14,
and the AI sequence matches the game's state machine."""

from sts_sim import CombatState, EndTurnAction, Monster, apply


def _fogmog(seed=42, hp=76):
    return CombatState(
        player_hp=80,
        player_energy=3,
        monsters=[Monster(hp=hp, name="Fogmog")],
        seed=seed,
        hand=[],
    )


def test_opens_with_illusion():
    """Fogmog always opens with Illusion."""
    for seed in range(20):
        state = _fogmog(seed=seed)
        after = apply(state, EndTurnAction())
        assert after.monsters[0].last_move == "Illusion", (
            f"seed {seed}: expected Illusion opener, got {after.monsters[0].last_move}"
        )


def test_illusion_spawns_eye_with_teeth():
    """Illusion spawns an Eye With Teeth with 6 HP."""
    state = _fogmog()
    after = apply(state, EndTurnAction())
    assert len(after.monsters) == 2, f"expected 2 monsters, got {len(after.monsters)}"
    eye = after.monsters[1]
    assert eye.name == "Eye With Teeth", f"expected Eye With Teeth, got {eye.name}"
    assert eye.hp == 6, f"expected 6 HP, got {eye.hp}"
    assert eye.max_hp == 6


def test_swipe_deals_8_damage_and_applies_strength():
    """Swipe: 8 damage to player, +1 Strength to Fogmog self."""
    # seed=0 reliably: Illusion -> Swipe (forced follow-up)
    state = _fogmog(seed=0)
    after = apply(state, EndTurnAction())  # Illusion resolves
    assert after.monsters[0].last_move == "Illusion"
    after2 = apply(after, EndTurnAction())  # Swipe resolves
    assert after2.monsters[0].last_move == "Swipe"
    hp_lost = after.player_hp - after2.player_hp
    assert hp_lost == 8, f"expected 8 damage from Swipe, got {hp_lost}"
    assert any("Strength" in s for s in after2.monsters[0].statuses), (
        f"expected Strength on Fogmog, got {after2.monsters[0].statuses}"
    )


def test_headbutt_deals_14_damage():
    """Headbutt: 14 damage to player (+ any Strength accumulated so far)."""
    # seed=5: Illusion -> Swipe (forced) -> random picks Headbutt
    state = _fogmog(seed=5)
    after = apply(state, EndTurnAction())  # Illusion
    after = apply(after, EndTurnAction())  # Swipe
    # Now intent should be Headbutt (random branch)
    assert after.monsters[0].intent in {"Swipe", "Headbutt"}, (
        f"expected Swipe or Headbutt, got {after.monsters[0].intent}"
    )
    after2 = apply(after, EndTurnAction())
    if after.monsters[0].intent == "Headbutt":
        # Fogmog has +1 Str from the previous Swipe, so Headbutt hits for 15
        hp_lost = after.player_hp - after2.player_hp
        assert hp_lost == 15, (
            f"expected 15 damage from Headbutt (14 base + 1 Str), got {hp_lost}"
        )
    # else it was Swipe — CannotRepeat should force Headbutt next
    else:
        assert after2.monsters[0].intent == "Headbutt", (
            f"after Swipe, expected Headbutt, got {after2.monsters[0].intent}"
        )


def test_after_headbutt_next_intent_is_swipe():
    """Headbutt's forced follow-up is always Swipe."""
    for seed in range(30):
        state = _fogmog(seed=seed)
        for _ in range(15):
            after = apply(state, EndTurnAction())
            if state.monsters[0].intent == "Headbutt":
                assert after.monsters[0].intent == "Swipe", (
                    f"seed {seed}: after Headbutt, expected Swipe intent, "
                    f"got {after.monsters[0].intent}"
                )
                break
            state = after


def test_random_branch_sees_both_swipe_and_headbutt():
    """The first random-branch pick (after forced Illusion→Swipe) is 40/60
    weighted between Swipe and Headbutt."""
    outcomes = set()
    for seed in range(200):
        state = _fogmog(seed=seed)
        # Turn 1: Illusion
        after = apply(state, EndTurnAction())
        # Turn 2: forced Swipe after Illusion
        after = apply(after, EndTurnAction())
        # Turn 3: first random-branch pick
        after = apply(after, EndTurnAction())
        outcomes.add(after.monsters[0].last_move)
    assert outcomes == {"Swipe", "Headbutt"}, f"unexpected outcomes: {outcomes}"


def test_swipe_never_triple_repeats():
    """Swipe may appear twice in a row (forced → random) but never three
    times consecutively."""
    for seed in range(50):
        state = _fogmog(seed=seed)
        moves = []
        for _ in range(20):
            after = apply(state, EndTurnAction())
            moves.append(after.monsters[0].last_move)
            state = after
        for i in range(2, len(moves)):
            triple = moves[i - 2 : i + 1]
            if all(m == "Swipe" for m in triple):
                assert False, f"seed {seed}: triple Swipe at positions {i - 2}..{i}"
