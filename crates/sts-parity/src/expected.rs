//! Known divergences, each with the issue that tracks it.

/// `card` carries a `+` for the upgraded variant; `field` is a
/// [`crate::ParityDiff`] path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expected {
    pub card: &'static str,
    pub field: &'static str,
    pub issue: &'static str,
}

const fn e(card: &'static str, field: &'static str, issue: &'static str) -> Expected {
    Expected { card, field, issue }
}

/// HOL-136: empty-effect stub cards. HOL-138: `LoseHp` is not counted as hp
/// lost. HOL-139: Mangle. HOL-140: Crimson Mantle and the upgraded power
/// amounts. HOL-141 to HOL-146: one sim gap each (see the issues).
pub const EXPECTED: &[Expected] = &[
    e("Aggression", "hand", "HOL-141"),
    e("Aggression+", "hand", "HOL-141"),
    e("CrimsonMantle", "player.hp", "HOL-140"),
    e("CrimsonMantle", "player.powers[CrimsonMantle]", "HOL-140"),
    e("CrimsonMantle+", "player.block", "HOL-140"),
    e("CrimsonMantle+", "player.hp", "HOL-140"),
    e("CrimsonMantle+", "player.powers[CrimsonMantle]", "HOL-140"),
    e("Cruelty+", "enemies[0].block", "HOL-140"),
    e("Cruelty+", "player.powers[Cruelty]", "HOL-140"),
    e("DemonForm+", "player.powers[DemonForm]", "HOL-140"),
    e("DemonForm+", "player.powers[Strength]", "HOL-140"),
    e("Expect A Fight", "player.energy", "HOL-136"),
    e(
        "Expect A Fight",
        "player.powers[NoEnergyGainPower]",
        "HOL-136",
    ),
    e("Expect A Fight+", "player.energy", "HOL-136"),
    e(
        "Expect A Fight+",
        "player.powers[NoEnergyGainPower]",
        "HOL-136",
    ),
    e("FeelNoPain+", "player.block", "HOL-140"),
    e("FeelNoPain+", "player.powers[FeelNoPain]", "HOL-140"),
    e("FlameBarrier", "enemies[0].hp", "HOL-143"),
    e("FlameBarrier+", "enemies[0].hp", "HOL-143"),
    e("FlameBarrier+", "player.powers[FlameBarrier]", "HOL-140"),
    e("Headbutt", "discard", "HOL-144"),
    e("Headbutt", "draw", "HOL-144"),
    e("Headbutt+", "discard", "HOL-144"),
    e("Headbutt+", "draw", "HOL-144"),
    e("Hellraiser", "player.powers[HellraiserPower]", "HOL-136"),
    e("Hellraiser+", "player.powers[HellraiserPower]", "HOL-136"),
    e("HowlFromBeyond/trigger", "enemies[0].block", "HOL-146"),
    e("HowlFromBeyond/trigger", "enemies[0].hp", "HOL-146"),
    e("HowlFromBeyond/trigger", "enemies[1].hp", "HOL-146"),
    e("HowlFromBeyond/trigger", "exhaust", "HOL-146"),
    e("HowlFromBeyond/trigger", "hand", "HOL-146"),
    e("Inferno+", "enemies[0].hp", "HOL-140"),
    e("Inferno+", "enemies[1].hp", "HOL-140"),
    e("Inferno+", "player.powers[Inferno]", "HOL-140"),
    e("Mangle", "enemies[0].powers[ManglePower]", "HOL-139"),
    e("Mangle+", "enemies[0].powers[ManglePower]", "HOL-139"),
    e("Pillage", "draw", "HOL-136"),
    e("Pillage", "enemies[0].block", "HOL-136"),
    e("Pillage", "hand", "HOL-136"),
    e("Pillage+", "draw", "HOL-136"),
    e("Pillage+", "enemies[0].block", "HOL-136"),
    e("Pillage+", "hand", "HOL-136"),
    e("Primal Force", "hand", "HOL-136"),
    e("Primal Force+", "hand", "HOL-136"),
    e("Pyre", "player.energy", "HOL-145"),
    e("Pyre+", "player.energy", "HOL-145"),
    e("Pyre+", "player.powers[Pyre]", "HOL-140"),
    e("Rage+", "player.block", "HOL-140"),
    e("Rage+", "player.powers[Rage]", "HOL-140"),
    e("Rupture", "player.powers[RupturePower]", "HOL-136"),
    e("Rupture+", "player.powers[RupturePower]", "HOL-136"),
    e("Spite", "enemies[0].block", "HOL-138"),
    e("Spite+", "enemies[0].block", "HOL-138"),
    e("Stampede", "player.powers[StampedePower]", "HOL-136"),
    e("Stampede+", "player.powers[StampedePower]", "HOL-136"),
    e("Stoke", "exhaust", "HOL-136"),
    e("Stoke+", "exhaust", "HOL-136"),
    e("TearAsunder", "enemies[0].block", "HOL-138"),
    e("TearAsunder+", "enemies[0].block", "HOL-138"),
    e("Vicious", "draw", "HOL-142"),
    e("Vicious", "hand", "HOL-142"),
    e("Vicious+", "draw", "HOL-142"),
    e("Vicious+", "hand", "HOL-142"),
];
