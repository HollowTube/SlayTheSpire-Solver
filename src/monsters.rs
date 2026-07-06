use crate::engine::{EffectOp, Status};
use crate::ids::{CardId, MonsterId};
use rand::Rng;
use rand_pcg::Pcg32;

/// The move a monster telegraphs before it has acted at all — Slay the
/// Spire's documented monster patterns each name a fixed opening move (e.g.
/// Jaw Worm always opens with Chomp) before any RNG-driven selection kicks in.
pub(crate) fn opening_intent(monster_id: MonsterId) -> Option<String> {
    match monster_id {
        MonsterId::JawWorm => Some("Chomp".to_string()),
        MonsterId::GremlinNob => Some("Bellow".to_string()),
        MonsterId::Nibbit => Some("Butt".to_string()),
        MonsterId::FuzzyWurmCrawler => Some("Acid Goop".to_string()),
        MonsterId::TwigSlimeS => Some("Tackle".to_string()),
        MonsterId::ShrinkerBeetle => Some("Shrink".to_string()),
        // Per the wiki, Leaf Slime (S) opens with a 50/50 roll between Tackle
        // and Goop; `opening_intent` has no RNG access (mirrors every other
        // monster's fixed opener), so Tackle is picked deterministically —
        // it then strictly alternates with Goop forever regardless.
        MonsterId::LeafSlimeS => Some("Tackle".to_string()),
        // Leaf Slime (M) always opens with StickyShot.
        MonsterId::LeafSlimeM => Some("StickyShot".to_string()),
        // Twig Slime (M) always opens with StickyShot.
        MonsterId::TwigSlimeM => Some("StickyShot".to_string()),
        // Byrdonis always opens with Swoop.
        MonsterId::Byrdonis => Some("Swoop".to_string()),
        // Inklet always opens with Jab.
        MonsterId::Inklet => Some("Jab".to_string()),
        // Vantom always opens with Ink Blot.
        MonsterId::Vantom => Some("Ink Blot".to_string()),
        // Snapping Jaxfruit: single move "Energy Orb" forever.
        MonsterId::SnappingJaxfruit => Some("Energy Orb".to_string()),
        // Axe Ruby Raider: 3-move fixed cycle, opens with Swing 1.
        MonsterId::AxeRubyRaider => Some("Swing 1".to_string()),
        // Assassin Ruby Raider: single move "Killshot" forever.
        MonsterId::AssassinRubyRaider => Some("Killshot".to_string()),
        // Brute Ruby Raider: 2-move fixed cycle, opens with Beat.
        MonsterId::BruteRubyRaider => Some("Beat".to_string()),
        // Crossbow Ruby Raider: 2-move fixed cycle, opens with Reload.
        MonsterId::CrossbowRubyRaider => Some("Reload".to_string()),
        // Slithering Strangler: opens with Constrict.
        MonsterId::SlitheringStrangler => Some("Constrict".to_string()),
        // Cubex Construct: opens with Charge Up.
        MonsterId::CubexConstruct => Some("Charge Up".to_string()),
        // Kin Priest: fixed 4-move cycle, opens with Orb of Frailty.
        MonsterId::KinPriest => Some("Orb of Frailty".to_string()),
        // Kin Follower: fixed 3-move cycle, opens with Quick Slash.
        MonsterId::KinFollower => Some("Quick Slash".to_string()),
        // Phrog Parasite: fixed 2-move cycle, opens with Infect.
        MonsterId::PhrogParasite => Some("Infect".to_string()),
        // Wriggler: the opening intent is set via the intent= constructor
        // override (odd-index Wrigglers open with Nasty Bite, even-index open
        // with Wriggle). The default opener is Nasty Bite.
        MonsterId::Wriggler => Some("Nasty Bite".to_string()),
        // Tracker Ruby Raider: opens with Track (no damage, 2 Frail), then
        // Hounds forever.
        MonsterId::TrackerRubyRaider => Some("Track".to_string()),
        // Mawler (elite): opens with Claw (2x4 damage), then random branch.
        MonsterId::Mawler => Some("Claw".to_string()),
        // Vine Shambler (elite): opens with Swipe.
        MonsterId::VineShambler => Some("Swipe".to_string()),
        // Bygone Effigy (elite): opens with Sleep (no-op).
        MonsterId::BygoneEffigy => Some("Sleep".to_string()),
        // Flyconid: opening is a random 2:1 branch between Frail Spores and
        // Smash, handled in select_next_intent (none available turn 1).
        MonsterId::Flyconid => None,
        // Fogmog: always opens with Illusion (spawns Eye With Teeth).
        MonsterId::Fogmog => Some("Illusion".to_string()),
        // Ceremonial Beast: always opens with Stamp (applies Plow(150) to
        // self, no damage).
        MonsterId::CeremonialBeast => Some("Stamp".to_string()),
        // Eye With Teeth is a spawned minion with no intent-based AI — it
        // uses flat attack damage (set via Monster::new's attack parameter).
        MonsterId::EyeWithTeeth => None,
    }
}

