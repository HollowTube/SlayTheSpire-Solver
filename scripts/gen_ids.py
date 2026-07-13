#!/usr/bin/env python3
"""Generate ids.rs, names.py, and cards.rs regions from data/cards.toml + data/monsters.toml.

Usage:
    python scripts/gen_ids.py          # regenerate in place
    python scripts/gen_ids.py --check  # fail if committed files differ from generated output
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent

BEGIN = "BEGIN GENERATED"
END = "END GENERATED"


# ── helpers ──────────────────────────────────────────────────────────────────


def _display_to_py_member(display: str) -> str:
    """Convert a display name to a Python SCREAMING_SNAKE enum member name.

    "Iron Wave"        -> IRON_WAVE
    "DemonForm"        -> DEMON_FORM  (camel split)
    "Twig Slime (S)"   -> TWIG_SLIME_S
    "Ascender's Bane"  -> ASCENDERS_BANE
    """
    s = display
    s = s.replace("'", "")
    s = re.sub(r"\((\w+)\)", r"_\1", s)  # (S) → _S
    s = re.sub(r"(?<=[a-z])(?=[A-Z])", "_", s)  # camelCase → camel_Case
    s = re.sub(r"[^A-Za-z0-9]+", "_", s)  # spaces/misc → _
    s = re.sub(r"_+", "_", s)
    return s.strip("_").upper()


def _replace_region(
    text: str, new_content: str, comment_char: str, tag: str = ""
) -> str:
    """Replace the content between BEGIN/END GENERATED markers.

    If `tag` is given (e.g. "CardName"), markers must include it:
        # BEGIN GENERATED CardName
        # END GENERATED CardName
    Otherwise matches the first untagged BEGIN/END pair.
    """
    tag_suffix = f" {tag}" if tag else ""
    begin_marker = f"{comment_char} {BEGIN}{tag_suffix}"
    end_marker = f"{comment_char} {END}{tag_suffix}"
    pattern = re.compile(
        rf"({re.escape(begin_marker)}[^\n]*\n).*?([ \t]*{re.escape(end_marker)})",
        re.DOTALL,
    )
    if not pattern.search(text):
        raise ValueError(f"Could not find {begin_marker!r} / {end_marker!r} markers")
    return pattern.sub(rf"\g<1>{new_content}\g<2>", text)


# ── generators ───────────────────────────────────────────────────────────────


def _gen_ids_rs(cards: list[dict], monsters: list[dict]) -> str:
    """Content between the markers in src/ids.rs (two define_ids! blocks)."""
    col = 16  # alignment column for display strings

    def fmt_card(c: dict) -> str:
        v = c["variant"]
        d = c["display"]
        s = c["sts2_id"]
        pad = " " * max(1, col - len(v))
        return f'    {v}{pad}=> "{d}"{" " * max(1, col - len(d))}/ "{s}",'

    def fmt_monster(m: dict) -> str:
        v = m["variant"]
        d = m["display"]
        s = m["sts2_id"]
        pad = " " * max(1, col + 4 - len(v))
        return f'    {v}{pad}=> "{d}"{" " * max(1, col + 4 - len(d))}/ "{s}",'

    card_lines = "\n".join(fmt_card(c) for c in cards)
    monster_lines = "\n".join(fmt_monster(m) for m in monsters)

    return (
        f"define_ids!(CardId {{\n{card_lines}\n}});\n"
        f"\n"
        f"// ── MonsterId ────────────────────────────────────────────────────────────────\n"
        f"\n"
        f"define_ids!(MonsterId {{\n{monster_lines}\n}});\n"
    )


def _gen_card_name_enum(cards: list[dict]) -> str:
    """CardName enum body (indented lines between the markers in names.py)."""
    lines = []
    for c in cards:
        member = _display_to_py_member(c["display"])
        lines.append(f'    {member} = "{c["display"]}"')
    # Two blank lines required before the unindented # END GENERATED marker
    # so ruff format doesn't add them and make --check fail.
    return "\n".join(lines) + "\n\n\n"


def _gen_monster_name_enum(monsters: list[dict]) -> str:
    """MonsterName enum body."""
    lines = []
    for m in monsters:
        member = _display_to_py_member(m["display"])
        lines.append(f'    {member} = "{m["display"]}"')
    return "\n".join(lines) + "\n\n\n"


def _gen_monster_map(monsters: list[dict]) -> str:
    """_MONSTER_MAP entries (between the markers, before the acid-slime section)."""
    lines = []
    for m in monsters:
        member = _display_to_py_member(m["display"])
        note = m.get("bridge_note")
        if note:
            lines.append(f"    # {note}")
        for bc in m["bridge_classes"]:
            lines.append(f'    "{bc}": MonsterName.{member},')
    return "\n".join(lines) + "\n"


def _gen_card_sts2_id_map(cards: list[dict]) -> str:
    """CARD_STS2_ID dict body — display name → STS2 console ID."""
    lines = [f'    "{c["display"]}": "{c["sts2_id"]}",' for c in cards]
    return "\n".join(lines) + "\n"


# Character suffixes the bridge appends to card class names.
_CHAR_SUFFIXES = ("Ironclad", "Silent", "Defect", "Watcher", "Huntress")


def _gen_bridge_card_map(cards: list[dict]) -> str:
    """_BRIDGE_CARD_MAP entries — bridge class name → sim display name.

    Generates two entries per card:
      - variant → display          (e.g. "PommelStrike" → "Pommel Strike")
      - variant+suffix → display   (e.g. "PommelStrikeIronclad" → "Pommel Strike")
    Skips variant+suffix when the variant already ends with that suffix to
    avoid double-suffixing (e.g. "StrikeIronclad" would become "StrikeIroncladIronclad").
    """
    entries: dict[str, str] = {}
    for c in cards:
        v, d = c["variant"], c["display"]
        entries[v] = d
        for suffix in _CHAR_SUFFIXES:
            if not v.endswith(suffix):
                entries[v + suffix] = d
    lines = [f'    "{k}": "{v}",' for k, v in sorted(entries.items())]
    return "\n".join(lines) + "\n"


def _gen_all_card_names(cards: list[dict]) -> str:
    """ALL_CARD_NAMES array body, sorted case-insensitively."""
    pool = sorted(
        (c["display"] for c in cards if c.get("in_card_pool", True)),
        key=str.casefold,
    )
    lines = [f'    "{d}",' for d in pool]
    return "\n".join(lines) + "\n"


# ── main ─────────────────────────────────────────────────────────────────────


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit non-zero if any file would be changed (for CI)",
    )
    args = parser.parse_args()

    cards_toml = REPO_ROOT / "data" / "cards.toml"
    monsters_toml = REPO_ROOT / "data" / "monsters.toml"

    with cards_toml.open("rb") as f:
        cards = tomllib.load(f)["cards"]
    with monsters_toml.open("rb") as f:
        monsters = tomllib.load(f)["monsters"]

    # (path, new_content, comment_char, region_tag)
    # names.py has five distinct regions, disambiguated by tag.
    targets: list[tuple[Path, str, str, str]] = [
        (REPO_ROOT / "src" / "ids.rs", _gen_ids_rs(cards, monsters), "//", ""),
        (
            REPO_ROOT / "python" / "sts_sim" / "names.py",
            _gen_card_name_enum(cards),
            "#",
            "CardName",
        ),
        (
            REPO_ROOT / "python" / "sts_sim" / "names.py",
            _gen_monster_name_enum(monsters),
            "#",
            "MonsterName",
        ),
        (
            REPO_ROOT / "python" / "sts_sim" / "names.py",
            _gen_monster_map(monsters),
            "#",
            "_MONSTER_MAP",
        ),
        (
            REPO_ROOT / "python" / "sts_sim" / "names.py",
            _gen_card_sts2_id_map(cards),
            "#",
            "CARD_STS2_ID",
        ),
        (
            REPO_ROOT / "python" / "sts_sim" / "names.py",
            _gen_bridge_card_map(cards),
            "#",
            "_BRIDGE_CARD_MAP",
        ),
        (REPO_ROOT / "src" / "cards.rs", _gen_all_card_names(cards), "//", ""),
    ]

    # For files with multiple regions we apply them sequentially so each
    # replacement operates on the already-updated text.
    file_texts: dict[Path, str] = {}
    changed: list[Path] = []

    for path, content, comment_char, tag in targets:
        if path not in file_texts:
            file_texts[path] = path.read_text()
        try:
            updated = _replace_region(file_texts[path], content, comment_char, tag)
        except ValueError as e:
            print(f"ERROR in {path.relative_to(REPO_ROOT)}: {e}", file=sys.stderr)
            sys.exit(1)
        if updated != file_texts[path]:
            if path not in changed:
                changed.append(path)
            file_texts[path] = updated

    if args.check:
        if changed:
            for p in changed:
                print(f"STALE: {p.relative_to(REPO_ROOT)}", file=sys.stderr)
            print("\nRun `python scripts/gen_ids.py` to regenerate.", file=sys.stderr)
            sys.exit(1)
        print("OK: all generated regions are up to date.")
        return

    for path, text in file_texts.items():
        if path in changed:
            path.write_text(text)
            print(f"Updated {path.relative_to(REPO_ROOT)}")

    if not changed:
        print("Nothing to do — all generated regions are already up to date.")


if __name__ == "__main__":
    main()
