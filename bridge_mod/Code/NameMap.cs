using System.Collections.Generic;

namespace sts_sim_bridge_mod;

/// <summary>
/// Translates STS2 PowerModel.Id.Entry ids to the status names
/// sts_sim's Status constructors expect.
/// Cards and monsters no longer require translation — StateBuilder sends
/// raw STS2 IDs directly; the Rust sim resolves them via from_sts2().
/// </summary>
public static class NameMap
{
    public static readonly Dictionary<string, string> PowerNameMap = new()
        ["RINGING"] = "Ringing",
    {
        ["VULNERABLE"] = "Vulnerable",
        ["WEAK"] = "Weak",
        ["STRENGTH"] = "Strength",
        // Vantom (x9) and Inklet (x1) both apply this to themselves on
        // AfterAddedToRoom; engine.rs's Slippery caps the next HP loss the
        // holder takes to 1 and consumes a stack.
        ["SLIPPERY"] = "Slippery",
        // NOTE: The STS2 PowerModel.Id.Entry string is derived from
        // StringHelper.Slugify(type.Name) where type is the C# class (e.g.
        // `VulnerablePower`). If the class name carries a `Power` suffix the
        // resulting id would be `VULNERABLE_POWER`, `STRENGTH_POWER`, etc.
        // The entries below assume NO `Power` suffix (matching the existing
        // Vulnerable/Weak/Strength/Slippery entries above) — if the live
        // bridge silently drops statuses, verify the actual id string before
        // adding new entries here.
        //
        // Frail: −25% Block gained. Applied by Kin Priest's Orb of Frailty.
        ["FRAIL"] = "Frail",
        // Artifact: negates next N debuff applications. Cubex Construct starts
        // with Artifact(1).
        ["ARTIFACT"] = "Artifact",
        // Constrict: end-of-turn unblockable damage, stacks additively.
        // Applied by Slithering Strangler's Constrict move.
        ["CONSTRICT"] = "Constrict",
        // Stun: monster skips next turn. Applied to spawned Wrigglers (Phrog
        // Parasite) and reusable for Ceremonial Beast's Plow self-stun.
        ["STUN"] = "Stun",
        // Tangled: Attack cards cost +n to play this turn. Removed at end of
        // player's turn. Applied by Vine Shambler's Grasping Vines.
        ["TANGLED"] = "Tangled",
        // Slow: Attack-card damage to the holder scales with
        // cards_played_this_turn. Inherent to Bygone Effigy (no decay, not a
        // debuff).
        ["SLOW"] = "Slow",
        // Plating: grants block at end of player turn, decrements after each
        // enemy turn.
        ["PLATING"] = "Plating",
        // Vicious: when the player self-applies Vulnerable, draw cards.
        ["VICIOUS"] = "Vicious",
        // Juggling: after the player's 3rd attack each turn, add copies to hand.
        ["JUGGLING"] = "Juggling",
        // Unmovable: the first N block gains per turn from cards are doubled.
        ["UNMOVABLE"] = "Unmovable",
        // Minion: { leader } — not straightforward to map from a simple
        // name+amount power; the `leader` field must be set explicitly. If
        // the bridge encounters a MinionPower, it should be dropped and the
        // caller should construct the Minion status manually with the correct
        // leader name instead.
        // ["MINION"] = "Minion",
    };

    /// sts_sim monster name -> python/sts_sim's
    /// `Encounter` key, for the "deck_baseline" command's `monster` field.
    /// Only single-monster encounters that `sts_sim.bench._SCENARIOS` models
    /// are listed; fights against anything else (multi-monster rooms, or a
    /// monster not modelled in the sim) the generic placeholder is used.
    public static readonly Dictionary<string, string> EncounterNameMap = new()
    {
        ["Nibbit"] = "nibbit",
        ["Fuzzy Wurm Crawler"] = "fuzzy-wurm-crawler",
        ["Shrinker Beetle"] = "shrinker-beetle",
        ["Leaf Slime (S)"] = "leaf-slime-s",
        ["Leaf Slime (M)"] = "leaf-slime-m",
        ["Twig Slime (S)"] = "twig-slime-s",
        ["Twig Slime (M)"] = "twig-slime-m",
        ["Byrdonis"] = "byrdonis",
        ["Inklet"] = "inklet",
        ["Vantom"] = "vantom",
        ["Snapping Jaxfruit"] = "snapping-jaxfruit",
        ["Axe Ruby Raider"] = "axe-ruby-raider",
        ["Assassin Ruby Raider"] = "assassin-ruby-raider",
        ["Brute Ruby Raider"] = "brute-ruby-raider",
        ["Crossbow Ruby Raider"] = "crossbow-ruby-raider",
        ["Slithering Strangler"] = "slithering-strangler",
        ["Cubex Construct"] = "cubex-construct",
        ["Tracker Ruby Raider"] = "tracker-ruby-raider",
        ["Mawler"] = "mawler",
        ["Vine Shambler"] = "vine-shambler",
        ["Bygone Effigy"] = "bygone-effigy",
        ["Flyconid"] = "flyconid",
        ["Fogmog"] = "fogmog",
        ["Ceremonial Beast"] = "ceremonial-beast",
    };
}
