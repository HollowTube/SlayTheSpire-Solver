"""Read the dev console's command list out of decompiled STS2 C# (``AbstractConsoleCmd`` subclasses).

Same stance as ``csharp.py``: a closed set of recognised shapes, and an
``ExtractError`` naming the class and line for anything else, so a game patch
that reshapes a command fails the extraction instead of producing a wrong
catalog. The result is written to ``crates/sts-bridge/catalog/<version>/`` and
compiled into the bridge crate.
"""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from tree_sitter import Node

from .csharp import (
    ExtractError,
    _Card,
    _field,
    _named,
    _parse,
    _text,
    _walk,
    slugify,
)
from .fixture import FixtureDiff, diff_fixtures, read_release_info

CONSOLE_DIR = "MegaCrit.Sts2.Core.DevConsole.ConsoleCommands"
BASE_CLASS = "AbstractConsoleCmd"
SUBTYPES_FILE = "AbstractConsoleCmdSubtypes.cs"
PILE_TYPE_FILE = "MegaCrit.Sts2.Core.Entities.Cards/PileType.cs"
# id domain -> (decompiled directory, the model base class)
ID_DOMAINS = {
    "cards": ("MegaCrit.Sts2.Core.Models.Cards", "CardModel"),
    "encounters": ("MegaCrit.Sts2.Core.Models.Encounters", "EncounterModel"),
    "powers": ("MegaCrit.Sts2.Core.Models.Powers", "PowerModel"),
    "relics": ("MegaCrit.Sts2.Core.Models.Relics", "RelicModel"),
}
# Calls that move the run to another room or act. Sending one while a run is
# still loading crashes the game.
ENTERS_ROOM = frozenset({"EnterRoom", "EnterRoomDebug", "EnterAct"})
SOURCE = "extracted from the game files by scripts/console_catalog.py extract"
# The built-in the dev console answers itself (DevConsole.ProcessCommand).
HELP = {
    "class": None,
    "args": "[command:string]",
    "parsed_args": [
        {
            "raw": "[command:string]",
            "name": "command",
            "type": "string",
            "optional": True,
        }
    ],
    "description": "Lists every command, or one command's usage.",
    "networked": False,
    "debug_only": False,
    "enters_room": False,
    "process_sha256": None,
}
_TYPES = r"(?:int|string|bool)(?:\|(?:int|string|bool))*"


def subtype_classes(source: str | bytes) -> list[str]:
    """The ``typeof`` entries of ``AbstractConsoleCmdSubtypes``, checked against its own count."""
    text = source.decode() if isinstance(source, bytes) else source
    names = re.findall(r"static readonly Type _t\d+ = typeof\((\w+)\);", text)
    sized = re.search(r"new Type\[(\d+)\]", text)
    if not names or sized is None or int(sized.group(1)) != len(names):
        raise ExtractError(
            f"{SUBTYPES_FILE}: {len(names)} typeof entries but the array says "
            f"{sized.group(1) if sized else 'nothing'}"
        )
    if len(set(names)) != len(names):
        raise ExtractError(f"{SUBTYPES_FILE}: a class is listed twice")
    return names


def _unquote(literal: str) -> str:
    body = literal[1:-1]
    return re.sub(
        r"\\(.)", lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)), body
    )


def string_value(node: Node) -> str:
    """A string expression: literals and ``+``; anything else becomes ``{source}``."""
    if node.type == "string_literal":
        return _unquote(_text(node))
    if node.type == "binary_expression" and _text(_field(node, "operator")) == "+":
        return string_value(_field(node, "left")) + string_value(_field(node, "right"))
    if node.type == "parenthesized_expression":
        return string_value(_named(node)[0])
    return "{" + " ".join(_text(node).split()) + "}"


