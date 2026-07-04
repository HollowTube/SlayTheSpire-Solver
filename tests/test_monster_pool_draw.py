"""Behavioural tests for the Overgrowth weak/normal monster-pool draw
(HOL-61): a deterministic, anti-repeat-then-cycle sequence generator for
run-level encounter assignment, verified in isolation from RunState."""

from sts_sim import draw_overgrowth_monster_sequence

WEAK_POOL = {
    "Nibbit",
    "Fuzzy Wurm Crawler",
    "Leaf Slime (S)",
    "Leaf Slime (M)",
    "Twig Slime (S)",
    "Twig Slime (M)",
    "Shrinker Beetle",
}

# Every other monster sts_sim implements as a regular Overgrowth room
# encounter — Kin Priest/Kin Follower are excluded (TheKinBoss-only).
NORMAL_POOL = {
    "Inklet",
    "Snapping Jaxfruit",
    "ruby_raiders",
    "Slithering Strangler",
    "Cubex Construct",
    "Mawler",
    "Vine Shambler",
    "Flyconid",
    "Fogmog",
}


def test_a_short_sequence_draws_only_from_the_weak_pool():
    sequence = draw_overgrowth_monster_sequence(seed=1, slots=3)
    assert len(sequence) == 3
    assert all(name in WEAK_POOL for name in sequence)


def test_slots_beyond_the_weak_encounter_count_draw_from_the_normal_pool():
    """The real game's weak_encounter_count is 3 for Overgrowth — the 4th+
    slot must come from the normal pool, not the weak one."""
    sequence = draw_overgrowth_monster_sequence(seed=1, slots=5)
    assert all(name in WEAK_POOL for name in sequence[:3])
    assert all(name in NORMAL_POOL for name in sequence[3:])


def test_same_seed_and_slot_count_reproduces_the_same_sequence():
    first = draw_overgrowth_monster_sequence(seed=42, slots=10)
    second = draw_overgrowth_monster_sequence(seed=42, slots=10)
    assert first == second


def test_no_monster_repeats_immediately_within_either_pool_even_when_cycling():
    """Drawing far more slots than either pool's size forces cycling
    (reshuffling) multiple times — no adjacent duplicate should ever appear,
    including exactly at a reshuffle boundary."""
    sequence = draw_overgrowth_monster_sequence(seed=5, slots=60)
    weak_portion = sequence[:3]
    normal_portion = sequence[3:]
    for portion in (weak_portion, normal_portion):
        for a, b in zip(portion, portion[1:]):
            assert a != b


def test_normal_pool_visits_every_monster_it_should_contain_and_no_others():
    """Pins the normal pool's exact membership — a hand-copied set in this
    test file could otherwise drift silently from what the Rust side
    actually contains (it would only ever fail to disprove itself). The
    weak pool can't be exercised this way through the public draw function
    (its slot count is always capped at weak_encounter_count=3) — its exact
    membership is pinned by a Rust-side unit test in `src/act.rs` instead."""
    sequence = draw_overgrowth_monster_sequence(seed=11, slots=3 + len(NORMAL_POOL) * 3)
    assert set(sequence[3:]) == NORMAL_POOL
