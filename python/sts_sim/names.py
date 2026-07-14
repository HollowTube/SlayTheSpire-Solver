"""Canonical card, monster, and status name enums + bridge normalization.

Provides:
  - ``CardName``    — enum of every card in ``cards.rs`` / ``data/cards.toml``
  - ``MonsterName`` — enum of every monster in ``monsters.rs`` / ``data/monsters.toml``
  - ``StatusName``  — enum of every status in ``data/statuses.toml``
  - ``card()``      — normalise a bridge C# card class name → sim string
  - ``monster()``   — normalise a bridge C# monster class name → sim string

Bridge returns C# class names (e.g. ``StrikeIronclad``, ``FuzzyWurmCrawler``).
Sim uses display strings (e.g. ``"Strike"``, ``"Fuzzy Wurm Crawler"``).

**Do not edit the generated regions by hand.** Edit the relevant TOML in ``data/``
and run ``python scripts/gen_ids.py``.
"""

from __future__ import annotations

import re
from enum import Enum

# ── Card catalogue ────────────────────────────────────────────────────────────


class CardName(str, Enum):
    """Canonical card names — one per card implemented in ``cards.rs``.

    A ``str`` subclass so ``CardName.STRIKE == "Strike"`` is true and these
    compare equal to plain strings everywhere in the codebase.
    """

    # BEGIN GENERATED CardName
    STRIKE = "Strike"
    DEFEND = "Defend"
    BASH = "Bash"
    IRON_WAVE = "Iron Wave"
    INFLAME = "Inflame"
    SWORD_BOOMERANG = "Sword Boomerang"
    THUNDERCLAP = "Thunderclap"
    RAGE = "Rage"
    DEMON_FORM = "DemonForm"
    CRIMSON_MANTLE = "CrimsonMantle"
    INFERNO = "Inferno"
    AGGRESSION = "Aggression"
    DARK_EMBRACE = "DarkEmbrace"
    FEEL_NO_PAIN = "FeelNoPain"
    BARRICADE = "Barricade"
    JUGGERNAUT = "Juggernaut"
    FLAME_BARRIER = "FlameBarrier"
    COLOSSUS = "Colossus"
    CORRUPTION = "Corruption"
    CRUELTY = "Cruelty"
    MANGLE = "Mangle"
    ONE_TWO_PUNCH = "OneTwoPunch"
    POMMEL_STRIKE = "Pommel Strike"
    BLOODLETTING = "Bloodletting"
    BLOOD_WALL = "BloodWall"
    HEMOKINESIS = "Hemokinesis"
    OFFERING = "Offering"
    TREMBLE = "Tremble"
    IMPERVIOUS = "Impervious"
    NOT_YET = "NotYet"
    ASCENDERS_BANE = "Ascender's Bane"
    SLIMED = "Slimed"
    WOUND = "Wound"
    INFECTION = "Infection"
    DAZED = "Dazed"
    CINDER = "Cinder"
    TRUE_GRIT = "TrueGrit"
    BURNING_PACT = "BurningPact"
    THRASH = "Thrash"
    SECOND_WIND = "SecondWind"
    HEADBUTT = "Headbutt"
    FIEND_FIRE = "FiendFire"
    INFERNAL_BLADE = "InfernalBlade"
    BLUDGEON = "Bludgeon"
    TWIN_STRIKE = "TwinStrike"
    BREAK = "Break"
    SHRUG_IT_OFF = "ShrugItOff"
    TAUNT = "Taunt"
    UPPERCUT = "Uppercut"
    BODY_SLAM = "BodySlam"
    PERFECTED_STRIKE = "PerfectedStrike"
    ASHEN_STRIKE = "AshenStrike"
    BULLY = "Bully"
    CONFLAGRATION = "Conflagration"
    TEAR_ASUNDER = "TearAsunder"
    SPITE = "Spite"
    DISMANTLE = "Dismantle"
    MOLTEN_FIST = "MoltenFist"
    DOMINATE = "Dominate"
    BREAKTHROUGH = "Breakthrough"
    SETUP_STRIKE = "Setup Strike"
    UNRELENTING = "Unrelenting"
    EVIL_EYE = "Evil Eye"
    FORGOTTEN_RITUAL = "Forgotten Ritual"
    PYRE = "Pyre"
    ANGER = "Anger"
    DRUM_OF_BATTLE = "DrumOfBattle"
    STOMP = "Stomp"
    FIGHT_ME = "FightMe"
    STONE_ARMOR = "StoneArmor"
    VICIOUS = "Vicious"
    JUGGLING = "Juggling"
    UNMOVABLE = "Unmovable"
    PACTS_END = "PactsEnd"
    HOWL_FROM_BEYOND = "HowlFromBeyond"
    HAVOC = "Havoc"
    BATTLE_TRANCE = "BattleTrance"
    WHIRLWIND = "Whirlwind"
    CASCADE = "Cascade"


# END GENERATED CardName


# ── Monster catalogue ─────────────────────────────────────────────────────────


