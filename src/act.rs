//! Per-Act monster-pool definitions and the weak/normal draw algorithm
//! (HOL-61). Mirrors the real game's `ActModel`: the first `weak_encounter
//! _count` monster-node slots draw from a "weak" pool, the rest from a
//! "normal" pool, each pool drawn without an immediate repeat and cycling
//! (reshuffling) once exhausted rather than erroring. Deliberately verified
//! here in isolation — wiring this into `RunState`'s assembled path is a
//! separate issue (HOL-65).
use pyo3::prelude::*;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_pcg::Pcg32;

/// Overgrowth's weak pool — the monsters `sts_sim` implements that the real
/// game's `NibbitsWeak`/`FuzzyWurmCrawlerWeak`/`SlimesWeak`/
/// `ShrinkerBeetleWeak` encounters draw from.
pub(crate) const OVERGROWTH_WEAK_POOL: &[&str] = &[
    "Nibbit",
    "Fuzzy Wurm Crawler",
    "Leaf Slime (S)",
    "Leaf Slime (M)",
    "Twig Slime (S)",
    "Twig Slime (M)",
    "Shrinker Beetle",
];

/// Overgrowth's normal pool — every other regular (non-elite, non-boss)
/// monster `sts_sim` implements. Kin Priest/Kin Follower are excluded: in
/// the real game they only appear assembled into `TheKinBoss`, never as a
/// standalone normal-room encounter. Widen this as more Overgrowth monsters
/// land (HOL-66).
pub(crate) const OVERGROWTH_NORMAL_POOL: &[&str] = &[
    "Inklet",
    "Snapping Jaxfruit",
    "Axe Ruby Raider",
    "Assassin Ruby Raider",
    "Brute Ruby Raider",
    "Crossbow Ruby Raider",
    "Tracker Ruby Raider",
    "Slithering Strangler",
    "Cubex Construct",
    "Mawler",
    "Vine Shambler",
    "Flyconid",
    "Fogmog",
];

/// Overgrowth's elite pool — distinct from the weak/normal monster pools,
/// drawn uniform-random-with-seed (no anti-repeat/cycling: HOL-62's scope
/// is one elite per draw, not a sequence). Every `sts_sim` implements with
/// a true `RoomType.Elite` encounter in the real game (verified directly
/// against `Overgrowth.cs` — several monsters informally called "elite" in
/// older issue text, e.g. Cubex Construct, are actually `RoomType.Monster`
/// and belong in `OVERGROWTH_NORMAL_POOL` instead, not here).
pub(crate) const OVERGROWTH_ELITE_POOL: &[&str] = &["Byrdonis", "Phrog Parasite", "Bygone Effigy"];

/// Boss encounters — each Overgrowth act ends with a boss fight. Unlike
/// the other pools, bosses aren't drawn randomly per-run; the pool is
/// recorded here for completeness and future encounter selection logic.
pub(crate) const OVERGROWTH_BOSS_POOL: &[&str] = &["Ceremonial Beast", "The Kin", "Vantom"];

/// How many of the leading monster-node slots draw from the weak pool
/// before falling through to the normal pool — matches the real game's
/// `Overgrowth.NumberOfWeakEncounters`. A plain constant for now; a future
/// issue can replace this with a randomized roll (HOL-67) without touching
/// `draw_monster_sequence`'s signature, since the count is already passed
/// in by the caller rather than baked into the draw algorithm itself.
pub(crate) const OVERGROWTH_WEAK_ENCOUNTER_COUNT: usize = 3;

/// Draws `slots` names from `pool` (in order), never repeating the
/// immediately-previous draw, and reshuffling (cycling) once the pool is
/// exhausted rather than erroring. A pool of size <= 1 can't avoid a repeat
/// and is allowed to repeat in that degenerate case.
fn draw_from_pool(pool: &[&str], slots: usize, rng: &mut Pcg32) -> Vec<String> {
    if pool.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(slots);
    let mut bag: Vec<&str> = Vec::new();
    let mut last: Option<&str> = None;
    while result.len() < slots {
        if bag.is_empty() {
            bag = pool.to_vec();
            bag.shuffle(rng);
            if pool.len() > 1 && bag.first() == last.as_ref() {
                bag.swap(0, 1);
            }
        }
        let next = bag.remove(0);
        result.push(next.to_string());
        last = Some(next);
    }
    result
}

/// The full weak-then-normal draw for `total_slots` monster nodes, seeded
/// so the same `(seed, total_slots)` always reproduces the same sequence.
pub(crate) fn draw_monster_sequence(
    weak_pool: &[&str],
    normal_pool: &[&str],
    weak_encounter_count: usize,
    total_slots: usize,
    seed: u64,
) -> Vec<String> {
    let mut rng = Pcg32::seed_from_u64(seed);
    let weak_slots = weak_encounter_count.min(total_slots);
    let normal_slots = total_slots - weak_slots;
    let mut result = draw_from_pool(weak_pool, weak_slots, &mut rng);
    result.extend(draw_from_pool(normal_pool, normal_slots, &mut rng));
    result
}

/// Python-facing entry point for Overgrowth's draw — the only Act `sts_sim`
/// currently models content for.
#[pyfunction]
pub(crate) fn draw_overgrowth_monster_sequence(seed: u64, slots: usize) -> Vec<String> {
    draw_monster_sequence(
        OVERGROWTH_WEAK_POOL,
        OVERGROWTH_NORMAL_POOL,
        OVERGROWTH_WEAK_ENCOUNTER_COUNT,
        slots,
        seed,
    )
}

/// Draws one elite name from `OVERGROWTH_ELITE_POOL`, seeded.
#[pyfunction]
pub(crate) fn draw_overgrowth_elite(seed: u64) -> String {
    let mut rng = Pcg32::seed_from_u64(seed);
    OVERGROWTH_ELITE_POOL.choose(&mut rng).expect("elite pool is never empty").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Pins the weak pool's exact membership directly (bypassing the
    /// weak_encounter_count=3 cap that `draw_monster_sequence`/
    /// `draw_overgrowth_monster_sequence` would otherwise impose on any
    /// slot count) — a hand-copied expected set elsewhere could otherwise
    /// drift silently from what this pool actually contains.
    #[test]
    fn weak_pool_draw_visits_every_monster_it_should_contain_and_no_others() {
        let mut rng = Pcg32::seed_from_u64(11);
        let sequence = draw_from_pool(OVERGROWTH_WEAK_POOL, OVERGROWTH_WEAK_POOL.len() * 3, &mut rng);
        let visited: HashSet<&str> = sequence.iter().map(String::as_str).collect();
        let expected: HashSet<&str> = OVERGROWTH_WEAK_POOL.iter().copied().collect();
        assert_eq!(visited, expected);
    }
}
