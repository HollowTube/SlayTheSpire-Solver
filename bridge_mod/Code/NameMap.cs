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
        ["BATTLE_TRANCE"] = "BattleTrance",
        ["BRAND"] = "Brand",
        ["CASCADE"] = "Cascade",
        ["DEMONIC_SHIELD"] = "DemonicShield",
        ["DRUM_OF_BATTLE"] = "DrumOfBattle",
        ["EXPECT_A_FIGHT"] = "ExpectAFight",
        ["FEED"] = "Feed",
        ["FIGHT_ME"] = "FightMe",
        ["HAVOC"] = "Havoc",
        ["HELLRAISER"] = "Hellraiser",
        ["HOWL_FROM_BEYOND"] = "HowlFromBeyond",
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
        ["WHIRLWIND"] = "Whirlwind",
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
        // Vantom (x9) and Inklet (x1) both apply this to themselves on
        // AfterAddedToRoom; engine.rs's Slippery caps the next HP loss the
        // holder takes to 1 and consumes a stack.
        ["SLIPPERY"] = "Slippery",
    };

    /// sts_sim monster name (MonsterNameMap's values) -> python/sts_sim's
    /// `Encounter` key, for the "deck_baseline" command's `monster` field.
    /// Only single-monster encounters that `sts_sim.bench._SCENARIOS` models
    /// are listed; multi-monster encounters use MultiMonsterEncounterMap.
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
    };

    /// Sorted "|"-joined sts_sim monster names -> `Encounter` key for
    /// multi-monster fights. Keys are `string.Join("|", names.OrderBy(n=>n))`
    /// of each enemy's MonsterNameMap value. Covers the three multi-monster
    /// encounters that `sts_sim.bench._SCENARIOS` has modeled scenarios for.
    public static readonly Dictionary<string, string> MultiMonsterEncounterMap = new()
    {
        // SlimesWeak — Leaf Slime (M) + Leaf Slime (S) + Twig Slime (S)
        ["Leaf Slime (M)|Leaf Slime (S)|Twig Slime (S)"] = "slimes-weak",
        // SlimesWeak — Twig Slime (M) variant
        ["Leaf Slime (S)|Twig Slime (M)|Twig Slime (S)"] = "slimes-weak-twig",
        // Inklet encounter — three Inklets
        ["Inklet|Inklet|Inklet"] = "inklets",
    };
}