class MonsterName(str, Enum):
    """Canonical monster names — one per monster implemented in ``monsters.rs``.

    A ``str`` subclass so ``MonsterName.JAW_WORM == "Jaw Worm"`` is true.
    """

    # BEGIN GENERATED MonsterName
    JAW_WORM = "Jaw Worm"
    GREMLIN_NOB = "Gremlin Nob"
    NIBBIT = "Nibbit"
    FUZZY_WURM_CRAWLER = "Fuzzy Wurm Crawler"
    TWIG_SLIME_S = "Twig Slime (S)"
    SHRINKER_BEETLE = "Shrinker Beetle"
    LEAF_SLIME_S = "Leaf Slime (S)"
    LEAF_SLIME_M = "Leaf Slime (M)"
    TWIG_SLIME_M = "Twig Slime (M)"
    BYRDONIS = "Byrdonis"
    INKLET = "Inklet"
    VANTOM = "Vantom"
    SNAPPING_JAXFRUIT = "Snapping Jaxfruit"
    AXE_RUBY_RAIDER = "Axe Ruby Raider"
    ASSASSIN_RUBY_RAIDER = "Assassin Ruby Raider"
    BRUTE_RUBY_RAIDER = "Brute Ruby Raider"
    CROSSBOW_RUBY_RAIDER = "Crossbow Ruby Raider"
    SLITHERING_STRANGLER = "Slithering Strangler"
    CUBEX_CONSTRUCT = "Cubex Construct"
    KIN_PRIEST = "Kin Priest"
    KIN_FOLLOWER = "Kin Follower"
    PHROG_PARASITE = "Phrog Parasite"
    WRIGGLER = "Wriggler"
    TRACKER_RUBY_RAIDER = "Tracker Ruby Raider"
    MAWLER = "Mawler"
    VINE_SHAMBLER = "Vine Shambler"
    BYGONE_EFFIGY = "Bygone Effigy"
    FLYCONID = "Flyconid"
    FOGMOG = "Fogmog"
    CEREMONIAL_BEAST = "Ceremonial Beast"
    EYE_WITH_TEETH = "Eye With Teeth"


# END GENERATED MonsterName


# ── Status catalogue ─────────────────────────────────────────────────────────


class StatusName(str, Enum):
    """Canonical status/power names — one per status in ``data/statuses.toml``.

    Each member carries the bridge class aliases that map to it, so
    ``STATUS_MAP`` in ``bridge.py`` can be derived without a separate
    generated artifact.

    ``StatusName.VULNERABLE == "Vulnerable"`` is true (str subclass).
    ``StatusName.VULNERABLE.bridge_classes`` gives ``("Vulnerable", "VulnerablePower")``.
    """

    def __new__(cls, value: str, bridge_classes: tuple[str, ...] = ()):
        obj = str.__new__(cls, value)
        obj._value_ = value
        obj.bridge_classes = bridge_classes  # type: ignore[attr-defined]
        return obj

    # BEGIN GENERATED StatusName
    VULNERABLE = ("Vulnerable", ("Vulnerable", "VulnerablePower"))
    WEAK = ("Weak", ("Weak", "WeakPower"))
    FRAIL = ("Frail", ("Frail", "FrailPower"))
    STRENGTH = ("Strength", ("Strength",))
    DEXTERITY = ("Dexterity", ("Dexterity",))
    POISON = ("Poison", ("Poison",))
    THORNS = ("Thorns", ("Thorns",))
    METALLICIZE = ("Metallicize", ("Metallicize",))
    RITUAL = ("Ritual", ("Ritual",))
    BARRICADE = ("Barricade", ("Barricade",))
    PLATING = ("Plating", ("Plating",))
    REGEN = ("Regen", ("Regen",))
    BRUTALITY = ("Brutality", ("Brutality",))
    DEMON_FORM = ("DemonForm", ("DemonForm",))
    JUGGERNAUT = ("Juggernaut", ("Juggernaut",))
    INFLAME = ("Inflame", ("Inflame", "InflamePower"))
    FEEL_NO_PAIN = ("FeelNoPain", ("FeelNoPain", "FeelNoPainPower"))
    NO_DRAW = ("NoDraw", ("NoDraw", "NoDrawPower"))
    SHRINK = ("Shrink", ("Shrink", "ShrinkPower"))


# END GENERATED StatusName


# ── Bridge → sim card normalization ──────────────────────────────────────────

