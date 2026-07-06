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
        ["COLOSSUS"] = "Colossus",
        // Status card the slime monsters' Goop/StickyShot moves stick into
        // the player's discard pile; sts_sim's card_data models it.
        ["SLIMED"] = "Slimed",
        // Vantom's Dismember move sticks these into the player's discard
        // pile; sts_sim's card_data models it identically to Slimed.
        ["WOUND"] = "Wound",
        // The remaining entries are every other card src/cards.rs's
        // card_data models (mostly Ironclad card-reward pool), keyed by
        // StringHelper.Slugify(type.Name) of each card's STS2 class.
        ["AGGRESSION"] = "Aggression",
        ["ASHEN_STRIKE"] = "AshenStrike",
        ["BARRICADE"] = "Barricade",
        ["BLOOD_WALL"] = "BloodWall",
        ["BLOODLETTING"] = "Bloodletting",
        ["BLUDGEON"] = "Bludgeon",
        ["BODY_SLAM"] = "BodySlam",
        ["BREAK"] = "Break",
        ["BREAKTHROUGH"] = "Breakthrough",
        ["BULLY"] = "Bully",
        ["BURNING_PACT"] = "BurningPact",
        ["CINDER"] = "Cinder",
        ["CONFLAGRATION"] = "Conflagration",
        ["CORRUPTION"] = "Corruption",
        ["CRIMSON_MANTLE"] = "CrimsonMantle",
        ["CRUELTY"] = "Cruelty",
        ["DARK_EMBRACE"] = "DarkEmbrace",
        ["DAZED"] = "Dazed",
        ["DEMON_FORM"] = "DemonForm",
        ["DISMANTLE"] = "Dismantle",
        ["DOMINATE"] = "Dominate",
        ["EVIL_EYE"] = "Evil Eye",
        ["FEEL_NO_PAIN"] = "FeelNoPain",
        ["FIEND_FIRE"] = "FiendFire",
        ["FLAME_BARRIER"] = "FlameBarrier",
        ["FORGOTTEN_RITUAL"] = "Forgotten Ritual",
        ["HEADBUTT"] = "Headbutt",
        ["HEMOKINESIS"] = "Hemokinesis",
        ["IMPERVIOUS"] = "Impervious",
        ["INFERNAL_BLADE"] = "InfernalBlade",
        ["INFERNO"] = "Inferno",
        ["JUGGERNAUT"] = "Juggernaut",
        ["MANGLE"] = "Mangle",
        ["MOLTEN_FIST"] = "MoltenFist",
        ["NOT_YET"] = "NotYet",
        ["OFFERING"] = "Offering",
        ["ONE_TWO_PUNCH"] = "OneTwoPunch",
        ["PERFECTED_STRIKE"] = "PerfectedStrike",
        ["SECOND_WIND"] = "SecondWind",
        ["SETUP_STRIKE"] = "Setup Strike",
        ["SHRUG_IT_OFF"] = "ShrugItOff",
        ["SPITE"] = "Spite",
        ["TAUNT"] = "Taunt",
        ["TEAR_ASUNDER"] = "TearAsunder",
        ["THRASH"] = "Thrash",
        ["TREMBLE"] = "Tremble",
        ["TRUE_GRIT"] = "TrueGrit",
        ["TWIN_STRIKE"] = "TwinStrike",
        ["UNRELENTING"] = "Unrelenting",
        ["UPPERCUT"] = "Uppercut",
        // Everything below has no src/cards.rs `card_data` entry yet (no
        // sts_sim effects implemented), but mapping the STS2 id to its class
        // name still lets these cards flow through to sts_sim instead of
        // tripping the overlay's "unknown card" warning. sts_sim's
        // `card_data(name)` returns None for these, so the engine just
        // treats them as inert/unplayable cards sitting in a pile (matching
        // how an un-castable curse/status behaves) until they're modeled.
        //
        // -- Ironclad card pool --
        ["ANGER"] = "Anger",
        ["ARMAMENTS"] = "Armaments",
        ["BRAND"] = "Brand",
        ["DEMONIC_SHIELD"] = "DemonicShield",
        ["DRUM_OF_BATTLE"] = "DrumOfBattle",
        ["EXPECT_A_FIGHT"] = "ExpectAFight",
        ["FEED"] = "Feed",
        ["FIGHT_ME"] = "FightMe",
        ["HELLRAISER"] = "Hellraiser",
        ["HOWL_FROM_BEYOND"] = "HowlFromBeyond",
        ["STAMPEDE"] = "Stampede",
        ["RUPTURE"] = "Rupture",
        ["HELLRAISER"] = "Hellraiser",
        ["EXPECT_A_FIGHT"] = "ExpectAFight",
        ["PILLAGE"] = "Pillage",
        ["JUGGLING"] = "Juggling",
        ["PACTS_END"] = "PactsEnd",
        ["PILLAGE"] = "Pillage",
        ["PRIMAL_FORCE"] = "PrimalForce",
        ["PYRE"] = "Pyre",
        ["RAMPAGE"] = "Rampage",
        ["RUPTURE"] = "Rupture",
        ["STAMPEDE"] = "Stampede",
        ["STOKE"] = "Stoke",
        ["STOMP"] = "Stomp",
        ["STONE_ARMOR"] = "StoneArmor",
        ["TANK"] = "Tank",
        ["UNMOVABLE"] = "Unmovable",
        ["VICIOUS"] = "Vicious",
        // -- Colorless card pool --
        ["ALCHEMIZE"] = "Alchemize",
        ["ANOINTED"] = "Anointed",
        ["AUTOMATION"] = "Automation",
        ["BEACON_OF_HOPE"] = "BeaconOfHope",
        ["BEAT_DOWN"] = "BeatDown",
        ["BELIEVE_IN_YOU"] = "BelieveInYou",
        ["BOLAS"] = "Bolas",
        ["CALAMITY"] = "Calamity",
        ["CATASTROPHE"] = "Catastrophe",
        ["COORDINATE"] = "Coordinate",
        ["DARK_SHACKLES"] = "DarkShackles",
        ["DISCOVERY"] = "Discovery",
        ["DRAMATIC_ENTRANCE"] = "DramaticEntrance",
        ["ENTROPY"] = "Entropy",
        ["EQUILIBRIUM"] = "Equilibrium",
        ["ETERNAL_ARMOR"] = "EternalArmor",
        ["FASTEN"] = "Fasten",
        ["FINESSE"] = "Finesse",
        ["FISTICUFFS"] = "Fisticuffs",
        ["FLASH_OF_STEEL"] = "FlashOfSteel",
        ["GANG_UP"] = "GangUp",
        ["GOLD_AXE"] = "GoldAxe",
        ["HAND_OF_GREED"] = "HandOfGreed",
        ["HIDDEN_GEM"] = "HiddenGem",
        ["HUDDLE_UP"] = "HuddleUp",
        ["IMPATIENCE"] = "Impatience",
        ["INTERCEPT"] = "Intercept",
        ["JACK_OF_ALL_TRADES"] = "JackOfAllTrades",
        ["JACKPOT"] = "Jackpot",
        ["KNOCKDOWN"] = "Knockdown",
        ["LIFT"] = "Lift",
        ["MASTER_OF_STRATEGY"] = "MasterOfStrategy",
        ["MAYHEM"] = "Mayhem",
        ["MIMIC"] = "Mimic",
        ["MIND_BLAST"] = "MindBlast",
        ["NOSTALGIA"] = "Nostalgia",
        ["OMNISLICE"] = "Omnislice",
        ["PANACHE"] = "Panache",
        ["PANIC_BUTTON"] = "PanicButton",
        ["PREP_TIME"] = "PrepTime",
        ["PRODUCTION"] = "Production",
        ["PROLONG"] = "Prolong",
        ["PROWESS"] = "Prowess",
        ["PURITY"] = "Purity",
        ["RALLY"] = "Rally",
        ["REND"] = "Rend",
        ["RESTLESSNESS"] = "Restlessness",
        ["ROLLING_BOULDER"] = "RollingBoulder",
        ["SALVO"] = "Salvo",
        ["SCRAWL"] = "Scrawl",
        ["SECRET_TECHNIQUE"] = "SecretTechnique",
        ["SECRET_WEAPON"] = "SecretWeapon",
        ["SEEKER_STRIKE"] = "SeekerStrike",
        ["SHOCKWAVE"] = "Shockwave",
        ["SPLASH"] = "Splash",
        ["STRATAGEM"] = "Stratagem",
        ["TAG_TEAM"] = "TagTeam",
        ["THE_BOMB"] = "TheBomb",
        ["THE_GAMBIT"] = "TheGambit",
        ["THINKING_AHEAD"] = "ThinkingAhead",
        ["THRUMMING_HATCHET"] = "ThrummingHatchet",
        ["ULTIMATE_DEFEND"] = "UltimateDefend",
        ["ULTIMATE_STRIKE"] = "UltimateStrike",
        ["VOLLEY"] = "Volley",
        // -- Curse card pool --
        ["ASCENDERS_BANE"] = "AscendersBane",
        ["BAD_LUCK"] = "BadLuck",
        ["CLUMSY"] = "Clumsy",
        ["CURSE_OF_THE_BELL"] = "CurseOfTheBell",
        ["DEBT"] = "Debt",
        ["DECAY"] = "Decay",
        ["DOUBT"] = "Doubt",
        ["ENTHRALLED"] = "Enthralled",
        ["FOLLY"] = "Folly",
        ["GREED"] = "Greed",
        ["GUILTY"] = "Guilty",
        ["INJURY"] = "Injury",
        ["NORMALITY"] = "Normality",
        ["POOR_SLEEP"] = "PoorSleep",
        ["REGRET"] = "Regret",
        ["SHAME"] = "Shame",
        ["SPORE_MIND"] = "SporeMind",
        ["WRITHE"] = "Writhe",
        // -- Status card pool (Slimed/Wound/Dazed are already mapped above
        // with sts_sim effects) --
        ["BECKON"] = "Beckon",
        ["BURN"] = "Burn",
        ["DEBRIS"] = "Debris",
        ["FRANTIC_ESCAPE"] = "FranticEscape",
        ["INFECTION"] = "Infection",
        ["SOOT"] = "Soot",
        ["TOXIC"] = "Toxic",
        ["VOID"] = "Void",
    };

    /// STS2 Creature.ModelId.Entry -> sts_sim monster name. Only monsters
    /// sts_sim's monsters.rs currently models are listed; everything else
    /// falls back to the generic placeholder (name=null, attack from intent,
    /// repeated every turn forever - see src/lib.rs's None-name handling).
    public static readonly Dictionary<string, string> MonsterNameMap = new()
    {
        ["NIBBIT"] = "Nibbit",
        ["FUZZY_WURM_CRAWLER"] = "Fuzzy Wurm Crawler",
        // Shrinker Beetle's "Shrink" self-debuff (-30% outgoing damage,
        // permanent) is applied by sts_sim's own move-resolution when it
        // simulates the Shrink move - no status translation needed here.
        ["SHRINKER_BEETLE"] = "Shrinker Beetle",
        ["LEAF_SLIME_S"] = "Leaf Slime (S)",
        ["LEAF_SLIME_M"] = "Leaf Slime (M)",
        ["TWIG_SLIME_S"] = "Twig Slime (S)",
        ["TWIG_SLIME_M"] = "Twig Slime (M)",
        // Byrdonis's Territorial (+1 Strength every turn) is baked into the
        // tail of its Swoop/Peck move effects in monsters.rs - no status
        // translation needed here.
        ["BYRDONIS"] = "Byrdonis",
        ["INKLET"] = "Inklet",
        // Vantom's Slippery x9 (AfterAddedToRoom) is reported via the
        // PowerNameMap "SLIPPERY" entry below, like any other status.
        ["VANTOM"] = "Vantom",
        // Snapping Jaxfruit: single move "Energy Orb" forever.
        ["SNAPPING_JAXFRUIT"] = "Snapping Jaxfruit",
        // Ruby Raiders: 4 variants with distinct move cycles.
        ["AXE_RUBY_RAIDER"] = "Axe Ruby Raider",
        ["ASSASSIN_RUBY_RAIDER"] = "Assassin Ruby Raider",
        ["BRUTE_RUBY_RAIDER"] = "Brute Ruby Raider",
        ["CROSSBOW_RUBY_RAIDER"] = "Crossbow Ruby Raider",
        // Slithering Strangler (elite): Constrict + Thwack/Lash cycle.
        ["SLITHERING_STRANGLER"] = "Slithering Strangler",
        // Cubex Construct (elite): Charge Up + Repeater/Expel Blast cycle,
        // starts with Artifact + block.
        ["CUBEX_CONSTRUCT"] = "Cubex Construct",
        // The Kin boss encounter: Kin Priest + 2 Kin Followers.
        ["KIN_PRIEST"] = "Kin Priest",
        ["KIN_FOLLOWER"] = "Kin Follower",
        // Tracker Ruby Raider: Track (2 Frail, no damage) → Hounds forever.
        ["TRACKER_RUBY_RAIDER"] = "Tracker Ruby Raider",
        // Mawler (elite): opening Claw → random Rip and Tear / Roar / Claw.
        ["MAWLER"] = "Mawler",
        // Vine Shambler (elite): 3-move fixed cycle — Swipe → Grasping Vines
        // (+Tangled) → Chomp → repeat.
        ["VINE_SHAMBLER"] = "Vine Shambler",
        // Bygone Effigy (elite): Sleep → Wake → Slashes → repeat.
        // Status::Slow scales Attack-card damage by cards_played_this_turn.
        ["BYGONE_EFFIGY"] = "Bygone Effigy",
        ["FLYCONID"] = "Flyconid",
        ["FOGMOG"] = "Fogmog",
    };
        ["CEREMONIAL_BEAST"] = "Ceremonial Beast",

    /// STS2 PowerModel.Id.Entry -> sts_sim status name. Only the statuses
    /// Status::from_name_and_amount understands are listed; other powers
    /// are silently dropped, matching that function's "unknown name -> empty
    /// vec" behavior.
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
        ["STAMPEDE_POWER"] = "Stampede",
        ["RUPTURE_POWER"] = "Rupture",
        ["HELLRAISER_POWER"] = "Hellraiser",
        ["NO_ENERGY_GAIN_POWER"] = "NoEnergyGain",
        // ["MINION"] = "Minion",
    };

    /// sts_sim monster name (MonsterNameMap's values) -> python/sts_sim's
    /// `Encounter` key, for the "deck_baseline" command's `monster` field.
    /// Only single-monster encounters that `sts_sim.bench._SCENARIOS` models
    /// are listed; fights against anything else (multi-monster rooms, or a
    /// monster not in MonsterNameMap) skip the deck-baseline request.
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
