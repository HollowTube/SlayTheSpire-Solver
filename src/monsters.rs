use crate::engine::{EffectOp, Status};
use rand::Rng;
use rand_pcg::Pcg32;

/// The move a monster telegraphs before it has acted at all — Slay the
/// Spire's documented monster patterns each name a fixed opening move (e.g.
/// Jaw Worm always opens with Chomp) before any RNG-driven selection kicks in.
pub(crate) fn opening_intent(monster_name: &str) -> Option<String> {
    match monster_name {
        "Jaw Worm" => Some("Chomp".to_string()),
        "Gremlin Nob" => Some("Bellow".to_string()),
        "Nibbit" => Some("Butt".to_string()),
        "Fuzzy Wurm Crawler" => Some("Acid Goop".to_string()),
        "Twig Slime (S)" => Some("Tackle".to_string()),
        "Shrinker Beetle" => Some("Shrink".to_string()),
        // Per the wiki, Leaf Slime (S) opens with a 50/50 roll between Tackle
        // and Goop; `opening_intent` has no RNG access (mirrors every other
        // monster's fixed opener), so Tackle is picked deterministically —
        // it then strictly alternates with Goop forever regardless.
        "Leaf Slime (S)" => Some("Tackle".to_string()),
        // Leaf Slime (M) always opens with StickyShot.
        "Leaf Slime (M)" => Some("StickyShot".to_string()),
        // Twig Slime (M) always opens with StickyShot.
        "Twig Slime (M)" => Some("StickyShot".to_string()),
        // Byrdonis always opens with Swoop.
        "Byrdonis" => Some("Swoop".to_string()),
        // Inklet always opens with Jab.
        "Inklet" => Some("Jab".to_string()),
        // Vantom always opens with Ink Blot.
        "Vantom" => Some("Ink Blot".to_string()),
        // Snapping Jaxfruit: single move "Energy Orb" forever.
        "Snapping Jaxfruit" => Some("Energy Orb".to_string()),
        // Axe Ruby Raider: 3-move fixed cycle, opens with Swing 1.
        "Axe Ruby Raider" => Some("Swing 1".to_string()),
        // Assassin Ruby Raider: single move "Killshot" forever.
        "Assassin Ruby Raider" => Some("Killshot".to_string()),
        // Brute Ruby Raider: 2-move fixed cycle, opens with Beat.
        "Brute Ruby Raider" => Some("Beat".to_string()),
        // Crossbow Ruby Raider: 2-move fixed cycle, opens with Reload.
        "Crossbow Ruby Raider" => Some("Reload".to_string()),
        // Slithering Strangler: opens with Constrict.
        "Slithering Strangler" => Some("Constrict".to_string()),
        // Cubex Construct: opens with Charge Up.
        "Cubex Construct" => Some("Charge Up".to_string()),
        // Kin Priest: fixed 4-move cycle, opens with Orb of Frailty.
        "Kin Priest" => Some("Orb of Frailty".to_string()),
        // Kin Follower: fixed 3-move cycle, opens with Quick Slash.
        "Kin Follower" => Some("Quick Slash".to_string()),
        // Phrog Parasite: fixed 2-move cycle, opens with Infect.
        "Phrog Parasite" => Some("Infect".to_string()),
        // Wriggler: the opening intent is set via the intent= constructor
        // override (odd-index Wrigglers open with Nasty Bite, even-index open
        // with Wriggle). The default opener is Nasty Bite.
        "Wriggler" => Some("Nasty Bite".to_string()),
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
        // Per the Slay the Spire wiki, Gremlin Nob's move pool:
        // Bellow grants Enrage(2) — not Strength; Enrage triggers on Skill plays.
        ("Gremlin Nob", "Bellow") => Some(vec![EffectOp::ApplyStatusToSelf(Status::Enrage(2))]),
        ("Gremlin Nob", "Rush") => Some(vec![EffectOp::DealDamage(14)]),
        ("Gremlin Nob", "Skull Bash") => Some(vec![
            EffectOp::DealDamage(6),
            EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            EffectOp::ApplyStatusToTarget(Status::Vulnerable),
        ]),
        // Nibbit: fixed 3-move cycle (Butt → Hesitant Slice → Hiss → repeat).
        ("Nibbit", "Butt") => Some(vec![EffectOp::DealDamage(12)]),
        ("Nibbit", "Hesitant Slice") => Some(vec![
            EffectOp::DealDamage(6),
            EffectOp::GainBlock(5),
        ]),
        ("Nibbit", "Hiss") => Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))]),
        // Fuzzy Wurm Crawler: alternating Acid Goop / Inhale.
        ("Fuzzy Wurm Crawler", "Acid Goop") => Some(vec![EffectOp::DealDamage(4)]),
        ("Fuzzy Wurm Crawler", "Inhale") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(7))])
        }
        // Twig Slime (S): a single repeating Tackle for 4 damage.
        ("Twig Slime (S)", "Tackle") => Some(vec![EffectOp::DealDamage(4)]),
        // Shrinker Beetle: opens with Shrink (applies Status::Shrink to the
        // player, no damage), then alternates Chomp (7) <-> Stomp (13).
        ("Shrinker Beetle", "Shrink") => {
            Some(vec![EffectOp::ApplyStatusToTarget(Status::Shrink)])
        }
        ("Shrinker Beetle", "Chomp") => Some(vec![EffectOp::DealDamage(7)]),
        ("Shrinker Beetle", "Stomp") => Some(vec![EffectOp::DealDamage(13)]),
        // Leaf Slime (S): Tackle for 3, or Goop a "Slimed" card into the
        // player's discard pile (no damage).
        ("Leaf Slime (S)", "Tackle") => Some(vec![EffectOp::DealDamage(3)]),
        ("Leaf Slime (S)", "Goop") => {
            Some(vec![EffectOp::ApplyCardToTarget("Slimed".to_string())])
        }
        // Leaf Slime (M): StickyShot gives the player two "Slimed" cards (no
        // damage); ClumpShot deals 8.
        ("Leaf Slime (M)", "StickyShot") => Some(vec![
            EffectOp::ApplyCardToTarget("Slimed".to_string()),
            EffectOp::ApplyCardToTarget("Slimed".to_string()),
        ]),
        ("Leaf Slime (M)", "ClumpShot") => Some(vec![EffectOp::DealDamage(8)]),
        // Twig Slime (M): StickyShot gives the player one "Slimed" card (no
        // damage); ClumpShot deals 11.
        ("Twig Slime (M)", "StickyShot") => {
            Some(vec![EffectOp::ApplyCardToTarget("Slimed".to_string())])
        }
        ("Twig Slime (M)", "ClumpShot") => Some(vec![EffectOp::DealDamage(11)]),
        // Byrdonis: alternates Swoop (17) <-> Peck (3 hits of 3), and has
        // Territorial 1 - +1 Strength to itself at the end of every one of
        // its turns, regardless of which move it used. Baking the Strength
        // gain into the tail of each move's effects (after the damage ops)
        // means this turn's damage uses last turn's Strength, and the gain
        // is in place for next turn - matching TerritorialPower's AfterTurnEnd
        // hook.
        ("Byrdonis", "Swoop") => Some(vec![
            EffectOp::DealDamage(17),
            EffectOp::ApplyStatusToSelf(Status::Strength(1)),
        ]),
        ("Byrdonis", "Peck") => Some(vec![
            EffectOp::DealDamage(3),
            EffectOp::DealDamage(3),
            EffectOp::DealDamage(3),
            EffectOp::ApplyStatusToSelf(Status::Strength(1)),
        ]),
        // Inklet: Jab (3), Windup Punch (2 x3), Piercing Gaze (10).
        ("Inklet", "Jab") => Some(vec![EffectOp::DealDamage(3)]),
        ("Inklet", "Windup Punch") => Some(vec![
            EffectOp::DealDamage(2),
            EffectOp::DealDamage(2),
            EffectOp::DealDamage(2),
        ]),
        ("Inklet", "Piercing Gaze") => Some(vec![EffectOp::DealDamage(10)]),
        // Vantom: fixed 4-move cycle, no RNG - Ink Blot (7) -> Inky Lance
        // (6 x2) -> Dismember (27 + three "Wound" cards) -> Prepare
        // (+2 Strength) -> repeat.
        ("Vantom", "Ink Blot") => Some(vec![EffectOp::DealDamage(7)]),
        ("Vantom", "Inky Lance") => Some(vec![
            EffectOp::DealDamage(6),
            EffectOp::DealDamage(6),
        ]),
        ("Vantom", "Dismember") => Some(vec![
            EffectOp::DealDamage(27),
            EffectOp::ApplyCardToTarget("Wound".to_string()),
            EffectOp::ApplyCardToTarget("Wound".to_string()),
            EffectOp::ApplyCardToTarget("Wound".to_string()),
        ]),
        ("Vantom", "Prepare") => Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))]),
        // Snapping Jaxfruit: Energy Orb — deal 3 damage, gain 2 Strength.
        ("Snapping Jaxfruit", "Energy Orb") => Some(vec![
            EffectOp::DealDamage(3),
            EffectOp::ApplyStatusToSelf(Status::Strength(2)),
        ]),
        // Axe Ruby Raider: Swing 1 (5 damage + 5 block), Swing 2 (5 damage + 5
        // block), Big Swing (12 damage, no block).
        ("Axe Ruby Raider", "Swing 1") => Some(vec![
            EffectOp::DealDamage(5),
            EffectOp::GainBlock(5),
        ]),
        ("Axe Ruby Raider", "Swing 2") => Some(vec![
            EffectOp::DealDamage(5),
            EffectOp::GainBlock(5),
        ]),
        ("Axe Ruby Raider", "Big Swing") => Some(vec![EffectOp::DealDamage(12)]),
        // Assassin Ruby Raider: Killshot — deal 11 damage.
        ("Assassin Ruby Raider", "Killshot") => Some(vec![EffectOp::DealDamage(11)]),
        // Brute Ruby Raider: Beat (7 damage), Roar (gain 3 Strength).
        ("Brute Ruby Raider", "Beat") => Some(vec![EffectOp::DealDamage(7)]),
        ("Brute Ruby Raider", "Roar") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(3))])
        }
        // Crossbow Ruby Raider: Reload (gain 3 block), Fire (14 damage).
        ("Crossbow Ruby Raider", "Reload") => Some(vec![EffectOp::GainBlock(3)]),
        ("Crossbow Ruby Raider", "Fire") => Some(vec![EffectOp::DealDamage(14)]),
        // Slithering Strangler (elite): Constrict applies 3 stacks, then
        // alternates Thwack/Lash with Constrict reapplied each time.
        ("Slithering Strangler", "Constrict") => {
            Some(vec![EffectOp::ApplyStatusToTarget(Status::Constrict(3))])
        }
        ("Slithering Strangler", "Thwack") => Some(vec![
            EffectOp::DealDamage(7),
            EffectOp::GainBlock(5),
        ]),
        ("Slithering Strangler", "Lash") => Some(vec![EffectOp::DealDamage(12)]),
        // Cubex Construct (elite): fixed cycle with two consecutive Repeater Blasts
        // then one Expel Blast.
        ("Cubex Construct", "Charge Up") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))])
        }
        ("Cubex Construct", "Repeater Blast") => Some(vec![
            EffectOp::DealDamage(7),
            EffectOp::ApplyStatusToSelf(Status::Strength(2)),
        ]),
        ("Cubex Construct", "Expel Blast") => Some(vec![
            EffectOp::DealDamage(5),
            EffectOp::DealDamage(5),
        ]),
        // Kin Priest: fixed 4-move cycle.
        ("Kin Priest", "Orb of Frailty") => Some(vec![
            EffectOp::DealDamage(8),
            EffectOp::ApplyStatusToTarget(Status::Frail(1)),
        ]),
        ("Kin Priest", "Orb of Weakness") => Some(vec![
            EffectOp::DealDamage(8),
            EffectOp::ApplyStatusToTarget(Status::Weak),
        ]),
        ("Kin Priest", "Soul Beam") => Some(vec![
            EffectOp::DealDamage(3),
            EffectOp::DealDamage(3),
            EffectOp::DealDamage(3),
        ]),
        ("Kin Priest", "Dark Ritual") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))])
        }
        // Kin Follower: fixed 3-move cycle.
        ("Kin Follower", "Quick Slash") => Some(vec![EffectOp::DealDamage(5)]),
        ("Kin Follower", "Boomerang") => Some(vec![
            EffectOp::DealDamage(2),
            EffectOp::DealDamage(2),
        ]),
        ("Kin Follower", "Power Dance") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))])
        }
        // Phrog Parasite (elite): fixed 2-move cycle — Infect (apply 3
        // Infection cards) ↔ Lash (4 hits of 4 damage).
        ("Phrog Parasite", "Infect") => Some(vec![
            EffectOp::ApplyCardToTarget("Infection".to_string()),
            EffectOp::ApplyCardToTarget("Infection".to_string()),
            EffectOp::ApplyCardToTarget("Infection".to_string()),
        ]),
        ("Phrog Parasite", "Lash") => Some(vec![
            EffectOp::DealDamage(4),
            EffectOp::DealDamage(4),
            EffectOp::DealDamage(4),
            EffectOp::DealDamage(4),
        ]),
        // Wriggler (summoned minion): fixed 2-move cycle — Nasty Bite (6
        // damage) ↔ Wriggle (1 Infection card + self +2 Strength).
        ("Wriggler", "Nasty Bite") => Some(vec![EffectOp::DealDamage(6)]),
        ("Wriggler", "Wriggle") => Some(vec![
            EffectOp::ApplyCardToTarget("Infection".to_string()),
            EffectOp::ApplyStatusToSelf(Status::Strength(2)),
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
        // Skull Bash applies Vulnerable permanently — repeating it is pointless
        // and the wiki confirms Nob never uses it twice in a row.
        ("Gremlin Nob", "Rush") => 2,
        ("Gremlin Nob", _) => 1,
        // Twig Slime (M)'s StickyShot can never repeat consecutively.
        ("Twig Slime (M)", "StickyShot") => 1,
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
        // Per the wiki, Gremlin Nob's post-opening pattern: 67% Rush, 33%
        // Skull Bash, never Bellow again, streak limits enforced as above.
        "Gremlin Nob" => loop {
            let roll = rng.gen_range(0..100);
            let candidate = if roll < 67 { "Rush" } else { "Skull Bash" };
            let resulting_streak = if last_move.as_deref() == Some(candidate) {
                streak + 1
            } else {
                1
            };
            if resulting_streak <= max_streak(monster_name, candidate) {
                return Some(candidate.to_string());
            }
        },
        // Nibbit cycles deterministically: Butt → Hesitant Slice → Hiss → Butt…
        "Nibbit" => match last_move.as_deref() {
            Some("Butt") => Some("Hesitant Slice".to_string()),
            Some("Hesitant Slice") => Some("Hiss".to_string()),
            _ => Some("Butt".to_string()),
        },
        // Fuzzy Wurm Crawler alternates: Acid Goop ↔ Inhale.
        "Fuzzy Wurm Crawler" => match last_move.as_deref() {
            Some("Acid Goop") => Some("Inhale".to_string()),
            _ => Some("Acid Goop".to_string()),
        },
        // Twig Slime (S) has a single move and repeats it forever.
        "Twig Slime (S)" => Some("Tackle".to_string()),
        // Shrinker Beetle: Shrink only ever opens the fight; afterward it
        // alternates Chomp <-> Stomp forever.
        "Shrinker Beetle" => match last_move.as_deref() {
            Some("Chomp") => Some("Stomp".to_string()),
            _ => Some("Chomp".to_string()),
        },
        // Leaf Slime (S) strictly alternates Tackle <-> Goop forever.
        "Leaf Slime (S)" => match last_move.as_deref() {
            Some("Tackle") => Some("Goop".to_string()),
            _ => Some("Tackle".to_string()),
        },
        // Leaf Slime (M) strictly alternates StickyShot <-> ClumpShot forever.
        "Leaf Slime (M)" => match last_move.as_deref() {
            Some("StickyShot") => Some("ClumpShot".to_string()),
            _ => Some("StickyShot".to_string()),
        },
        // Per the wiki, Twig Slime (M)'s post-opening pattern: 67% ClumpShot,
        // 33% StickyShot, with StickyShot never repeating consecutively
        // (enforced via `max_streak`).
        "Twig Slime (M)" => loop {
            let roll = rng.gen_range(0..100);
            let candidate = if roll < 67 { "ClumpShot" } else { "StickyShot" };
            let resulting_streak = if last_move.as_deref() == Some(candidate) {
                streak + 1
            } else {
                1
            };
            if resulting_streak <= max_streak(monster_name, candidate) {
                return Some(candidate.to_string());
            }
        },
        // Byrdonis strictly alternates Swoop <-> Peck forever.
        "Byrdonis" => match last_move.as_deref() {
            Some("Swoop") => Some("Peck".to_string()),
            _ => Some("Swoop".to_string()),
        },
        // Inklet: after Jab, randomly choose Piercing Gaze or Windup Punch
        // (50/50); after either of those, always return to Jab.
        "Inklet" => match last_move.as_deref() {
            Some("Windup Punch") | Some("Piercing Gaze") => Some("Jab".to_string()),
            _ => {
                let roll = rng.gen_range(0..100);
                if roll < 50 {
                    Some("Piercing Gaze".to_string())
                } else {
                    Some("Windup Punch".to_string())
                }
            }
        },
        // Vantom cycles deterministically: Ink Blot -> Inky Lance ->
        // Dismember -> Prepare -> Ink Blot...
        "Vantom" => match last_move.as_deref() {
            Some("Ink Blot") => Some("Inky Lance".to_string()),
            Some("Inky Lance") => Some("Dismember".to_string()),
            Some("Dismember") => Some("Prepare".to_string()),
            _ => Some("Ink Blot".to_string()),
        },
        // Snapping Jaxfruit: single move "Energy Orb" forever.
        "Snapping Jaxfruit" => Some("Energy Orb".to_string()),
        // Axe Ruby Raider: 3-move fixed cycle.
        "Axe Ruby Raider" => match last_move.as_deref() {
            Some("Swing 1") => Some("Swing 2".to_string()),
            Some("Swing 2") => Some("Big Swing".to_string()),
            _ => Some("Swing 1".to_string()),
        },
        // Assassin Ruby Raider: single move "Killshot" forever.
        "Assassin Ruby Raider" => Some("Killshot".to_string()),
        // Brute Ruby Raider: 2-move fixed cycle.
        "Brute Ruby Raider" => match last_move.as_deref() {
            Some("Beat") => Some("Roar".to_string()),
            _ => Some("Beat".to_string()),
        },
        // Crossbow Ruby Raider: Reload <-> Fire alternating, opens with Reload.
        "Crossbow Ruby Raider" => match last_move.as_deref() {
            Some("Reload") => Some("Fire".to_string()),
            _ => Some("Reload".to_string()),
        },
        // Slithering Strangler: Constrict (opening) -> random Thwack/Lash ->
        // Constrict -> random Thwack/Lash -> ... (Constrict reapplied every
        // other turn, so stacks accumulate: 3, 6, 9, ...).
        "Slithering Strangler" => match last_move.as_deref() {
            Some("Constrict") => {
                let roll = rng.gen_range(0..100);
                if roll < 50 {
                    Some("Thwack".to_string())
                } else {
                    Some("Lash".to_string())
                }
            }
            _ => Some("Constrict".to_string()),
        },
        // Cubex Construct: Charge Up (opening) → Repeater Blast → Repeater
        // Blast → Expel Blast → Repeater Blast → ...
        "Cubex Construct" => match last_move.as_deref() {
            Some("Charge Up") => Some("Repeater Blast".to_string()),
            Some("Repeater Blast") if streak >= 2 => Some("Expel Blast".to_string()),
            Some("Repeater Blast") => Some("Repeater Blast".to_string()),
            Some("Expel Blast") => Some("Repeater Blast".to_string()),
            _ => Some("Charge Up".to_string()),
        },
        // Kin Priest: fixed 4-move cycle: Orb of Frailty -> Orb of Weakness
        // -> Soul Beam -> Dark Ritual -> repeat.
        "Kin Priest" => match last_move.as_deref() {
            Some("Orb of Frailty") => Some("Orb of Weakness".to_string()),
            Some("Orb of Weakness") => Some("Soul Beam".to_string()),
            Some("Soul Beam") => Some("Dark Ritual".to_string()),
            _ => Some("Orb of Frailty".to_string()),
        },
        // Kin Follower: fixed 3-move cycle: Quick Slash -> Boomerang ->
        // Power Dance -> repeat.
        "Kin Follower" => match last_move.as_deref() {
            Some("Quick Slash") => Some("Boomerang".to_string()),
            Some("Boomerang") => Some("Power Dance".to_string()),
            _ => Some("Quick Slash".to_string()),
        },
        // Phrog Parasite: Infect ↔ Lash forever.
        "Phrog Parasite" => match last_move.as_deref() {
            Some("Infect") => Some("Lash".to_string()),
            _ => Some("Infect".to_string()),
        },
        // Wriggler: Nasty Bite ↔ Wriggle forever.
        "Wriggler" => match last_move.as_deref() {
            Some("Nasty Bite") => Some("Wriggle".to_string()),
            _ => Some("Nasty Bite".to_string()),
        },
        _ => None,
    }
}