def parse_args(args: str) -> list[dict[str, Any]]:
    """Best-effort split of the usage string; ``raw`` is always authoritative."""
    tokens: list[str] = []
    depth, current = 0, ""
    for ch in args:
        if ch in "<[":
            depth += 1
        elif ch in ">]":
            depth -= 1
        if ch.isspace() and depth <= 0:
            if current:
                tokens.append(current)
            current = ""
        else:
            current += ch
    if current:
        tokens.append(current)
    out = []
    for raw in tokens:
        optional = raw.startswith("[") and raw.endswith("]")
        required = raw.startswith("<") and raw.endswith(">")
        inner = raw[1:-1] if optional or required else raw
        name, type_ = inner.strip(), None
        if ":" in inner:
            left, right = (p.strip() for p in inner.split(":", 1))
            if re.fullmatch(_TYPES, left):
                name, type_ = right, left
            elif re.fullmatch(_TYPES, right):
                name, type_ = left, right
        out.append({"raw": raw, "name": name, "type": type_, "optional": optional})
    return out


def _bool_property(cmd: _Card, name: str, default: bool | None) -> bool:
    prop = cmd.member("property_declaration", name)
    if prop is None:
        if default is None:
            raise cmd.fail(None, f"no {name}")
        return default
    value = _text(cmd.arrow_value(prop))
    if value not in ("true", "false"):
        raise cmd.fail(prop, f"unrecognised {name} value {value!r}")
    return value == "true"


def _string_property(cmd: _Card, name: str) -> str:
    prop = cmd.member("property_declaration", name)
    if prop is None:
        raise cmd.fail(None, f"no {name}")
    return string_value(cmd.arrow_value(prop))


def base_debug_only(source: str | bytes) -> bool:
    return _bool_property(_Card(source, BASE_CLASS), "DebugOnly", None)


def _enters_room(cmd: _Card, process: Node) -> bool:
    """Whether ``Process``, or a method of this class it calls, enters a room or act."""
    methods: dict[str, list[Node]] = {}
    for m in cmd.members:
        if m.type == "method_declaration":
            methods.setdefault(_text(_field(m, "name")), []).append(m)
    seen: set[str] = set()
    todo = [process]
    while todo:
        for node in _walk(todo.pop()):
            if node.type != "identifier":
                continue
            name = _text(node)
            if name in ENTERS_ROOM:
                return True
            if name in methods and name not in seen:
                seen.add(name)
                todo.extend(methods[name])
    return False


def extract_command(
    source: str | bytes, class_name: str, debug_default: bool
) -> dict[str, Any]:
    """One command: name, usage, description, flags and a hash of its ``Process``."""
    cmd = _Card(source, class_name)
    process = cmd.member("method_declaration", "Process")
    if process is None or process.child_by_field_name("body") is None:
        raise cmd.fail(None, "no Process body")
    body = _text(process)
    args = _string_property(cmd, "Args")
    return {
        "class": class_name,
        "name": _string_property(cmd, "CmdName"),
        "args": args,
        "parsed_args": parse_args(args),
        "description": _string_property(cmd, "Description"),
        "networked": _bool_property(cmd, "IsNetworked", None),
        "debug_only": _bool_property(cmd, "DebugOnly", debug_default),
        "enters_room": _enters_room(cmd, process),
        "process_sha256": hashlib.sha256(" ".join(body.split()).encode()).hexdigest(),
    }


def _is_abstract(cls: Node) -> bool:
    return any(c.type == "modifier" and _text(c) == "abstract" for c in cls.children)


def _top_level_classes(root: Node) -> list[Node]:
    out = []

    def visit(node: Node) -> None:
        for child in node.children:
            if child.type == "class_declaration":
                out.append(child)
            elif child.type in (
                "namespace_declaration",
                "file_scoped_namespace_declaration",
                "declaration_list",
            ):
                visit(child)

    visit(root)
    return out


def _base_names(cls: Node) -> list[str]:
    bases = [c for c in cls.children if c.type == "base_list"]
    return [_text(b).split("<")[0] for b in _named(bases[0])] if bases else []


def command_classes(directory: Path) -> list[str]:
    """Every concrete ``AbstractConsoleCmd`` subclass declared in ``directory``."""
    found = []
    for path in sorted(directory.glob("*.cs")):
        for cls in _top_level_classes(_parse(path.read_bytes())):
            if BASE_CLASS in _base_names(cls) and not _is_abstract(cls):
                found.append(_text(_field(cls, "name")))
    return found


