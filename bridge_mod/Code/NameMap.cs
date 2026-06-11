using System.Collections.Generic;

namespace sts_sim_bridge_mod;

/// <summary>
/// Translates STS2 model ids (CardModel.Id.Entry / Creature.ModelId.Entry /
/// PowerModel.Id.Entry) to the names sts_sim's CombatState/Monster/Status
/// constructors expect. Anything not listed here is unsupported and should
/// be dropped by the caller rather than sent to sts_sim.
/// </summary>
public static class NameMap
{
    /// STS2 CardModel.Id.Entry -> sts_sim card name.
    public static readonly Dictionary<string, string> CardNameMap = new()
    {
        ["STRIKE_IRONCLAD"] = "Strike",
        ["DEFEND_IRONCLAD"] = "Defend",
        ["BASH"] = "Bash",
        ["IRON_WAVE"] = "Iron Wave",
        ["INFLAME"] = "Inflame",
        ["SWORD_BOOMERANG"] = "Sword Boomerang",
        ["THUNDERCLAP"] = "Thunderclap",
        ["RAGE"] = "Rage",
        ["POMMEL_STRIKE"] = "Pommel Strike",
        // Status card the slime monsters' Goop/StickyShot moves stick into
        // the player's discard pile; sts_sim's card_data models it.
        ["SLIMED"] = "Slimed",
        // ASCENDERS_BANE (Curse) has no sts_sim equivalent; left unmapped so
        // it gets dropped from translated piles.
    };

    /// STS2 Creature.ModelId.Entry -> sts_sim monster name. Only monsters
    /// sts_sim's monsters.rs currently models are listed; everything else
    /// falls back to the generic placeholder (name=null, attack from intent).
    /// TwigSlimeS/TwigSlimeM/LeafSlimeS/LeafSlimeM match sts_sim names too,
    /// but aren't modeled in monsters.rs yet - add them here once they are.
    public static readonly Dictionary<string, string> MonsterNameMap = new()
    {
        ["NIBBIT"] = "Nibbit",
        ["FUZZY_WURM_CRAWLER"] = "Fuzzy Wurm Crawler",
        // Shrinker Beetle's "Shrink" self-debuff (-30% outgoing damage,
        // permanent) is applied by sts_sim's own move-resolution when it
        // simulates the Shrink move - no status translation needed here.
        ["SHRINKER_BEETLE"] = "Shrinker Beetle",
    };

    /// STS2 PowerModel.Id.Entry -> sts_sim status name. Only the statuses
    /// Status::from_name_and_amount understands are listed; other powers
    /// are silently dropped, matching that function's "unknown name -> empty
    /// vec" behavior.
    public static readonly Dictionary<string, string> PowerNameMap = new()
    {
        ["VULNERABLE"] = "Vulnerable",
        ["WEAK"] = "Weak",
        ["STRENGTH"] = "Strength",
    };
}
