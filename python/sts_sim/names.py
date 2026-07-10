"""Single source of truth for all sim-known card and monster names.

Provides:
  - ``CardName``   — enum of every card implemented in ``cards.rs``
  - ``MonsterName`` — enum of every monster implemented in ``monsters.rs``
  - ``card()``     — normalise a bridge C# card class name → sim string
  - ``monster()``  — normalise a bridge C# monster class name → sim string

Bridge returns C# class names (e.g. ``StrikeIronclad``, ``FuzzyWurmCrawler``).
Sim uses display strings (e.g. ``"Strike"``, ``"Fuzzy Wurm Crawler"``).
Adding a new card or monster: add it to the enum here, add a bridge mapping
if the default suffix-strip / CamelCase heuristic doesn't produce the right
name, and update ``cards.rs`` / ``monsters.rs`` in Rust.
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

    # ── Ironclad starter ──
    STRIKE = "Strike"
    DEFEND = "Defend"
    BASH = "Bash"

    # ── Common attacks ──
    ANGER = "Anger"
    ASHEN_STRIKE = "AshenStrike"
    BATTLE_TRANCE = "BattleTrance"
    BLUDGEON = "Bludgeon"
    BODY_SLAM = "BodySlam"
    BREAKTHROUGH = "Breakthrough"
    CASCADE = "Cascade"
    HEADBUTT = "Headbutt"
    HAVOC = "Havoc"
    HEMOKINESIS = "Hemokinesis"
    HOWL_FROM_BEYOND = "HowlFromBeyond"
    IRON_WAVE = "Iron Wave"
    MANGLE = "Mangle"
    MOLTEN_FIST = "MoltenFist"
    ONE_TWO_PUNCH = "OneTwoPunch"
    PERFECTED_STRIKE = "PerfectedStrike"
    POMMEL_STRIKE = "Pommel Strike"
    SETUP_STRIKE = "Setup Strike"
    SWORD_BOOMERANG = "Sword Boomerang"
    THRASH = "Thrash"
    THUNDERCLAP = "Thunderclap"
    TWIN_STRIKE = "TwinStrike"
    UPPERCUT = "Uppercut"
    WHIRLWIND = "Whirlwind"
    VICIOUS = "Vicious"

    # ── Common skills ──
    AGGRESSION = "Aggression"
    BARRICADE = "Barricade"
    BLOOD_WALL = "BloodWall"
    BLOODLETTING = "Bloodletting"
    BREAK = "Break"
    BULLY = "Bully"
    BURNING_PACT = "BurningPact"
    CINDER = "Cinder"
    COLOSSUS = "Colossus"
    CONFLAGRATION = "Conflagration"
    CORRUPTION = "Corruption"
    CRIMSON_MANTLE = "CrimsonMantle"
    CRUELTY = "Cruelty"
    DARK_EMBRACE = "DarkEmbrace"
    DISMANTLE = "Dismantle"
    DOMINATE = "Dominate"
    DRUM_OF_BATTLE = "DrumOfBattle"
    EVIL_EYE = "Evil Eye"
    FEEL_NO_PAIN = "FeelNoPain"
    FIEND_FIRE = "FiendFire"
    FIGHT_ME = "FightMe"
    FLAME_BARRIER = "FlameBarrier"
    FORGOTTEN_RITUAL = "Forgotten Ritual"
    IMPERVIOUS = "Impervious"
    INFECTION = "Infection"
    INFERNAL_BLADE = "InfernalBlade"
    INFERNO = "Inferno"
    INFLAME = "Inflame"
    JUGGERNAUT = "Juggernaut"
    JUGGLING = "Juggling"
    NOT_YET = "NotYet"
    OFFERING = "Offering"
    PACTS_END = "PactsEnd"
    PYRE = "Pyre"
    RAGE = "Rage"
    SECOND_WIND = "SecondWind"
    SHRUG_IT_OFF = "ShrugItOff"
    SPITE = "Spite"
    STONE_ARMOR = "StoneArmor"
    STOMP = "Stomp"
    TAUNT = "Taunt"
    TEAR_ASUNDER = "TearAsunder"
    TREMBLE = "Tremble"
    TRUE_GRIT = "TrueGrit"
    UNMOVABLE = "Unmovable"
    UNRELENTING = "Unrelenting"

    # ── Powers ──
    DEMON_FORM = "DemonForm"

    # ── Status / Curse cards (added to hand/deck by game effects or ascension) ──
    DAZED = "Dazed"
    SLIMED = "Slimed"
    WOUND = "Wound"
    ASCENDERS_BANE = "Ascender's Bane"


# ── Monster catalogue ─────────────────────────────────────────────────────────


class MonsterName(str, Enum):
    """Canonical monster names — one per monster implemented in ``monsters.rs``.

    A ``str`` subclass so ``MonsterName.JAW_WORM == "Jaw Worm"`` is true.
    """

    JAW_WORM = "Jaw Worm"
    GREMLIN_NOB = "Gremlin Nob"
    NIBBIT = "Nibbit"
    FUZZY_WURM_CRAWLER = "Fuzzy Wurm Crawler"
    TWIG_SLIME_S = "Twig Slime (S)"
    TWIG_SLIME_M = "Twig Slime (M)"
    SHRINKER_BEETLE = "Shrinker Beetle"
    LEAF_SLIME_S = "Leaf Slime (S)"
    LEAF_SLIME_M = "Leaf Slime (M)"
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
    "JawWorm": MonsterName.JAW_WORM,
    "GremlinNob": MonsterName.GREMLIN_NOB,
    "Nibbit": MonsterName.NIBBIT,
    "FuzzyWurmCrawler": MonsterName.FUZZY_WURM_CRAWLER,
    "TwigSlime_S": MonsterName.TWIG_SLIME_S,
    "TwigSlime_M": MonsterName.TWIG_SLIME_M,
    "ShrinkerBeetle": MonsterName.SHRINKER_BEETLE,
    "LeafSlime_S": MonsterName.LEAF_SLIME_S,
    "LeafSlime_M": MonsterName.LEAF_SLIME_M,
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


def unknown_cards(bridge_names: list[str]) -> list[str]:
    """Return bridge card names that don't normalise to a known ``CardName``."""
    known = {c.value for c in CardName}
    return [n for n in bridge_names if card(n) not in known]


def unknown_monsters(bridge_names: list[str]) -> list[str]:
    """Return bridge monster names not in the explicit map."""
    return [n for n in bridge_names if n not in _MONSTER_MAP]