def model_ids(directory: Path, base: str) -> list[str]:
    """``Slugify(class name)`` of every concrete top-level class in ``directory``."""
    ids = set()
    for path in sorted(directory.glob("*.cs")):
        for cls in _top_level_classes(_parse(path.read_bytes())):
            if not _is_abstract(cls):
                ids.add(slugify(_text(_field(cls, "name"))))
    if not ids:
        raise ExtractError(f"{directory}: no {base} classes found")
    return sorted(ids)


def pile_types(source: str | bytes) -> list[str]:
    enums = [n for n in _walk(_parse(source)) if n.type == "enum_declaration"]
    if len(enums) != 1:
        raise ExtractError(f"{PILE_TYPE_FILE}: expected one enum, found {len(enums)}")
    body = _field(enums[0], "body")
    return [
        _text(_field(m, "name"))
        for m in _named(body)
        if m.type == "enum_member_declaration"
    ]


def build_catalog(
    decompiled: str | Path, pck: str | Path, game_version: str
) -> dict[str, Any]:
    decompiled, pck = Path(decompiled), Path(pck)
    release = read_release_info(pck)
    if str(release.get("version", "")).removeprefix("v") != game_version:
        raise ExtractError(
            f"--game-version {game_version} but {pck.parent / 'release_info.json'} "
            f"says {release.get('version')}"
        )
    console = decompiled / CONSOLE_DIR
    listed = subtype_classes((console / SUBTYPES_FILE).read_bytes())
    declared = command_classes(console)
    if set(listed) != set(declared) or len(declared) != len(set(declared)):
        raise ExtractError(
            f"{SUBTYPES_FILE} lists {len(listed)} commands but {CONSOLE_DIR} declares "
            f"{len(declared)}: only listed {sorted(set(listed) - set(declared))}, "
            f"only declared {sorted(set(declared) - set(listed))}"
        )
    debug_default = base_debug_only((console / f"{BASE_CLASS}.cs").read_bytes())
    commands: dict[str, Any] = {}
    for cls in sorted(listed):
        entry = extract_command(
            (console / f"{cls}.cs").read_bytes(), cls, debug_default
        )
        name = entry.pop("name")
        if name in commands or name == "help":
            raise ExtractError(f"{cls}: command name {name!r} is used twice")
        commands[name] = entry
    commands["help"] = dict(HELP)
    ids = {
        domain: model_ids(decompiled / directory, base)
        for domain, (directory, base) in ID_DOMAINS.items()
    }
    ids["piles"] = pile_types((decompiled / PILE_TYPE_FILE).read_bytes())
    return {
        "game_version": game_version,
        "game_commit": release.get("commit"),
        "source": SOURCE,
        "commands": commands,
        "ids": ids,
    }


def catalog_path(game_version: str) -> Path:
    root = Path(__file__).resolve().parents[3]
    return (
        root
        / "crates"
        / "sts-bridge"
        / "catalog"
        / game_version
        / "console_commands.json"
    )


def dumps(catalog: Mapping[str, Any]) -> str:
    return json.dumps(catalog, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def write_catalog(catalog: Mapping[str, Any], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(dumps(catalog), encoding="utf-8")


def load_catalog(path: str | Path) -> dict[str, Any]:
    data: dict[str, Any] = json.loads(Path(path).read_text(encoding="utf-8"))
    return data


def diff_catalogs(old: Mapping[str, Any], new: Mapping[str, Any]) -> FixtureDiff:
    """Commands added, removed and changed field by field, and ids added or removed."""
    out = diff_fixtures({"cards": old["commands"]}, {"cards": new["commands"]})
    for domain in sorted(set(old["ids"]) | set(new["ids"])):
        a, b = set(old["ids"].get(domain, [])), set(new["ids"].get(domain, []))
        out.added += [f"ids.{domain}.{i}" for i in sorted(b - a)]
        out.removed += [f"ids.{domain}.{i}" for i in sorted(a - b)]
    return out


__all__ = [
    "ExtractError",
    "FixtureDiff",
    "build_catalog",
    "catalog_path",
    "diff_catalogs",
    "dumps",
    "extract_command",
    "load_catalog",
    "parse_args",
    "subtype_classes",
    "write_catalog",
]
