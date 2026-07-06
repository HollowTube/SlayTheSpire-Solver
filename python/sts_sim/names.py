"""Bridge → sim name normalization for cards and monsters.

Bridge returns C# class names (e.g. ``StrikeIronclad``, ``FuzzyWurmCrawler``).
Sim uses display strings (e.g. ``"Strike"``, ``"Fuzzy Wurm Crawler"``).
"""

from __future__ import annotations

import re

# ── Cards ────────────────────────────────────────────────────────────────────

# Character suffixes appended to card class names in bridge output.
_CHAR_SUFFIXES = (
    "Ironclad",
    "Silent",
    "Defect",
    "Watcher",
    "Huntress",
)

# Cards whose sim name differs from what suffix-stripping would produce.
# Keyed by bridge class name (after suffix stripping if applicable).
_CARD_OVERRIDES: dict[str, str] = {
    # Multi-word names that are stored without spaces in the sim
    "ShrugItOff": "ShrugItOff",
    "TwinStrike": "TwinStrike",
    "OneTwoPunch": "OneTwoPunch",
    "PommelStrike": "Pommel Strike",
    "IronWave": "Iron Wave",
    "SwordBoomerang": "Sword Boomerang",
    "EvilEye": "Evil Eye",
    "SetupStrike": "Setup Strike",
    "ForgottenRitual": "Forgotten Ritual",
    "PerfectedStrike": "PerfectedStrike",
    "AshenStrike": "AshenStrike",
    "BloodWall": "BloodWall",
    "BodySlam": "BodySlam",
    "BurningPact": "BurningPact",
    "DarkEmbrace": "DarkEmbrace",
    "DemonForm": "DemonForm",
    "FeelNoPain": "FeelNoPain",
    "FiendFire": "FiendFire",
    "FlameBarrier": "FlameBarrier",
    "InfernalBlade": "InfernalBlade",
    "MoltenFist": "MoltenFist",
    "SecondWind": "SecondWind",
    "TearAsunder": "TearAsunder",
    "CrimsonMantle": "CrimsonMantle",
    "DrumOfBattle": "DrumOfBattle",
    "BattleTrance": "BattleTrance",
    # Status cards
    "Dazed": "Dazed",
    "Slimed": "Slimed",
    "Wound": "Wound",
    "Burn": "Burn",
}


def card(bridge_name: str) -> str:
    """Normalise a bridge card class name to a sim card name string.

    Strips known character suffixes, then applies override table, then
    returns the stripped name as-is (best-effort for unknown cards).
    """
    stripped = bridge_name
    for suffix in _CHAR_SUFFIXES:
        if bridge_name.endswith(suffix) and bridge_name != suffix:
            stripped = bridge_name[: -len(suffix)]
            break

    return _CARD_OVERRIDES.get(stripped, stripped)


# ── Monsters ─────────────────────────────────────────────────────────────────

# Explicit monster map: bridge class name → sim display name.
# Bridge uses CamelCase or CamelCase_Size (e.g. AcidSlime_L).
_MONSTER_MAP: dict[str, str] = {
    "JawWorm": "Jaw Worm",
    "GremlinNob": "Gremlin Nob",
    "Nibbit": "Nibbit",
    "FuzzyWurmCrawler": "Fuzzy Wurm Crawler",
    "TwigSlime_S": "Twig Slime (S)",
    "TwigSlime_M": "Twig Slime (M)",
    "ShrinkerBeetle": "Shrinker Beetle",
    "LeafSlime_S": "Leaf Slime (S)",
    "LeafSlime_M": "Leaf Slime (M)",
    "Byrdonis": "Byrdonis",
    "Inklet": "Inklet",
    "Vantom": "Vantom",
    "SnappingJaxfruit": "Snapping Jaxfruit",
    "AxeRubyRaider": "Axe Ruby Raider",
    "AssassinRubyRaider": "Assassin Ruby Raider",
    "BruteRubyRaider": "Brute Ruby Raider",
    "CrossbowRubyRaider": "Crossbow Ruby Raider",
    "SlitheringSstrangler": "Slithering Strangler",
    "SlitheriingStrangler": "Slithering Strangler",
    "SlithingStrangler": "Slithering Strangler",
    "SliteringStrangler": "Slithering Strangler",
    "CubexConstruct": "Cubex Construct",
    "KinPriest": "Kin Priest",
    "KinFollower": "Kin Follower",
    "PhrogParasite": "Phrog Parasite",
    "Wriggler": "Wriggler",
    "TrackerRubyRaider": "Tracker Ruby Raider",
    "Mawler": "Mawler",
    "VineShambler": "Vine Shambler",
    "BygoneEffigy": "Bygone Effigy",
    "Flyconid": "Flyconid",
    "Fogmog": "Fogmog",
    "CeremonialBeast": "Ceremonial Beast",
    # Acid slime variants — check actual bridge names when encountered
    "AcidSlime_L": "Acid Slime (L)",
    "AcidSlime_S": "Acid Slime (S)",
    "AcidSlime_M": "Acid Slime (M)",
}


def monster(bridge_name: str) -> str:
    """Normalise a bridge monster class name to a sim monster name string.

    Uses explicit table first, then falls back to inserting spaces before
    uppercase runs (CamelCase → Title Case).
    """
    if bridge_name in _MONSTER_MAP:
        return _MONSTER_MAP[bridge_name]

    # Heuristic: split CamelCase on transitions, handle _Size suffixes
    name = bridge_name
    size_match = re.search(r"_([LMS])$", name)
    size_suffix = f" ({size_match.group(1)})" if size_match else ""
    if size_match:
        name = name[: size_match.start()]

    # Insert spaces before uppercase letters that follow lowercase
    spaced = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", name)
    return spaced + size_suffix


def unknown_cards(bridge_names: list[str]) -> list[str]:
    """Return any bridge card names that aren't in the known mapping."""
    return [n for n in bridge_names if card(n) == n and n not in _CARD_OVERRIDES]


def unknown_monsters(bridge_names: list[str]) -> list[str]:
    """Return any bridge monster names that aren't in the explicit map."""
    return [n for n in bridge_names if n not in _MONSTER_MAP]
