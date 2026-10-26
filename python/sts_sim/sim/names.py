"""Canonical card, monster, and status name enums + bridge normalization.

Provides:
  - ``CardName``    — enum of every registry card
  - ``MonsterName`` — enum of every registry monster
  - ``StatusName``  — enum of every registry status
  - ``card()``      — normalise a bridge C# card class name → sim string
  - ``monster()``   — normalise a bridge C# monster class name → sim string

Bridge returns C# class names (e.g. ``StrikeIronclad``, ``FuzzyWurmCrawler``).
Sim uses display strings (e.g. ``"Strike"``, ``"Fuzzy Wurm Crawler"``).

Everything here is derived from the registry rows in
``crates/sts-ids/src/registry.rs``; add a card, monster or status there.
"""

from __future__ import annotations

import re

from ._registry import (
    CARD_ROWS,
    MONSTER_ROWS,
    CardName,
    MonsterName,
    StatusName,
)

__all__ = [
    "CARD_STS2_ID",
    "CardName",
    "MonsterName",
    "StatusName",
    "card",
    "monster",
    "unknown_cards",
    "unknown_monsters",
]

# ── Bridge → sim card normalization ──────────────────────────────────────────

# Bridge card class name → sim display name: the variant, the variant plus
# each character suffix, and the row's aliases (e.g. ``Ascender'sBane``).
_BRIDGE_CARD_MAP: dict[str, str] = {
    name: row["display"] for row in CARD_ROWS for name in row["bridge_names"]
}


def card(bridge_name: str) -> str:
    """Normalise a bridge card class name to a sim card name string."""
    return _BRIDGE_CARD_MAP.get(bridge_name, bridge_name)


# ── Bridge → sim monster normalization ───────────────────────────────────────

# Bridge class name → sim display name. The bridge uses CamelCase or
# CamelCase_Size (e.g. TwigSlime_S).
_MONSTER_MAP: dict[str, str] = {
    name: MonsterName(row["display"])
    for row in MONSTER_ROWS
    for name in row["bridge_names"]
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

# CardName display value → STS2 console ID used by the dev console
# (e.g. `card STRIKE_IRONCLAD hand`).
CARD_STS2_ID: dict[str, str] = {row["display"]: row["sts2_id"] for row in CARD_ROWS}


def unknown_cards(bridge_names: list[str]) -> list[str]:
    """Return bridge card names that don't normalise to a known ``CardName``."""
    known = {c.value for c in CardName}
    return [n for n in bridge_names if card(n) not in known]


def unknown_monsters(bridge_names: list[str]) -> list[str]:
    """Return bridge monster names not in the explicit map."""
    return [n for n in bridge_names if n not in _MONSTER_MAP]