# Maps bridge card class name → sim display name.
# Generated from data/cards.toml (variant and variant+character-suffix entries).
# Hand-written exceptions below the generated region cover the two cases where
# the bridge class name can't be derived from the TOML variant field.
_BRIDGE_CARD_MAP: dict[str, str] = {
    # BEGIN GENERATED _BRIDGE_CARD_MAP
    "Aggression": "Aggression",
    "AggressionDefect": "Aggression",
    "AggressionHuntress": "Aggression",
    "AggressionIronclad": "Aggression",
    "AggressionSilent": "Aggression",
    "AggressionWatcher": "Aggression",
    "Anger": "Anger",
    "AngerDefect": "Anger",
    "AngerHuntress": "Anger",
    "AngerIronclad": "Anger",
    "AngerSilent": "Anger",
    "AngerWatcher": "Anger",
    "AscendersBane": "Ascender's Bane",
    "AscendersBaneDefect": "Ascender's Bane",
    "AscendersBaneHuntress": "Ascender's Bane",
    "AscendersBaneIronclad": "Ascender's Bane",
    "AscendersBaneSilent": "Ascender's Bane",
    "AscendersBaneWatcher": "Ascender's Bane",
    "AshenStrike": "AshenStrike",
    "AshenStrikeDefect": "AshenStrike",
    "AshenStrikeHuntress": "AshenStrike",
    "AshenStrikeIronclad": "AshenStrike",
    "AshenStrikeSilent": "AshenStrike",
    "AshenStrikeWatcher": "AshenStrike",
    "Barricade": "Barricade",
    "BarricadeDefect": "Barricade",
    "BarricadeHuntress": "Barricade",
    "BarricadeIronclad": "Barricade",
    "BarricadeSilent": "Barricade",
    "BarricadeWatcher": "Barricade",
    "Bash": "Bash",
    "BashDefect": "Bash",
    "BashHuntress": "Bash",
    "BashIronclad": "Bash",
    "BashSilent": "Bash",
    "BashWatcher": "Bash",
    "BattleTrance": "BattleTrance",
    "BattleTranceDefect": "BattleTrance",
    "BattleTranceHuntress": "BattleTrance",
    "BattleTranceIronclad": "BattleTrance",
    "BattleTranceSilent": "BattleTrance",
    "BattleTranceWatcher": "BattleTrance",
    "BloodWall": "BloodWall",
    "BloodWallDefect": "BloodWall",
    "BloodWallHuntress": "BloodWall",
    "BloodWallIronclad": "BloodWall",
    "BloodWallSilent": "BloodWall",
    "BloodWallWatcher": "BloodWall",
    "Bloodletting": "Bloodletting",
    "BloodlettingDefect": "Bloodletting",
    "BloodlettingHuntress": "Bloodletting",
    "BloodlettingIronclad": "Bloodletting",
    "BloodlettingSilent": "Bloodletting",
    "BloodlettingWatcher": "Bloodletting",
    "Bludgeon": "Bludgeon",
    "BludgeonDefect": "Bludgeon",
    "BludgeonHuntress": "Bludgeon",
    "BludgeonIronclad": "Bludgeon",
    "BludgeonSilent": "Bludgeon",
    "BludgeonWatcher": "Bludgeon",
    "BodySlam": "BodySlam",
    "BodySlamDefect": "BodySlam",
    "BodySlamHuntress": "BodySlam",
    "BodySlamIronclad": "BodySlam",
    "BodySlamSilent": "BodySlam",
    "BodySlamWatcher": "BodySlam",
    "Break": "Break",
    "BreakDefect": "Break",
    "BreakHuntress": "Break",
    "BreakIronclad": "Break",
    "BreakSilent": "Break",
    "BreakWatcher": "Break",
    "Breakthrough": "Breakthrough",
    "BreakthroughDefect": "Breakthrough",
    "BreakthroughHuntress": "Breakthrough",
    "BreakthroughIronclad": "Breakthrough",
    "BreakthroughSilent": "Breakthrough",
    "BreakthroughWatcher": "Breakthrough",
    "Bully": "Bully",
    "BullyDefect": "Bully",
    "BullyHuntress": "Bully",
    "BullyIronclad": "Bully",
    "BullySilent": "Bully",
    "BullyWatcher": "Bully",
    "BurningPact": "BurningPact",
    "BurningPactDefect": "BurningPact",
    "BurningPactHuntress": "BurningPact",
    "BurningPactIronclad": "BurningPact",
    "BurningPactSilent": "BurningPact",
    "BurningPactWatcher": "BurningPact",
    "Cascade": "Cascade",
    "CascadeDefect": "Cascade",
    "CascadeHuntress": "Cascade",
    "CascadeIronclad": "Cascade",
    "CascadeSilent": "Cascade",
    "CascadeWatcher": "Cascade",
    "Cinder": "Cinder",
    "CinderDefect": "Cinder",
    "CinderHuntress": "Cinder",
    "CinderIronclad": "Cinder",
    "CinderSilent": "Cinder",
    "CinderWatcher": "Cinder",
    "Colossus": "Colossus",
    "ColossusDefect": "Colossus",
    "ColossusHuntress": "Colossus",
    "ColossusIronclad": "Colossus",
    "ColossusSilent": "Colossus",
    "ColossusWatcher": "Colossus",
    "Conflagration": "Conflagration",
    "ConflagrationDefect": "Conflagration",
    "ConflagrationHuntress": "Conflagration",
    "ConflagrationIronclad": "Conflagration",
    "ConflagrationSilent": "Conflagration",
    "ConflagrationWatcher": "Conflagration",
    "Corruption": "Corruption",
    "CorruptionDefect": "Corruption",
    "CorruptionHuntress": "Corruption",
    "CorruptionIronclad": "Corruption",
    "CorruptionSilent": "Corruption",
    "CorruptionWatcher": "Corruption",
    "CrimsonMantle": "CrimsonMantle",
    "CrimsonMantleDefect": "CrimsonMantle",
    "CrimsonMantleHuntress": "CrimsonMantle",
    "CrimsonMantleIronclad": "CrimsonMantle",
    "CrimsonMantleSilent": "CrimsonMantle",
    "CrimsonMantleWatcher": "CrimsonMantle",
    "Cruelty": "Cruelty",
    "CrueltyDefect": "Cruelty",
    "CrueltyHuntress": "Cruelty",
    "CrueltyIronclad": "Cruelty",
    "CrueltySilent": "Cruelty",
    "CrueltyWatcher": "Cruelty",
    "DarkEmbrace": "DarkEmbrace",
    "DarkEmbraceDefect": "DarkEmbrace",
    "DarkEmbraceHuntress": "DarkEmbrace",
    "DarkEmbraceIronclad": "DarkEmbrace",
    "DarkEmbraceSilent": "DarkEmbrace",
    "DarkEmbraceWatcher": "DarkEmbrace",
    "Dazed": "Dazed",
    "DazedDefect": "Dazed",
    "DazedHuntress": "Dazed",
    "DazedIronclad": "Dazed",
    "DazedSilent": "Dazed",
    "DazedWatcher": "Dazed",
    "DefendIronclad": "Defend",
    "DefendIroncladDefect": "Defend",
    "DefendIroncladHuntress": "Defend",
    "DefendIroncladSilent": "Defend",
    "DefendIroncladWatcher": "Defend",
    "DemonForm": "DemonForm",
    "DemonFormDefect": "DemonForm",
    "DemonFormHuntress": "DemonForm",
    "DemonFormIronclad": "DemonForm",
    "DemonFormSilent": "DemonForm",
    "DemonFormWatcher": "DemonForm",
    "Dismantle": "Dismantle",
    "DismantleDefect": "Dismantle",
    "DismantleHuntress": "Dismantle",
    "DismantleIronclad": "Dismantle",
    "DismantleSilent": "Dismantle",
    "DismantleWatcher": "Dismantle",
    "Dominate": "Dominate",
    "DominateDefect": "Dominate",
    "DominateHuntress": "Dominate",
    "DominateIronclad": "Dominate",
    "DominateSilent": "Dominate",
    "DominateWatcher": "Dominate",
    "DrumOfBattle": "DrumOfBattle",
    "DrumOfBattleDefect": "DrumOfBattle",
    "DrumOfBattleHuntress": "DrumOfBattle",
    "DrumOfBattleIronclad": "DrumOfBattle",
    "DrumOfBattleSilent": "DrumOfBattle",
    "DrumOfBattleWatcher": "DrumOfBattle",
    "EvilEye": "Evil Eye",
    "EvilEyeDefect": "Evil Eye",
    "EvilEyeHuntress": "Evil Eye",
    "EvilEyeIronclad": "Evil Eye",
    "EvilEyeSilent": "Evil Eye",
    "EvilEyeWatcher": "Evil Eye",
    "FeelNoPain": "FeelNoPain",
    "FeelNoPainDefect": "FeelNoPain",
    "FeelNoPainHuntress": "FeelNoPain",
    "FeelNoPainIronclad": "FeelNoPain",
    "FeelNoPainSilent": "FeelNoPain",
    "FeelNoPainWatcher": "FeelNoPain",
    "FiendFire": "FiendFire",
    "FiendFireDefect": "FiendFire",
    "FiendFireHuntress": "FiendFire",
    "FiendFireIronclad": "FiendFire",
    "FiendFireSilent": "FiendFire",
    "FiendFireWatcher": "FiendFire",
    "FightMe": "FightMe",
    "FightMeDefect": "FightMe",
    "FightMeHuntress": "FightMe",
    "FightMeIronclad": "FightMe",
    "FightMeSilent": "FightMe",
    "FightMeWatcher": "FightMe",
    "FlameBarrier": "FlameBarrier",
    "FlameBarrierDefect": "FlameBarrier",
    "FlameBarrierHuntress": "FlameBarrier",
    "FlameBarrierIronclad": "FlameBarrier",
    "FlameBarrierSilent": "FlameBarrier",
    "FlameBarrierWatcher": "FlameBarrier",
    "ForgottenRitual": "Forgotten Ritual",
    "ForgottenRitualDefect": "Forgotten Ritual",
    "ForgottenRitualHuntress": "Forgotten Ritual",
    "ForgottenRitualIronclad": "Forgotten Ritual",
    "ForgottenRitualSilent": "Forgotten Ritual",
    "ForgottenRitualWatcher": "Forgotten Ritual",
    "Havoc": "Havoc",
    "HavocDefect": "Havoc",
    "HavocHuntress": "Havoc",
    "HavocIronclad": "Havoc",
    "HavocSilent": "Havoc",
    "HavocWatcher": "Havoc",
    "Headbutt": "Headbutt",
    "HeadbuttDefect": "Headbutt",
    "HeadbuttHuntress": "Headbutt",
    "HeadbuttIronclad": "Headbutt",
    "HeadbuttSilent": "Headbutt",
    "HeadbuttWatcher": "Headbutt",
    "Hemokinesis": "Hemokinesis",
    "HemokinesisDefect": "Hemokinesis",
    "HemokinesisHuntress": "Hemokinesis",
    "HemokinesisIronclad": "Hemokinesis",
    "HemokinesisSilent": "Hemokinesis",
    "HemokinesisWatcher": "Hemokinesis",
    "HowlFromBeyond": "HowlFromBeyond",
    "HowlFromBeyondDefect": "HowlFromBeyond",
    "HowlFromBeyondHuntress": "HowlFromBeyond",
    "HowlFromBeyondIronclad": "HowlFromBeyond",
    "HowlFromBeyondSilent": "HowlFromBeyond",
    "HowlFromBeyondWatcher": "HowlFromBeyond",
    "Impervious": "Impervious",
    "ImperviousDefect": "Impervious",
    "ImperviousHuntress": "Impervious",
    "ImperviousIronclad": "Impervious",
    "ImperviousSilent": "Impervious",
    "ImperviousWatcher": "Impervious",
    "Infection": "Infection",
    "InfectionDefect": "Infection",
    "InfectionHuntress": "Infection",
    "InfectionIronclad": "Infection",
    "InfectionSilent": "Infection",
    "InfectionWatcher": "Infection",
    "InfernalBlade": "InfernalBlade",
    "InfernalBladeDefect": "InfernalBlade",
    "InfernalBladeHuntress": "InfernalBlade",
    "InfernalBladeIronclad": "InfernalBlade",
    "InfernalBladeSilent": "InfernalBlade",
    "InfernalBladeWatcher": "InfernalBlade",
    "Inferno": "Inferno",
    "InfernoDefect": "Inferno",
    "InfernoHuntress": "Inferno",
    "InfernoIronclad": "Inferno",
    "InfernoSilent": "Inferno",
    "InfernoWatcher": "Inferno",
    "Inflame": "Inflame",
    "InflameDefect": "Inflame",
    "InflameHuntress": "Inflame",
    "InflameIronclad": "Inflame",
    "InflameSilent": "Inflame",
    "InflameWatcher": "Inflame",
    "IronWave": "Iron Wave",
    "IronWaveDefect": "Iron Wave",
    "IronWaveHuntress": "Iron Wave",
    "IronWaveIronclad": "Iron Wave",
    "IronWaveSilent": "Iron Wave",
    "IronWaveWatcher": "Iron Wave",
    "Juggernaut": "Juggernaut",
    "JuggernautDefect": "Juggernaut",
    "JuggernautHuntress": "Juggernaut",
    "JuggernautIronclad": "Juggernaut",
    "JuggernautSilent": "Juggernaut",
    "JuggernautWatcher": "Juggernaut",
    "Juggling": "Juggling",
    "JugglingDefect": "Juggling",
    "JugglingHuntress": "Juggling",
    "JugglingIronclad": "Juggling",
    "JugglingSilent": "Juggling",
    "JugglingWatcher": "Juggling",
    "Mangle": "Mangle",
    "MangleDefect": "Mangle",
    "MangleHuntress": "Mangle",
    "MangleIronclad": "Mangle",
    "MangleSilent": "Mangle",
    "MangleWatcher": "Mangle",
    "MoltenFist": "MoltenFist",
    "MoltenFistDefect": "MoltenFist",
    "MoltenFistHuntress": "MoltenFist",
    "MoltenFistIronclad": "MoltenFist",
    "MoltenFistSilent": "MoltenFist",
    "MoltenFistWatcher": "MoltenFist",
    "NotYet": "NotYet",
    "NotYetDefect": "NotYet",
    "NotYetHuntress": "NotYet",
    "NotYetIronclad": "NotYet",
    "NotYetSilent": "NotYet",
    "NotYetWatcher": "NotYet",
    "Offering": "Offering",
    "OfferingDefect": "Offering",
    "OfferingHuntress": "Offering",
    "OfferingIronclad": "Offering",
    "OfferingSilent": "Offering",
    "OfferingWatcher": "Offering",
    "OneTwoPunch": "OneTwoPunch",
    "OneTwoPunchDefect": "OneTwoPunch",
    "OneTwoPunchHuntress": "OneTwoPunch",
    "OneTwoPunchIronclad": "OneTwoPunch",
    "OneTwoPunchSilent": "OneTwoPunch",
    "OneTwoPunchWatcher": "OneTwoPunch",
    "PactsEnd": "PactsEnd",
    "PactsEndDefect": "PactsEnd",
    "PactsEndHuntress": "PactsEnd",
    "PactsEndIronclad": "PactsEnd",
    "PactsEndSilent": "PactsEnd",
    "PactsEndWatcher": "PactsEnd",
    "PerfectedStrike": "PerfectedStrike",
    "PerfectedStrikeDefect": "PerfectedStrike",
    "PerfectedStrikeHuntress": "PerfectedStrike",
    "PerfectedStrikeIronclad": "PerfectedStrike",
    "PerfectedStrikeSilent": "PerfectedStrike",
    "PerfectedStrikeWatcher": "PerfectedStrike",
    "PommelStrike": "Pommel Strike",
    "PommelStrikeDefect": "Pommel Strike",
    "PommelStrikeHuntress": "Pommel Strike",
    "PommelStrikeIronclad": "Pommel Strike",
    "PommelStrikeSilent": "Pommel Strike",
    "PommelStrikeWatcher": "Pommel Strike",
    "Pyre": "Pyre",
    "PyreDefect": "Pyre",
    "PyreHuntress": "Pyre",
    "PyreIronclad": "Pyre",
    "PyreSilent": "Pyre",
    "PyreWatcher": "Pyre",
    "Rage": "Rage",
    "RageDefect": "Rage",
    "RageHuntress": "Rage",
    "RageIronclad": "Rage",
    "RageSilent": "Rage",
    "RageWatcher": "Rage",
    "SecondWind": "SecondWind",
    "SecondWindDefect": "SecondWind",
    "SecondWindHuntress": "SecondWind",
    "SecondWindIronclad": "SecondWind",
    "SecondWindSilent": "SecondWind",
    "SecondWindWatcher": "SecondWind",
    "SetupStrike": "Setup Strike",
    "SetupStrikeDefect": "Setup Strike",
    "SetupStrikeHuntress": "Setup Strike",
    "SetupStrikeIronclad": "Setup Strike",
    "SetupStrikeSilent": "Setup Strike",
    "SetupStrikeWatcher": "Setup Strike",
    "ShrugItOff": "ShrugItOff",
    "ShrugItOffDefect": "ShrugItOff",
    "ShrugItOffHuntress": "ShrugItOff",
    "ShrugItOffIronclad": "ShrugItOff",
    "ShrugItOffSilent": "ShrugItOff",
    "ShrugItOffWatcher": "ShrugItOff",
    "Slimed": "Slimed",
    "SlimedDefect": "Slimed",
    "SlimedHuntress": "Slimed",
    "SlimedIronclad": "Slimed",
    "SlimedSilent": "Slimed",
    "SlimedWatcher": "Slimed",
    "Spite": "Spite",
    "SpiteDefect": "Spite",
    "SpiteHuntress": "Spite",
    "SpiteIronclad": "Spite",
    "SpiteSilent": "Spite",
    "SpiteWatcher": "Spite",
    "Stomp": "Stomp",
    "StompDefect": "Stomp",
    "StompHuntress": "Stomp",
    "StompIronclad": "Stomp",
    "StompSilent": "Stomp",
    "StompWatcher": "Stomp",
    "StoneArmor": "StoneArmor",
    "StoneArmorDefect": "StoneArmor",
    "StoneArmorHuntress": "StoneArmor",
    "StoneArmorIronclad": "StoneArmor",
    "StoneArmorSilent": "StoneArmor",
    "StoneArmorWatcher": "StoneArmor",
    "StrikeIronclad": "Strike",
    "StrikeIroncladDefect": "Strike",
    "StrikeIroncladHuntress": "Strike",
    "StrikeIroncladSilent": "Strike",
    "StrikeIroncladWatcher": "Strike",
    "SwordBoomerang": "Sword Boomerang",
    "SwordBoomerangDefect": "Sword Boomerang",
    "SwordBoomerangHuntress": "Sword Boomerang",
    "SwordBoomerangIronclad": "Sword Boomerang",
    "SwordBoomerangSilent": "Sword Boomerang",
    "SwordBoomerangWatcher": "Sword Boomerang",
    "Taunt": "Taunt",
    "TauntDefect": "Taunt",
    "TauntHuntress": "Taunt",
    "TauntIronclad": "Taunt",
    "TauntSilent": "Taunt",
    "TauntWatcher": "Taunt",
    "TearAsunder": "TearAsunder",
    "TearAsunderDefect": "TearAsunder",
    "TearAsunderHuntress": "TearAsunder",
    "TearAsunderIronclad": "TearAsunder",
    "TearAsunderSilent": "TearAsunder",
    "TearAsunderWatcher": "TearAsunder",
    "Thrash": "Thrash",
    "ThrashDefect": "Thrash",
    "ThrashHuntress": "Thrash",
    "ThrashIronclad": "Thrash",
    "ThrashSilent": "Thrash",
    "ThrashWatcher": "Thrash",
    "Thunderclap": "Thunderclap",
    "ThunderclapDefect": "Thunderclap",
    "ThunderclapHuntress": "Thunderclap",
    "ThunderclapIronclad": "Thunderclap",
    "ThunderclapSilent": "Thunderclap",
    "ThunderclapWatcher": "Thunderclap",
    "Tremble": "Tremble",
    "TrembleDefect": "Tremble",
    "TrembleHuntress": "Tremble",
    "TrembleIronclad": "Tremble",
    "TrembleSilent": "Tremble",
    "TrembleWatcher": "Tremble",
    "TrueGrit": "TrueGrit",
    "TrueGritDefect": "TrueGrit",
    "TrueGritHuntress": "TrueGrit",
    "TrueGritIronclad": "TrueGrit",
    "TrueGritSilent": "TrueGrit",
    "TrueGritWatcher": "TrueGrit",
    "TwinStrike": "TwinStrike",
    "TwinStrikeDefect": "TwinStrike",
    "TwinStrikeHuntress": "TwinStrike",
    "TwinStrikeIronclad": "TwinStrike",
    "TwinStrikeSilent": "TwinStrike",
    "TwinStrikeWatcher": "TwinStrike",
    "Unmovable": "Unmovable",
    "UnmovableDefect": "Unmovable",
    "UnmovableHuntress": "Unmovable",
    "UnmovableIronclad": "Unmovable",
    "UnmovableSilent": "Unmovable",
    "UnmovableWatcher": "Unmovable",
    "Unrelenting": "Unrelenting",
    "UnrelentingDefect": "Unrelenting",
    "UnrelentingHuntress": "Unrelenting",
    "UnrelentingIronclad": "Unrelenting",
    "UnrelentingSilent": "Unrelenting",
    "UnrelentingWatcher": "Unrelenting",
    "Uppercut": "Uppercut",
    "UppercutDefect": "Uppercut",
    "UppercutHuntress": "Uppercut",
    "UppercutIronclad": "Uppercut",
    "UppercutSilent": "Uppercut",
    "UppercutWatcher": "Uppercut",
    "Vicious": "Vicious",
    "ViciousDefect": "Vicious",
    "ViciousHuntress": "Vicious",
    "ViciousIronclad": "Vicious",
    "ViciousSilent": "Vicious",
    "ViciousWatcher": "Vicious",
    "Whirlwind": "Whirlwind",
    "WhirlwindDefect": "Whirlwind",
    "WhirlwindHuntress": "Whirlwind",
    "WhirlwindIronclad": "Whirlwind",
    "WhirlwindSilent": "Whirlwind",
    "WhirlwindWatcher": "Whirlwind",
    "Wound": "Wound",
    "WoundDefect": "Wound",
    "WoundHuntress": "Wound",
    "WoundIronclad": "Wound",
    "WoundSilent": "Wound",
    "WoundWatcher": "Wound",
    # END GENERATED _BRIDGE_CARD_MAP
    # Bridge sends an apostrophe in the class name; TOML variant "AscendersBane" has none.
    "Ascender'sBane": "Ascender's Bane",
    # Silent status card not yet in cards.toml.
    "Burn": "Burn",
}