/// A monster move's declarative effect pipeline — interpreted by the same
/// generic `run_effect_ops` engine as cards (with `Actor::Monster`), so that
/// adding an ordinary monster move means adding data here, not new logic.
pub(crate) fn monster_move(monster_id: MonsterId, move_name: &str) -> Option<Vec<EffectOp>> {
    match (monster_id, move_name) {
        // Per the Slay the Spire wiki, Jaw Worm's move pool:
        (MonsterId::JawWorm, "Chomp") => Some(vec![EffectOp::DealDamage(11)]),
        (MonsterId::JawWorm, "Thrash") => Some(vec![EffectOp::DealDamage(7), EffectOp::GainBlock(5)]),
        (MonsterId::JawWorm, "Bellow") => Some(vec![
            EffectOp::ApplyStatusToSelf(Status::Strength(3)),
            EffectOp::GainBlock(6),
        ]),
        // Per the Slay the Spire wiki, Gremlin Nob's move pool:
        // Bellow grants Enrage(2) — not Strength; Enrage triggers on Skill plays.
        (MonsterId::GremlinNob, "Bellow") => Some(vec![EffectOp::ApplyStatusToSelf(Status::Enrage(2))]),
        (MonsterId::GremlinNob, "Rush") => Some(vec![EffectOp::DealDamage(14)]),
        (MonsterId::GremlinNob, "Skull Bash") => Some(vec![
            EffectOp::DealDamage(6),
            EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            EffectOp::ApplyStatusToTarget(Status::Vulnerable),
        ]),
        // Nibbit: fixed 3-move cycle (Butt → Hesitant Slice → Hiss → repeat).
        (MonsterId::Nibbit, "Butt") => Some(vec![EffectOp::DealDamage(12)]),
        (MonsterId::Nibbit, "Hesitant Slice") => Some(vec![
            EffectOp::DealDamage(6),
            EffectOp::GainBlock(5),
        ]),
        (MonsterId::Nibbit, "Hiss") => Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))]),
        // Fuzzy Wurm Crawler: alternating Acid Goop / Inhale.
        (MonsterId::FuzzyWurmCrawler, "Acid Goop") => Some(vec![EffectOp::DealDamage(4)]),
        (MonsterId::FuzzyWurmCrawler, "Inhale") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(7))])
        }
        // Twig Slime (S): a single repeating Tackle for 4 damage.
        (MonsterId::TwigSlimeS, "Tackle") => Some(vec![EffectOp::DealDamage(4)]),
        // Shrinker Beetle: opens with Shrink (applies Status::Shrink to the
        // player, no damage), then alternates Chomp (7) <-> Stomp (13).
        (MonsterId::ShrinkerBeetle, "Shrink") => {
            Some(vec![EffectOp::ApplyStatusToTarget(Status::Shrink)])
        }
        (MonsterId::ShrinkerBeetle, "Chomp") => Some(vec![EffectOp::DealDamage(7)]),
        (MonsterId::ShrinkerBeetle, "Stomp") => Some(vec![EffectOp::DealDamage(13)]),
        // Leaf Slime (S): Tackle for 3, or Goop a "Slimed" card into the
        // player's discard pile (no damage).
        (MonsterId::LeafSlimeS, "Tackle") => Some(vec![EffectOp::DealDamage(3)]),
        (MonsterId::LeafSlimeS, "Goop") => {
            Some(vec![EffectOp::ApplyCardToTarget(CardId::Slimed)])
        }
        // Leaf Slime (M): StickyShot gives the player two "Slimed" cards (no
        // damage); ClumpShot deals 8.
        (MonsterId::LeafSlimeM, "StickyShot") => Some(vec![
            EffectOp::ApplyCardToTarget(CardId::Slimed),
            EffectOp::ApplyCardToTarget(CardId::Slimed),
        ]),
        (MonsterId::LeafSlimeM, "ClumpShot") => Some(vec![EffectOp::DealDamage(8)]),
        // Twig Slime (M): StickyShot gives the player one "Slimed" card (no
        // damage); ClumpShot deals 11.
        (MonsterId::TwigSlimeM, "StickyShot") => {
            Some(vec![EffectOp::ApplyCardToTarget(CardId::Slimed)])
        }
        (MonsterId::TwigSlimeM, "ClumpShot") => Some(vec![EffectOp::DealDamage(11)]),
        // Byrdonis: alternates Swoop (17) <-> Peck (3 hits of 3), and has
        // Territorial 1 - +1 Strength to itself at the end of every one of
        // its turns, regardless of which move it used. Baking the Strength
        // gain into the tail of each move's effects (after the damage ops)
        // means this turn's damage uses last turn's Strength, and the gain
        // is in place for next turn - matching TerritorialPower's AfterTurnEnd
        // hook.
        (MonsterId::Byrdonis, "Swoop") => Some(vec![
            EffectOp::DealDamage(17),
            EffectOp::ApplyStatusToSelf(Status::Strength(1)),
        ]),
        (MonsterId::Byrdonis, "Peck") => Some(vec![
            EffectOp::DealDamage(3),
            EffectOp::DealDamage(3),
            EffectOp::DealDamage(3),
            EffectOp::ApplyStatusToSelf(Status::Strength(1)),
        ]),
        // Inklet: Jab (3), Windup Punch (2 x3), Piercing Gaze (10).
        (MonsterId::Inklet, "Jab") => Some(vec![EffectOp::DealDamage(3)]),
        (MonsterId::Inklet, "Windup Punch") => Some(vec![
            EffectOp::DealDamage(2),
            EffectOp::DealDamage(2),
            EffectOp::DealDamage(2),
        ]),
        (MonsterId::Inklet, "Piercing Gaze") => Some(vec![EffectOp::DealDamage(10)]),
        // Vantom: fixed 4-move cycle, no RNG - Ink Blot (7) -> Inky Lance
        // (6 x2) -> Dismember (27 + three "Wound" cards) -> Prepare
        // (+2 Strength) -> repeat.
        (MonsterId::Vantom, "Ink Blot") => Some(vec![EffectOp::DealDamage(7)]),
        (MonsterId::Vantom, "Inky Lance") => Some(vec![
            EffectOp::DealDamage(6),
            EffectOp::DealDamage(6),
        ]),
        (MonsterId::Vantom, "Dismember") => Some(vec![
            EffectOp::DealDamage(27),
            EffectOp::ApplyCardToTarget(CardId::Wound),
            EffectOp::ApplyCardToTarget(CardId::Wound),
            EffectOp::ApplyCardToTarget(CardId::Wound),
        ]),
        (MonsterId::Vantom, "Prepare") => Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))]),
        // Snapping Jaxfruit: Energy Orb — deal 3 damage, gain 2 Strength.
        (MonsterId::SnappingJaxfruit, "Energy Orb") => Some(vec![
            EffectOp::DealDamage(3),
            EffectOp::ApplyStatusToSelf(Status::Strength(2)),
        ]),
        // Axe Ruby Raider: Swing 1 (5 damage + 5 block), Swing 2 (5 damage + 5
        // block), Big Swing (12 damage, no block).
        (MonsterId::AxeRubyRaider, "Swing 1") => Some(vec![
            EffectOp::DealDamage(5),
            EffectOp::GainBlock(5),
        ]),
        (MonsterId::AxeRubyRaider, "Swing 2") => Some(vec![
            EffectOp::DealDamage(5),
            EffectOp::GainBlock(5),
        ]),
        (MonsterId::AxeRubyRaider, "Big Swing") => Some(vec![EffectOp::DealDamage(12)]),
        // Assassin Ruby Raider: Killshot — deal 11 damage.
        (MonsterId::AssassinRubyRaider, "Killshot") => Some(vec![EffectOp::DealDamage(11)]),
        // Brute Ruby Raider: Beat (7 damage), Roar (gain 3 Strength).
        (MonsterId::BruteRubyRaider, "Beat") => Some(vec![EffectOp::DealDamage(7)]),
        (MonsterId::BruteRubyRaider, "Roar") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(3))])
        }
        // Crossbow Ruby Raider: Reload (gain 3 block), Fire (14 damage).
        (MonsterId::CrossbowRubyRaider, "Reload") => Some(vec![EffectOp::GainBlock(3)]),
        (MonsterId::CrossbowRubyRaider, "Fire") => Some(vec![EffectOp::DealDamage(14)]),
        // Slithering Strangler (elite): Constrict applies 3 stacks, then
        // alternates Thwack/Lash with Constrict reapplied each time.
        (MonsterId::SlitheringStrangler, "Constrict") => {
            Some(vec![EffectOp::ApplyStatusToTarget(Status::Constrict(3))])
        }
        (MonsterId::SlitheringStrangler, "Thwack") => Some(vec![
            EffectOp::DealDamage(7),
            EffectOp::GainBlock(5),
        ]),
        (MonsterId::SlitheringStrangler, "Lash") => Some(vec![EffectOp::DealDamage(12)]),
        // Cubex Construct (elite): fixed cycle with two consecutive Repeater Blasts
        // then one Expel Blast.
        (MonsterId::CubexConstruct, "Charge Up") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))])
        }
        (MonsterId::CubexConstruct, "Repeater Blast") => Some(vec![
            EffectOp::DealDamage(7),
            EffectOp::ApplyStatusToSelf(Status::Strength(2)),
        ]),
        (MonsterId::CubexConstruct, "Expel Blast") => Some(vec![
            EffectOp::DealDamage(5),
            EffectOp::DealDamage(5),
        ]),
        // Kin Priest: fixed 4-move cycle.
        (MonsterId::KinPriest, "Orb of Frailty") => Some(vec![
            EffectOp::DealDamage(8),
            EffectOp::ApplyStatusToTarget(Status::Frail(1)),
        ]),
        (MonsterId::KinPriest, "Orb of Weakness") => Some(vec![
            EffectOp::DealDamage(8),
            EffectOp::ApplyStatusToTarget(Status::Weak),
        ]),
        (MonsterId::KinPriest, "Soul Beam") => Some(vec![
            EffectOp::DealDamage(3),
            EffectOp::DealDamage(3),
            EffectOp::DealDamage(3),
        ]),
        (MonsterId::KinPriest, "Dark Ritual") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))])
        }
        // Kin Follower: fixed 3-move cycle.
        (MonsterId::KinFollower, "Quick Slash") => Some(vec![EffectOp::DealDamage(5)]),
        (MonsterId::KinFollower, "Boomerang") => Some(vec![
            EffectOp::DealDamage(2),
            EffectOp::DealDamage(2),
        ]),
        (MonsterId::KinFollower, "Power Dance") => {
            Some(vec![EffectOp::ApplyStatusToSelf(Status::Strength(2))])
        }
        // Phrog Parasite (elite): fixed 2-move cycle — Infect (apply 3
        // Infection cards) ↔ Lash (4 hits of 4 damage).
        (MonsterId::PhrogParasite, "Infect") => Some(vec![
            EffectOp::ApplyCardToTarget(CardId::Infection),
            EffectOp::ApplyCardToTarget(CardId::Infection),
            EffectOp::ApplyCardToTarget(CardId::Infection),
        ]),
        (MonsterId::PhrogParasite, "Lash") => Some(vec![
            EffectOp::DealDamage(4),
            EffectOp::DealDamage(4),
            EffectOp::DealDamage(4),
            EffectOp::DealDamage(4),
        ]),
        // Wriggler (summoned minion): fixed 2-move cycle — Nasty Bite (6
        // damage) ↔ Wriggle (1 Infection card + self +2 Strength).
        (MonsterId::Wriggler, "Nasty Bite") => Some(vec![EffectOp::DealDamage(6)]),
        (MonsterId::Wriggler, "Wriggle") => Some(vec![
            EffectOp::ApplyCardToTarget(CardId::Infection),
            EffectOp::ApplyStatusToSelf(Status::Strength(2)),
        ]),
        // Tracker Ruby Raider: Track applies 2 Frail, Hounds is 8 hits of 1.
        (MonsterId::TrackerRubyRaider, "Track") => Some(vec![
            EffectOp::ApplyStatusToTarget(Status::Frail(1)),
            EffectOp::ApplyStatusToTarget(Status::Frail(1)),
        ]),
        (MonsterId::TrackerRubyRaider, "Hounds") => Some(vec![
            EffectOp::DealDamage(1),
            EffectOp::DealDamage(1),
            EffectOp::DealDamage(1),
            EffectOp::DealDamage(1),
            EffectOp::DealDamage(1),
            EffectOp::DealDamage(1),
            EffectOp::DealDamage(1),
            EffectOp::DealDamage(1),
        ]),
        // Mawler (elite): Claw is 2x4, Rip and Tear is 14, Roar is 3
        // Vulnerable.
        (MonsterId::Mawler, "Claw") => Some(vec![
            EffectOp::DealDamage(4),
            EffectOp::DealDamage(4),
        ]),
        (MonsterId::Mawler, "Rip and Tear") => Some(vec![EffectOp::DealDamage(14)]),
        (MonsterId::Mawler, "Roar") => Some(vec![
            EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            EffectOp::ApplyStatusToTarget(Status::Vulnerable),
        ]),
        // Vine Shambler (elite): fixed 3-move cycle — Swipe (2x6), Grasping
        // Vines (8 + Tangled), Chomp (16).
        (MonsterId::VineShambler, "Swipe") => Some(vec![
            EffectOp::DealDamage(6),
            EffectOp::DealDamage(6),
        ]),
        (MonsterId::VineShambler, "Grasping Vines") => Some(vec![
            EffectOp::DealDamage(8),
            EffectOp::ApplyStatusToTarget(Status::Tangled(1)),
        ]),
        (MonsterId::VineShambler, "Chomp") => Some(vec![EffectOp::DealDamage(16)]),
        // Bygone Effigy (elite): Sleep (no-op), Wake (+10 Strength),
        // Slashes (13 damage).
        (MonsterId::BygoneEffigy, "Sleep") => Some(vec![]),
        (MonsterId::BygoneEffigy, "Wake") => Some(vec![
            EffectOp::ApplyStatusToSelf(Status::Strength(10)),
        ]),
        (MonsterId::BygoneEffigy, "Slashes") => Some(vec![EffectOp::DealDamage(13)]),
        // Flyconid (elite): Frail Spores (8 damage + 2 Frail), Vulnerable
        // Spores (2 Vulnerable, no damage), Smash (11 damage).
        (MonsterId::Flyconid, "Frail Spores") => Some(vec![
            EffectOp::DealDamage(8),
            EffectOp::ApplyStatusToTarget(Status::Frail(1)),
            EffectOp::ApplyStatusToTarget(Status::Frail(1)),
        ]),
        (MonsterId::Flyconid, "Vulnerable Spores") => Some(vec![
            EffectOp::ApplyStatusToTarget(Status::Vulnerable),
            EffectOp::ApplyStatusToTarget(Status::Vulnerable),
        ]),
        (MonsterId::Flyconid, "Smash") => Some(vec![EffectOp::DealDamage(11)]),
        // Fogmog (Overgrowth normal): Illusion spawns Eye With Teeth (6 HP),
        // Swipe deals 8 damage + self-Str, Headbutt deals 14 damage.
        (MonsterId::Fogmog, "Illusion") => Some(vec![EffectOp::SpawnMonster("Eye With Teeth".to_string(), 6)]),
        (MonsterId::Fogmog, "Swipe") => Some(vec![
            EffectOp::DealDamage(8),
            EffectOp::ApplyStatusToSelf(Status::Strength(1)),
        ]),
        (MonsterId::Fogmog, "Headbutt") => Some(vec![EffectOp::DealDamage(14)]),
        // Ceremonial Beast: Phase 1 — Stamp applies Plow(150) to self
        // (no damage), then Plow loops forever (18 damage + 2 Str).
        (MonsterId::CeremonialBeast, "Stamp") => Some(vec![
            EffectOp::ApplyStatusToSelf(Status::Plow(150)),
        ]),
        (MonsterId::CeremonialBeast, "Plow") => Some(vec![
            EffectOp::DealDamage(18),
            EffectOp::ApplyStatusToSelf(Status::Strength(2)),
        ]),
        // Ceremonial Beast Phase 2: Stun (skip), Beast Cry (Ringing),
        // Stomp (15 dmg), Crush (17 dmg + 3 Str).
        (MonsterId::CeremonialBeast, "Stun") => Some(vec![]),
        (MonsterId::CeremonialBeast, "Beast Cry") => Some(vec![
            EffectOp::ApplyStatusToTarget(Status::Ringing),
        ]),
        (MonsterId::CeremonialBeast, "Stomp") => Some(vec![EffectOp::DealDamage(15)]),
        (MonsterId::CeremonialBeast, "Crush") => Some(vec![
            EffectOp::DealDamage(17),
            EffectOp::ApplyStatusToSelf(Status::Strength(3)),
        ]),
        _ => None,
    }
}

