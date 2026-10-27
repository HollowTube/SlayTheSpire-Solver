"""Build, write, load and diff the game card-data fixture."""

from __future__ import annotations

import json
from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .csharp import (
    ExtractError,
    extract_card,
    load_typed_accessors,
    load_var_classes,
    pool_cards,
    slugify,
)
from .pck import read_pck_file
from .template import render, with_keywords

CARDS_DIR = "MegaCrit.Sts2.Core.Models.Cards"
POOLS_DIR = "MegaCrit.Sts2.Core.Models.CardPools"
DYNAMIC_VARS_DIR = "MegaCrit.Sts2.Core.Localization.DynamicVars"
CARDS_LOC = "localization/eng/cards.json"
KEYWORDS_LOC = "localization/eng/card_keywords.json"
FIXTURES = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "game"
# The game version the parity test checks the sim against.
GAME_VERSION = "0.107.1"


def fixture_path(game_version: str, pool: str = "ironclad") -> Path:
    return FIXTURES / game_version / f"{pool}_cards.json"


def read_release_info(pck: Path) -> dict[str, Any]:
    info = pck.parent / "release_info.json"
    if not info.exists():
        raise ExtractError(f"{info} not found next to the PCK")
    data: dict[str, Any] = json.loads(info.read_text())
    return data


def build_fixture(
    decompiled: str | Path,
    pck: str | Path,
    game_version: str,
    pool_class: str = "IroncladCardPool",
) -> dict[str, Any]:
    """Extract every card in ``pool_class`` from the decompiled code and the PCK."""
    decompiled, pck = Path(decompiled), Path(pck)
    release = read_release_info(pck)
    if str(release.get("version", "")).removeprefix("v") != game_version:
        raise ExtractError(
            f"--game-version {game_version} but {pck.parent / 'release_info.json'} "
            f"says {release.get('version')}"
        )
    loc = json.loads(read_pck_file(pck, CARDS_LOC))
    kw_loc = json.loads(read_pck_file(pck, KEYWORDS_LOC))
    classes = load_var_classes(decompiled / DYNAMIC_VARS_DIR)
    accessors = load_typed_accessors(decompiled / DYNAMIC_VARS_DIR / "DynamicVarSet.cs")
    names = pool_cards(
        (decompiled / POOLS_DIR / f"{pool_class}.cs").read_bytes(), pool_class
    )

    titles: dict[str, str] = {}
    cards: dict[str, Any] = {}
    for name in names:
        entry = extract_card(
            (decompiled / CARDS_DIR / f"{name}.cs").read_bytes(),
            name,
            classes,
            accessors,
        )
        key = slugify(name)
        for suffix in ("title", "description"):
            if f"{key}.{suffix}" not in loc:
                raise ExtractError(f"{name}: no {key}.{suffix} in {CARDS_LOC}")
        for kw in entry["keywords"] + entry["upgraded_keywords"]:
            title = kw_loc.get(f"{slugify(kw)}.title")
            if title is None:
                raise ExtractError(f"{name}: no {slugify(kw)}.title in {KEYWORDS_LOC}")
            titles[kw] = title
        entry["title"] = loc[f"{key}.title"]
        entry["template"] = loc[f"{key}.description"]
        entry["text"] = {
            level: card_text(entry, level == "upgraded", titles)
            for level in ("base", "upgraded")
        }
        cards[key] = entry
    return {
        "game_version": game_version,
        "game_commit": release.get("commit"),
        "pool": pool_class,
        "source": "extracted from the game files by scripts/card_parity.py extract",
        "keyword_titles": titles,
        "cards": cards,
    }


def card_text(
    entry: Mapping[str, Any], upgraded: bool, titles: Mapping[str, str]
) -> str:
    level = "upgraded" if upgraded else "base"
    values = {name: var[level] for name, var in entry["vars"].items()}
    body = render(entry["template"], values, upgraded, card=entry["class"])
    keywords = entry["upgraded_keywords"] if upgraded else entry["keywords"]
    return with_keywords(body, keywords, titles)


def dumps(fixture: Mapping[str, Any]) -> str:
    return json.dumps(fixture, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def write_fixture(fixture: Mapping[str, Any], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(dumps(fixture), encoding="utf-8")


def load_fixture(path: str | Path) -> dict[str, Any]:
    data: dict[str, Any] = json.loads(Path(path).read_text(encoding="utf-8"))
    return data


# --------------------------------------------------------------------------
# diff
# --------------------------------------------------------------------------


@dataclass
class FixtureDiff:
    added: list[str] = field(default_factory=list)
    removed: list[str] = field(default_factory=list)
    changed: dict[str, list[tuple[str, Any, Any]]] = field(default_factory=dict)

    def __bool__(self) -> bool:
        return bool(self.added or self.removed or self.changed)

    def lines(self) -> list[str]:
        out = [f"+ {card}" for card in self.added]
        out += [f"- {card}" for card in self.removed]
        for card, fields in self.changed.items():
            for path, old, new in fields:
                out.append(f"~ {card}.{path}: {old!r} -> {new!r}")
        return out


_MISSING = "<absent>"


def _flatten(value: Any, prefix: str = "") -> dict[str, Any]:
    if isinstance(value, dict) and value:
        out: dict[str, Any] = {}
        for k, v in value.items():
            out.update(_flatten(v, f"{prefix}.{k}" if prefix else str(k)))
        return out
    return {prefix: value}


def diff_fixtures(old: Mapping[str, Any], new: Mapping[str, Any]) -> FixtureDiff:
    """Cards added, removed, and changed field by field between two fixtures."""
    a, b = old["cards"], new["cards"]
    out = FixtureDiff(added=sorted(set(b) - set(a)), removed=sorted(set(a) - set(b)))
    for card in sorted(set(a) & set(b)):
        fa, fb = _flatten(a[card]), _flatten(b[card])
        fields = [
            (path, fa.get(path, _MISSING), fb.get(path, _MISSING))
            for path in sorted(set(fa) | set(fb))
            if fa.get(path, _MISSING) != fb.get(path, _MISSING)
        ]
        if fields:
            out.changed[card] = fields
    return out