def card(bridge_name: str) -> str:
    """Normalise a bridge card class name to a sim card name string."""
    return _BRIDGE_CARD_MAP.get(bridge_name, bridge_name)


# ── Bridge → sim monster normalization ───────────────────────────────────────

# Explicit map: bridge class name → sim display name.
# The bridge uses CamelCase or CamelCase_Size (e.g. TwigSlime_S).
_MONSTER_MAP: dict[str, str] = {
    # BEGIN GENERATED _MONSTER_MAP
    "JawWorm": MonsterName.JAW_WORM,
    "GremlinNob": MonsterName.GREMLIN_NOB,
    "Nibbit": MonsterName.NIBBIT,
    "FuzzyWurmCrawler": MonsterName.FUZZY_WURM_CRAWLER,
    "TwigSlime_S": MonsterName.TWIG_SLIME_S,
    "ShrinkerBeetle": MonsterName.SHRINKER_BEETLE,
    "LeafSlime_S": MonsterName.LEAF_SLIME_S,
    "LeafSlime_M": MonsterName.LEAF_SLIME_M,
    "TwigSlime_M": MonsterName.TWIG_SLIME_M,
    "Byrdonis": MonsterName.BYRDONIS,
    "Inklet": MonsterName.INKLET,
    "Vantom": MonsterName.VANTOM,
    "SnappingJaxfruit": MonsterName.SNAPPING_JAXFRUIT,
    "AxeRubyRaider": MonsterName.AXE_RUBY_RAIDER,
    "AssassinRubyRaider": MonsterName.ASSASSIN_RUBY_RAIDER,
    "BruteRubyRaider": MonsterName.BRUTE_RUBY_RAIDER,
    "CrossbowRubyRaider": MonsterName.CROSSBOW_RUBY_RAIDER,
    # Multiple possible spellings until confirmed against live bridge
    "SlitheriingStrangler": MonsterName.SLITHERING_STRANGLER,
    "SlitheringSstrangler": MonsterName.SLITHERING_STRANGLER,
    "SlithingStrangler": MonsterName.SLITHERING_STRANGLER,
    "SliteringStrangler": MonsterName.SLITHERING_STRANGLER,
    "SliteheringStrangler": MonsterName.SLITHERING_STRANGLER,
    "CubexConstruct": MonsterName.CUBEX_CONSTRUCT,
    "KinPriest": MonsterName.KIN_PRIEST,
    "KinFollower": MonsterName.KIN_FOLLOWER,
    "PhrogParasite": MonsterName.PHROG_PARASITE,
    "Wriggler": MonsterName.WRIGGLER,
    "TrackerRubyRaider": MonsterName.TRACKER_RUBY_RAIDER,
    "Mawler": MonsterName.MAWLER,
    "VineShambler": MonsterName.VINE_SHAMBLER,
    "BygoneEffigy": MonsterName.BYGONE_EFFIGY,
    "Flyconid": MonsterName.FLYCONID,
    "Fogmog": MonsterName.FOGMOG,
    "CeremonialBeast": MonsterName.CEREMONIAL_BEAST,
    "EyeWithTeeth": MonsterName.EYE_WITH_TEETH,
    # END GENERATED _MONSTER_MAP
    # Acid slime variants — bridge class names not yet confirmed
    "AcidSlime_L": "Acid Slime (L)",
    "AcidSlime_S": "Acid Slime (S)",
    "AcidSlime_M": "Acid Slime (M)",
}