/// How many times a move may occur back-to-back before the AI is forced to
/// pick something else — per the wiki, Jaw Worm can't repeat Chomp or Bellow
/// at all, but may Thrash up to twice in a row (i.e. not a 3rd time).
fn max_streak(monster_id: MonsterId, move_name: &str) -> u32 {
    match (monster_id, move_name) {
        (MonsterId::JawWorm, "Thrash") => 2,
        (MonsterId::JawWorm, _) => 1,
        // Skull Bash applies Vulnerable permanently — repeating it is pointless
        // and the wiki confirms Nob never uses it twice in a row.
        (MonsterId::GremlinNob, "Rush") => 2,
        (MonsterId::GremlinNob, _) => 1,
        // Twig Slime (M)'s StickyShot can never repeat consecutively.
        (MonsterId::TwigSlimeM, "StickyShot") => 1,
        // Mawler: Rip and Tear and Claw never repeat consecutively.
        (MonsterId::Mawler, "Rip and Tear") => 1,
        (MonsterId::Mawler, "Claw") => 1,
        // Mawler's Roar is "use only once" per combat — handled via
        // moves_used, not max_streak. Streak limit here is 1 to prevent
        // consecutive repeats if moved out of moves_used somehow.
        (MonsterId::Mawler, "Roar") => 1,
        // Flyconid (elite): none of its moves can repeat consecutively.
        (MonsterId::Flyconid, "Vulnerable Spores") => 1,
        (MonsterId::Flyconid, "Frail Spores") => 1,
        (MonsterId::Flyconid, "Smash") => 1,
        // Fogmog: Swipe can appear twice (forced → random), but a third
        // consecutive Swipe is blocked. Headbutt's forced follow-up is Swipe
        // so Headbutt→Headbutt never happens naturally; 1 is safe.
        (MonsterId::Fogmog, "Swipe") => 2,
        (MonsterId::Fogmog, "Headbutt") => 1,
        _ => u32::MAX,
    }
}

