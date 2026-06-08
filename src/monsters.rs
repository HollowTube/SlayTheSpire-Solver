use crate::engine::{EffectOp, Status};
use rand::Rng;
use rand_pcg::Pcg32;

/// The move a monster telegraphs before it has acted at all — Slay the
/// Spire's documented monster patterns each name a fixed opening move (e.g.
/// Jaw Worm always opens with Chomp) before any RNG-driven selection kicks in.
pub(crate) fn opening_intent(monster_name: &str) -> Option<String> {
    match monster_name {
        "Jaw Worm" => Some("Chomp".to_string()),
        _ => None,
    }
}

/// A monster move's declarative effect pipeline — interpreted by the same
/// generic `run_effect_ops` engine as cards (with `Actor::Monster`), so that
/// adding an ordinary monster move means adding data here, not new logic.
pub(crate) fn monster_move(monster_name: &str, move_name: &str) -> Option<Vec<EffectOp>> {
    match (monster_name, move_name) {
        // Per the Slay the Spire wiki, Jaw Worm's move pool:
        ("Jaw Worm", "Chomp") => Some(vec![EffectOp::DealDamage(11)]),
        ("Jaw Worm", "Thrash") => Some(vec![EffectOp::DealDamage(7), EffectOp::GainBlock(5)]),
        ("Jaw Worm", "Bellow") => Some(vec![
            EffectOp::ApplyStatusToSelf(Status::Strength(3)),
            EffectOp::GainBlock(6),
        ]),
        _ => None,
    }
}

/// How many times a move may occur back-to-back before the AI is forced to
/// pick something else — per the wiki, Jaw Worm can't repeat Chomp or Bellow
/// at all, but may Thrash up to twice in a row (i.e. not a 3rd time).
fn max_streak(monster_name: &str, move_name: &str) -> u32 {
    match (monster_name, move_name) {
        ("Jaw Worm", "Thrash") => 2,
        ("Jaw Worm", _) => 1,
        _ => u32::MAX,
    }
}

/// Rolls the monster's next telegraphed move from its documented weighted
/// pattern, re-rolling whenever the result would extend a same-move streak
/// past its limit — e.g. Jaw Worm picks Bellow 45% / Thrash 30% / Chomp 25%
/// of the time, but never repeats Bellow/Chomp and never Thrashes a 3rd
/// consecutive time. `last_move`/`streak` describe the run of moves leading
/// up to (and including) the one that just resolved.
pub(crate) fn select_next_intent(
    monster_name: &str,
    last_move: &Option<String>,
    streak: u32,
    rng: &mut Pcg32,
) -> Option<String> {
    match monster_name {
        "Jaw Worm" => loop {
            let roll = rng.gen_range(0..100);
            let candidate = if roll < 45 {
                "Bellow"
            } else if roll < 75 {
                "Thrash"
            } else {
                "Chomp"
            };
            let resulting_streak = if last_move.as_deref() == Some(candidate) {
                streak + 1
            } else {
                1
            };
            if resulting_streak <= max_streak(monster_name, candidate) {
                return Some(candidate.to_string());
            }
        },
        _ => None,
    }
}
