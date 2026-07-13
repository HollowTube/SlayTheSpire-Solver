"""Canonical card and monster name enums + bridge normalization.

Provides:
  - ``CardName``    — enum of every card in ``cards.rs`` / ``data/cards.toml``
  - ``MonsterName`` — enum of every monster in ``monsters.rs`` / ``data/monsters.toml``
  - ``card()``      — normalise a bridge C# card class name → sim string
  - ``monster()``   — normalise a bridge C# monster class name → sim string

Bridge returns C# class names (e.g. ``StrikeIronclad``, ``FuzzyWurmCrawler``).
Sim uses display strings (e.g. ``"Strike"``, ``"Fuzzy Wurm Crawler"``).

**Do not edit the generated regions by hand.** Edit ``data/cards.toml`` or
``data/monsters.toml`` and run ``python scripts/gen_ids.py``.
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


# ── Bridge → sim card normalization ──────────────────────────────────────────

# Character suffixes appended to card class names in bridge output.
_CHAR_SUFFIXES = (
    "Ironclad",
    "Silent",
    "Defect",
    "Watcher",
    "Huntress",
)

# Cards whose sim name differs from what suffix-stripping would produce.
# Keyed by the stripped bridge class name (suffix already removed).
# Only entries where the name has spaces or otherwise can't be inferred.
_CARD_OVERRIDES: dict[str, str] = {
    "PommelStrike": "Pommel Strike",
    "IronWave": "Iron Wave",
    "SwordBoomerang": "Sword Boomerang",
    "EvilEye": "Evil Eye",
    "SetupStrike": "Setup Strike",
    "ForgottenRitual": "Forgotten Ritual",
    "HowlFromBeyond": "HowlFromBeyond",
    # Status / Curse cards (no character suffix)
    "Dazed": "Dazed",
    "Slimed": "Slimed",
    "Wound": "Wound",
    "Burn": "Burn",
    "Ascender'sBane": "Ascender's Bane",
}


def card(bridge_name: str) -> str:
    """Normalise a bridge card class name to a sim card name string.

    Strips known character suffixes, applies the override table, then
    returns the stripped name as-is (best-effort for unknown cards).
    """
    stripped = bridge_name
    for suffix in _CHAR_SUFFIXES:
        if bridge_name.endswith(suffix) and bridge_name != suffix:
            stripped = bridge_name[: -len(suffix)]
            break

    return _CARD_OVERRIDES.get(stripped, stripped)


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


# ── Power names ───────────────────────────────────────────────────────────────


class PowerName(str):
    """Bridge power class names as string constants.

    Use these with ``CombatFixture.has_power()`` instead of raw strings.
    The values match what the bridge returns in the ``name`` field of power objects.
    """

    VULNERABLE = "VulnerablePower"
    WEAK = "WeakPower"
    STRENGTH = "StrengthPower"
    SHRINK = "ShrinkPower"
    RITUAL = "RitualPower"
    FRAIL = "FrailPower"
    NO_DRAW = "NoDrawPower"
    REGENERATION = "RegenerationPower"
    METALLICIZE = "MetallicizePower"
    DEMON_FORM = "DemonFormPower"
    INFLAME = "InflamePower"
    BARRICADE = "BarricadePower"
    FEEL_NO_PAIN = "FeelNoPainPower"


def unknown_cards(bridge_names: list[str]) -> list[str]:
    """Return bridge card names that don't normalise to a known ``CardName``."""
    known = {c.value for c in CardName}
    return [n for n in bridge_names if card(n) not in known]


def unknown_monsters(bridge_names: list[str]) -> list[str]:
    """Return bridge monster names not in the explicit map."""
    return [n for n in bridge_names if n not in _MONSTER_MAP]