def monster(bridge_name: str) -> str:
    """Normalise a bridge monster class name to a sim monster name string.

    Uses explicit table first, then falls back to CamelCase → Title Case.
    """
    if bridge_name in _MONSTER_MAP:
        return _MONSTER_MAP[bridge_name]

    name = bridge_name
    size_match = re.search(r"_([LMS])$", name)
    size_suffix = f" ({size_match.group(1)})" if size_match else ""
    if size_match:
        name = name[: size_match.start()]

    spaced = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", name)
    return spaced + size_suffix


# ── Card → STS2 console ID ───────────────────────────────────────────────────

# Maps CardName display value → STS2 console ID used by the dev console
# (e.g. `card STRIKE_IRONCLAD hand`).  Generated from data/cards.toml.
CARD_STS2_ID: dict[str, str] = {
    # BEGIN GENERATED CARD_STS2_ID
    "Strike": "STRIKE_IRONCLAD",
    "Defend": "DEFEND_IRONCLAD",
    "Bash": "BASH",
    "Iron Wave": "IRON_WAVE",
    "Inflame": "INFLAME",
    "Sword Boomerang": "SWORD_BOOMERANG",
    "Thunderclap": "THUNDERCLAP",
    "Rage": "RAGE",
    "DemonForm": "DEMON_FORM",
    "CrimsonMantle": "CRIMSON_MANTLE",
    "Inferno": "INFERNO",
    "Aggression": "AGGRESSION",
    "DarkEmbrace": "DARK_EMBRACE",
    "FeelNoPain": "FEEL_NO_PAIN",
    "Barricade": "BARRICADE",
    "Juggernaut": "JUGGERNAUT",
    "FlameBarrier": "FLAME_BARRIER",
    "Colossus": "COLOSSUS",
    "Corruption": "CORRUPTION",
    "Cruelty": "CRUELTY",
    "Mangle": "MANGLE",
    "OneTwoPunch": "ONE_TWO_PUNCH",
    "Pommel Strike": "POMMEL_STRIKE",
    "Bloodletting": "BLOODLETTING",
    "BloodWall": "BLOOD_WALL",
    "Hemokinesis": "HEMOKINESIS",
    "Offering": "OFFERING",
    "Tremble": "TREMBLE",
    "Impervious": "IMPERVIOUS",
    "NotYet": "NOT_YET",
    "Ascender's Bane": "ASCENDERS_BANE",
    "Slimed": "SLIMED",
    "Wound": "WOUND",
    "Infection": "INFECTION",
    "Dazed": "DAZED",
    "Cinder": "CINDER",
    "TrueGrit": "TRUE_GRIT",
    "BurningPact": "BURNING_PACT",
    "Thrash": "THRASH",
    "SecondWind": "SECOND_WIND",
    "Headbutt": "HEADBUTT",
    "FiendFire": "FIEND_FIRE",
    "InfernalBlade": "INFERNAL_BLADE",
    "Bludgeon": "BLUDGEON",
    "TwinStrike": "TWIN_STRIKE",
    "Break": "BREAK",
    "ShrugItOff": "SHRUG_IT_OFF",
    "Taunt": "TAUNT",
    "Uppercut": "UPPERCUT",
    "BodySlam": "BODY_SLAM",
    "PerfectedStrike": "PERFECTED_STRIKE",
    "AshenStrike": "ASHEN_STRIKE",
    "Bully": "BULLY",
    "Conflagration": "CONFLAGRATION",
    "TearAsunder": "TEAR_ASUNDER",
    "Spite": "SPITE",
    "Dismantle": "DISMANTLE",
    "MoltenFist": "MOLTEN_FIST",
    "Dominate": "DOMINATE",
    "Breakthrough": "BREAKTHROUGH",
    "Setup Strike": "SETUP_STRIKE",
    "Unrelenting": "UNRELENTING",
    "Evil Eye": "EVIL_EYE",
    "Forgotten Ritual": "FORGOTTEN_RITUAL",
    "Pyre": "PYRE",
    "Anger": "ANGER",
    "DrumOfBattle": "DRUM_OF_BATTLE",
    "Stomp": "STOMP",
    "FightMe": "FIGHT_ME",
    "StoneArmor": "STONE_ARMOR",
    "Vicious": "VICIOUS",
    "Juggling": "JUGGLING",
    "Unmovable": "UNMOVABLE",
    "PactsEnd": "PACTS_END",
    "HowlFromBeyond": "HOWL_FROM_BEYOND",
    "Havoc": "HAVOC",
    "BattleTrance": "BATTLE_TRANCE",
    "Whirlwind": "WHIRLWIND",
    "Cascade": "CASCADE",
    # END GENERATED CARD_STS2_ID
}


def unknown_cards(bridge_names: list[str]) -> list[str]:
    """Return bridge card names that don't normalise to a known ``CardName``."""
    known = {c.value for c in CardName}
    return [n for n in bridge_names if card(n) not in known]


def unknown_monsters(bridge_names: list[str]) -> list[str]:
    """Return bridge monster names not in the explicit map."""
    return [n for n in bridge_names if n not in _MONSTER_MAP]