/// Whether a move can only be used once across the entire combat (not just
/// "not twice in a row"). Once executed, the move is added to the monster's
/// `moves_used` set and `select_next_intent` will never return it again.
/// Currently only Mawler's Roar uses this.
pub(crate) fn is_one_time_move(monster_id: MonsterId, move_name: &str) -> bool {
    matches!(
        (monster_id, move_name),
        (MonsterId::Mawler, "Roar") | (MonsterId::CeremonialBeast, "Stun")
    )
}

/// Rolls the monster's next telegraphed move from its documented weighted
/// pattern, re-rolling whenever the result would extend a same-move streak
/// past its limit — e.g. Jaw Worm picks Bellow 45% / Thrash 30% / Chomp 25%
/// of the time, but never repeats Bellow/Chomp and never Thrashes a 3rd
/// consecutive time. `last_move`/`streak` describe the run of moves leading
/// up to (and including) the one that just resolved.
pub(crate) fn select_next_intent(
    monster_id: MonsterId,
    last_move: &Option<String>,
    streak: u32,
    moves_used: &[String],
    rng: &mut Pcg32,
) -> Option<String> {
    match monster_id {
        MonsterId::JawWorm => loop {
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
            if resulting_streak <= max_streak(monster_id, candidate) {
                return Some(candidate.to_string());
            }
        },
        // Per the wiki, Gremlin Nob's post-opening pattern: 67% Rush, 33%
        // Skull Bash, never Bellow again, streak limits enforced as above.
        MonsterId::GremlinNob => loop {
            let roll = rng.gen_range(0..100);
            let candidate = if roll < 67 { "Rush" } else { "Skull Bash" };
            let resulting_streak = if last_move.as_deref() == Some(candidate) {
                streak + 1
            } else {
                1
            };
            if resulting_streak <= max_streak(monster_id, candidate) {
                return Some(candidate.to_string());
            }
        },
        // Nibbit cycles deterministically: Butt → Hesitant Slice → Hiss → Butt…
        MonsterId::Nibbit => match last_move.as_deref() {
            Some("Butt") => Some("Hesitant Slice".to_string()),
            Some("Hesitant Slice") => Some("Hiss".to_string()),
            _ => Some("Butt".to_string()),
        },
        // Fuzzy Wurm Crawler alternates: Acid Goop ↔ Inhale.
        MonsterId::FuzzyWurmCrawler => match last_move.as_deref() {
            Some("Acid Goop") => Some("Inhale".to_string()),
            _ => Some("Acid Goop".to_string()),
        },
        // Twig Slime (S) has a single move and repeats it forever.
        MonsterId::TwigSlimeS => Some("Tackle".to_string()),
        // Shrinker Beetle: Shrink only ever opens the fight; afterward it
        // alternates Chomp <-> Stomp forever.
        MonsterId::ShrinkerBeetle => match last_move.as_deref() {
            Some("Chomp") => Some("Stomp".to_string()),
            _ => Some("Chomp".to_string()),
        },
        // Leaf Slime (S) strictly alternates Tackle <-> Goop forever.
        MonsterId::LeafSlimeS => match last_move.as_deref() {
            Some("Tackle") => Some("Goop".to_string()),
            _ => Some("Tackle".to_string()),
        },
        // Leaf Slime (M) strictly alternates StickyShot <-> ClumpShot forever.
        MonsterId::LeafSlimeM => match last_move.as_deref() {
            Some("StickyShot") => Some("ClumpShot".to_string()),
            _ => Some("StickyShot".to_string()),
        },
        // Per the wiki, Twig Slime (M)'s post-opening pattern: 67% ClumpShot,
        // 33% StickyShot, with StickyShot never repeating consecutively
        // (enforced via `max_streak`).
        MonsterId::TwigSlimeM => loop {
            let roll = rng.gen_range(0..100);
            let candidate = if roll < 67 { "ClumpShot" } else { "StickyShot" };
            let resulting_streak = if last_move.as_deref() == Some(candidate) {
                streak + 1
            } else {
                1
            };
            if resulting_streak <= max_streak(monster_id, candidate) {
                return Some(candidate.to_string());
            }
        },
        // Byrdonis strictly alternates Swoop <-> Peck forever.
        MonsterId::Byrdonis => match last_move.as_deref() {
            Some("Swoop") => Some("Peck".to_string()),
            _ => Some("Swoop".to_string()),
        },
        // Inklet: after Jab, randomly choose Piercing Gaze or Windup Punch
        // (50/50); after either of those, always return to Jab.
        MonsterId::Inklet => match last_move.as_deref() {
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
        MonsterId::Vantom => match last_move.as_deref() {
            Some("Ink Blot") => Some("Inky Lance".to_string()),
            Some("Inky Lance") => Some("Dismember".to_string()),
            Some("Dismember") => Some("Prepare".to_string()),
            _ => Some("Ink Blot".to_string()),
        },
        // Snapping Jaxfruit: single move "Energy Orb" forever.
        MonsterId::SnappingJaxfruit => Some("Energy Orb".to_string()),
        // Axe Ruby Raider: 3-move fixed cycle.
        MonsterId::AxeRubyRaider => match last_move.as_deref() {
            Some("Swing 1") => Some("Swing 2".to_string()),
            Some("Swing 2") => Some("Big Swing".to_string()),
            _ => Some("Swing 1".to_string()),
        },
        // Assassin Ruby Raider: single move "Killshot" forever.
        MonsterId::AssassinRubyRaider => Some("Killshot".to_string()),
        // Brute Ruby Raider: 2-move fixed cycle.
        MonsterId::BruteRubyRaider => match last_move.as_deref() {
            Some("Beat") => Some("Roar".to_string()),
            _ => Some("Beat".to_string()),
        },
        // Crossbow Ruby Raider: Reload <-> Fire alternating, opens with Reload.
        MonsterId::CrossbowRubyRaider => match last_move.as_deref() {
            Some("Reload") => Some("Fire".to_string()),
            _ => Some("Reload".to_string()),
        },
        // Slithering Strangler: Constrict (opening) -> random Thwack/Lash ->
        // Constrict -> random Thwack/Lash -> ... (Constrict reapplied every
        // other turn, so stacks accumulate: 3, 6, 9, ...).
        MonsterId::SlitheringStrangler => match last_move.as_deref() {
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
        MonsterId::CubexConstruct => match last_move.as_deref() {
            Some("Charge Up") => Some("Repeater Blast".to_string()),
            Some("Repeater Blast") if streak >= 2 => Some("Expel Blast".to_string()),
            Some("Repeater Blast") => Some("Repeater Blast".to_string()),
            Some("Expel Blast") => Some("Repeater Blast".to_string()),
            _ => Some("Charge Up".to_string()),
        },
        // Kin Priest: fixed 4-move cycle: Orb of Frailty -> Orb of Weakness
        // -> Soul Beam -> Dark Ritual -> repeat.
        MonsterId::KinPriest => match last_move.as_deref() {
            Some("Orb of Frailty") => Some("Orb of Weakness".to_string()),
            Some("Orb of Weakness") => Some("Soul Beam".to_string()),
            Some("Soul Beam") => Some("Dark Ritual".to_string()),
            _ => Some("Orb of Frailty".to_string()),
        },
        // Kin Follower: fixed 3-move cycle: Quick Slash -> Boomerang ->
        // Power Dance -> repeat.
        MonsterId::KinFollower => match last_move.as_deref() {
            Some("Quick Slash") => Some("Boomerang".to_string()),
            Some("Boomerang") => Some("Power Dance".to_string()),
            _ => Some("Quick Slash".to_string()),
        },
        // Phrog Parasite: Infect ↔ Lash forever.
        MonsterId::PhrogParasite => match last_move.as_deref() {
            Some("Infect") => Some("Lash".to_string()),
            _ => Some("Infect".to_string()),
        },
        // Wriggler: Nasty Bite ↔ Wriggle forever.
        MonsterId::Wriggler => match last_move.as_deref() {
            Some("Nasty Bite") => Some("Wriggle".to_string()),
            _ => Some("Nasty Bite".to_string()),
        },
        // Tracker Ruby Raider: Track once, then Hounds forever.
        MonsterId::TrackerRubyRaider => match last_move.as_deref() {
            Some("Track") => Some("Hounds".to_string()),
            _ => Some("Hounds".to_string()),
        },
        // Mawler (elite): opening Claw, then equal-weight random among three
        // moves. Roar is "use only once" per combat (checked via moves_used).
        // Rip and Tear and Claw are "cannot repeat" (max_streak 1).
        MonsterId::Mawler => {
            let candidates = ["Rip and Tear", "Roar", "Claw"];
            loop {
                let available: Vec<_> = candidates
                    .iter()
                    .filter(|c| {
                        // Skip one-time moves already used.
                        if moves_used.iter().any(|m| m == *c) {
                            return false;
                        }
                        // Skip moves that would exceed their streak limit.
                        let resulting_streak = if last_move.as_deref() == Some(*c) {
                            streak + 1
                        } else {
                            1
                        };
                        resulting_streak <= max_streak(monster_id, c)
                    })
                    .copied()
                    .collect();
                if available.is_empty() {
                    // Fallback: just repeat Claw (shouldn't happen in
                    // practice if the spec guarantees at least one move is
                    // always valid).
                    return Some("Claw".to_string());
                }
                let idx = rng.gen_range(0..available.len());
                return Some(available[idx].to_string());
            }
        }
        // Vine Shambler (elite): fixed 3-cycle — Swipe → Grasping Vines →
        // Chomp → repeat.
        MonsterId::VineShambler => match last_move.as_deref() {
            Some("Swipe") => Some("Grasping Vines".to_string()),
            Some("Grasping Vines") => Some("Chomp".to_string()),
            Some("Chomp") => Some("Swipe".to_string()),
            _ => Some("Swipe".to_string()),
        },
        // Bygone Effigy (elite): fixed cycle — Sleep → Wake → Slashes → Slashes
        // → Slashes → ... (after Wake, always Slashes).
        MonsterId::BygoneEffigy => match last_move.as_deref() {
            Some("Sleep") => Some("Wake".to_string()),
            _ => Some("Slashes".to_string()),
        },
        // Flyconid: opening is a random 2:1 branch (FrailSpores:Smash),
        // VulnerableSpores not available on turn 1.
        MonsterId::Flyconid => {
            if last_move.is_none() {
                let roll = rng.gen_range(0..3);
                return Some(if roll < 2 { "Frail Spores" } else { "Smash" }.to_string());
            }
            // Post-opening weighted random. All moves have max_streak=1
            // (cannot repeat consecutively). Weights: Vulnerable Spores 3,
            // Frail Spores 2, Smash 1 (total 6).
            loop {
                let roll = rng.gen_range(0..6);
                let candidate = if roll < 3 {
                    "Vulnerable Spores"
                } else if roll < 5 {
                    "Frail Spores"
                } else {
                    "Smash"
                };
                let resulting_streak = if last_move.as_deref() == Some(candidate) {
                    streak + 1
                } else {
                    1
                };
                if resulting_streak <= max_streak(monster_id, candidate) {
                    return Some(candidate.to_string());
                }
            }
        },
        // Fogmog: Illusion → Swipe (forced). Headbutt → Swipe (forced).
        // After any Swipe: random branch 40% Swipe / 60% Headbutt,
        // CannotRepeat (max_streak=1) forces alternation after first pick.
        MonsterId::Fogmog => match last_move.as_deref() {
            Some("Illusion") | Some("Headbutt") => Some("Swipe".to_string()),
            _ => loop {
                let roll = rng.gen_range(0..5);
                let candidate = if roll < 2 { "Swipe" } else { "Headbutt" };
                let resulting_streak = if last_move.as_deref() == Some(candidate) {
                    streak + 1
                } else {
                    1
                };
                if resulting_streak <= max_streak(monster_id, candidate) {
                    return Some(candidate.to_string());
                }
            },
        },
        // Ceremonial Beast: Phase 1 — Stamp → Plow → Plow → ... (forever).
        // Phase 2 activates after Stun appears in moves_used (see issue).
        MonsterId::CeremonialBeast => {
            let phase2 = moves_used.iter().any(|m| m == "Stun");
            if phase2 {
                match last_move.as_deref() {
                    Some("Beast Cry") => Some("Stomp".to_string()),
                    Some("Stomp") => Some("Crush".to_string()),
                    _ => Some("Beast Cry".to_string()),
                }
            } else {
                Some("Plow".to_string())
            }
        },
        // Eye With Teeth is a spawned minion with no intent-based AI — it
        // uses flat attack damage. This arm should never be reached since
        // the EndTurn loop skips unnamed monsters, but it's here for
        // exhaustiveness.
        MonsterId::EyeWithTeeth => None,
    }
}
