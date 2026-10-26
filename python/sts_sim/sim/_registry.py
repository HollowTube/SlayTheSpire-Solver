"""``CardName``, ``MonsterName``, ``StatusName`` and ``Encounter``, built at
import time from the Rust registry rows (``crates/sts-ids/src/registry.rs``).

The member list for type checkers is ``_registry.pyi``, written by
``python scripts/gen_ids.py``.
"""

from __future__ import annotations

import re
from enum import Enum
from typing import Any

from .._sts_sim import cards, monsters, statuses

CARD_ROWS: tuple[dict[str, Any], ...] = tuple(cards())
MONSTER_ROWS: tuple[dict[str, Any], ...] = tuple(monsters())
STATUS_ROWS: tuple[dict[str, Any], ...] = tuple(statuses())

# Encounter keys whose scenario fights more than one registry monster.
MULTI_MONSTER_ENCOUNTERS: tuple[str, ...] = (
    "slimes-weak",
    "slimes-weak-twig",
    "inklets",
    "ruby-raiders",
    "the-kin",
)


def display_to_member(display: str) -> str:
    """Convert a display name to a Python SCREAMING_SNAKE enum member name.

    "Iron Wave"        -> IRON_WAVE
    "DemonForm"        -> DEMON_FORM  (camel split)
    "Twig Slime (S)"   -> TWIG_SLIME_S
    "Ascender's Bane"  -> ASCENDERS_BANE
    """
    s = display.replace("'", "")
    s = re.sub(r"\((\w+)\)", r"_\1", s)
    s = re.sub(r"(?<=[a-z])(?=[A-Z])", "_", s)
    s = re.sub(r"[^A-Za-z0-9]+", "_", s)
    s = re.sub(r"_+", "_", s)
    return s.strip("_").upper()


def encounter_member(key: str) -> str:
    return key.upper().replace("-", "_")


class _StatusEnum(str, Enum):
    bridge_classes: tuple[str, ...]

    def __new__(cls, value: str, bridge_classes: tuple[str, ...] = ()):
        obj = str.__new__(cls, value)
        obj._value_ = value
        obj.bridge_classes = bridge_classes
        return obj


CardName: Any = Enum(
    "CardName",
    [(display_to_member(r["display"]), r["display"]) for r in CARD_ROWS],
    type=str,
    module=__name__,
)
CardName.__doc__ = """Canonical card names, one per registry card row.

A ``str`` subclass so ``CardName.STRIKE == "Strike"`` is true and these
compare equal to plain strings everywhere in the codebase.
"""

MonsterName: Any = Enum(
    "MonsterName",
    [(display_to_member(r["display"]), r["display"]) for r in MONSTER_ROWS],
    type=str,
    module=__name__,
)
MonsterName.__doc__ = """Canonical monster names, one per registry monster row.

A ``str`` subclass so ``MonsterName.JAW_WORM == "Jaw Worm"`` is true.
"""

StatusName: Any = _StatusEnum(
    "StatusName",
    [
        (display_to_member(r["display"]), (r["display"], tuple(r["bridge_names"])))
        for r in STATUS_ROWS
    ],
    module=__name__,
)
StatusName.__doc__ = """Canonical status/power names, one per registry status row.

``StatusName.VULNERABLE == "Vulnerable"`` is true (str subclass).
``StatusName.VULNERABLE.bridge_classes`` gives ``("Vulnerable", "VulnerablePower")``.
"""

_ENCOUNTER_KEYS = [r["encounter"] for r in MONSTER_ROWS if r["encounter"]]
Encounter: Any = Enum(
    "Encounter",
    [
        (encounter_member(k), k)
        for k in _ENCOUNTER_KEYS + list(MULTI_MONSTER_ENCOUNTERS)
    ],
    type=str,
    module=__name__,
)
Encounter.__doc__ = """Canonical keys for `run_deck`/`compare`'s `monster=` argument — one
per scenario in `sts_sim.sim.scenarios`. A `str` subclass, so plain strings
(e.g. `monster="jaw-worm"`) keep working: `Encounter.JAW_WORM == "jaw-worm"`.
"""
