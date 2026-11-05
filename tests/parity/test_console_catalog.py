"""Console catalog extractor tests. All C# here is synthetic, written in the decompile's shapes."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

from sts_sim.parity.console_catalog import (
    ExtractError,
    build_catalog,
    catalog_path,
    diff_catalogs,
    dumps,
    extract_command,
    load_catalog,
    parse_args,
    subtype_classes,
)

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "console_catalog.py"
FIVE_ROOM_CHANGERS = {"fight", "event", "room", "act", "ancient"}


def cmd(
    *,
    name: str = "card",
    args: str = '"<card-id:string> [pile:string]"',
    description: str = '"Spawns a card."',
    networked: str = "true",
    extra_props: str = "",
    process: str = 'return new CmdResult(success: true, "ok");',
    extra: str = "",
) -> str:
    return f"""
namespace MegaCrit.Sts2.Core.DevConsole.ConsoleCommands;
public class FooConsoleCmd : AbstractConsoleCmd
{{
    public override string CmdName => "{name}";
    public override string Args => {args};
    public override string Description => {description};
    public override bool IsNetworked => {networked};
    {extra_props}
    public override CmdResult Process(Player? issuingPlayer, string[] args)
    {{
        {process}
    }}
    {extra}
}}"""


def extract(source: str, debug_default: bool = True) -> dict[str, Any]:
    return extract_command(source, "FooConsoleCmd", debug_default)


def test_reads_name_usage_description_and_flags() -> None:
    entry = extract(cmd())
    assert entry["name"] == "card"
    assert entry["args"] == "<card-id:string> [pile:string]"
    assert entry["description"] == "Spawns a card."
    assert entry["networked"] is True
    assert entry["debug_only"] is True
    assert entry["enters_room"] is False
    assert [(a["name"], a["type"], a["optional"]) for a in entry["parsed_args"]] == [
        ("card-id", "string", False),
        ("pile", "string", True),
    ]


def test_debug_only_comes_from_the_override_or_the_base_default() -> None:
    assert (
        extract(cmd(extra_props="public override bool DebugOnly => false;"))[
            "debug_only"
        ]
        is False
    )
    assert extract(cmd(), debug_default=False)["debug_only"] is False


def test_description_concatenation_keeps_literals_and_marks_the_rest() -> None:
    entry = extract(
        cmd(
            description='"Set level. " + string.Join(",", Enum.GetNames<LogType>()) + "."'
        )
    )
    assert (
        entry["description"]
        == 'Set level. {string.Join(",", Enum.GetNames<LogType>())}.'
    )


def test_arg_forms() -> None:
    assert [
        (a["name"], a["type"], a["optional"])
        for a in parse_args("<int:amount> <target-index:int>")
    ] == [("amount", "int", False), ("target-index", "int", False)]
    assert [a["raw"] for a in parse_args("<int|string: act>")] == ["<int|string: act>"]
    assert parse_args("<int|string: act>")[0]["type"] == "int|string"
    assert [a["raw"] for a in parse_args("[add|remove] <relic-id:string>")] == [
        "[add|remove]",
        "<relic-id:string>",
    ]
    assert parse_args("") == []


def test_enters_room_when_process_or_a_method_it_calls_does() -> None:
    direct = cmd(
        process='Task t = RunManager.Instance.EnterRoomDebug(room); return new CmdResult(t, true, "");'
    )
    assert extract(direct)["enters_room"] is True
    helper = cmd(
        process='return new CmdResult(Go(), true, "");',
        extra="private static async Task Go() { await RunManager.Instance.EnterAct(1); }",
    )
    assert extract(helper)["enters_room"] is True
    unrelated = cmd(
        process='return new CmdResult(Go(), true, "");',
        extra="private static async Task Go() { await Other.Instance.Toggle(1); }",
    )
    assert extract(unrelated)["enters_room"] is False


def test_process_hash_ignores_whitespace_and_tracks_the_body() -> None:
    a = extract(cmd(process="int x = 1;   return Ok(x);"))
    b = extract(cmd(process="int x = 1;\n  return Ok(x);"))
    c = extract(cmd(process="int x = 2; return Ok(x);"))
    assert a["process_sha256"] == b["process_sha256"] != c["process_sha256"]


def test_unrecognised_shapes_name_the_class() -> None:
    with pytest.raises(ExtractError, match="FooConsoleCmd: unrecognised IsNetworked"):
        extract(cmd(networked="Compute()"))
    with pytest.raises(ExtractError, match="FooConsoleCmd: no Description"):
        extract(cmd().replace("Description", "Other"))
    with pytest.raises(ExtractError, match="FooConsoleCmd: no Process body"):
        extract(cmd().replace("Process", "Run"))


def subtypes(names: list[str], declared: int | None = None) -> str:
    fields = "\n".join(
        f"    private static readonly Type _t{i} = typeof({n});"
        for i, n in enumerate(names)
    )
    return f"public static class AbstractConsoleCmdSubtypes {{\n{fields}\n" + (
        f"    private static readonly Type[] _subtypes = new Type[{declared or len(names)}] {{ }};\n}}"
    )


def test_subtype_list_is_read_and_checked_against_its_own_count() -> None:
    assert subtype_classes(subtypes(["ACmd", "BCmd"])) == ["ACmd", "BCmd"]
    with pytest.raises(ExtractError, match="2 typeof entries but the array says 3"):
        subtype_classes(subtypes(["ACmd", "BCmd"], declared=3))


def _game(tmp_path: Path, listed: list[str], declared: list[str]) -> tuple[Path, Path]:
    decompiled = tmp_path / "decompiled"
    console = decompiled / "MegaCrit.Sts2.Core.DevConsole.ConsoleCommands"
    console.mkdir(parents=True)
    (console / "AbstractConsoleCmdSubtypes.cs").write_text(subtypes(listed))
    (console / "AbstractConsoleCmd.cs").write_text(
        "public abstract class AbstractConsoleCmd { public virtual bool DebugOnly => true; }"
    )
    for cls in declared:
        (console / f"{cls}.cs").write_text(
            cmd(name=cls.lower()).replace("FooConsoleCmd", cls)
        )
    for directory, base, ids in (
        ("Cards", "CardModel", ["Bash"]),
        ("Encounters", "EncounterModel", ["NibbitsWeak"]),
        ("Powers", "PowerModel", ["VulnerablePower"]),
        ("Relics", "RelicModel", ["BurningBlood"]),
    ):
        d = decompiled / f"MegaCrit.Sts2.Core.Models.{directory}"
        d.mkdir()
        for i in ids:
            (d / f"{i}.cs").write_text(
                f"namespace X;\npublic sealed class {i} : {base} {{ }}"
            )
        (d / f"Abstract{base}.cs").write_text(
            f"namespace X;\npublic abstract class Abstract{base} : {base} {{ }}"
        )
    pile = decompiled / "MegaCrit.Sts2.Core.Entities.Cards"
    pile.mkdir()
    (pile / "PileType.cs").write_text(
        "namespace X;\npublic enum PileType { None, Draw, Hand }"
    )
    pck = tmp_path / "game" / "SlayTheSpire2.pck"
    pck.parent.mkdir()
    pck.write_bytes(b"")
    (pck.parent / "release_info.json").write_text(
        json.dumps({"version": "v1.2.3", "commit": "abc"})
    )
    return decompiled, pck


def test_catalog_lists_every_subtype_class_plus_help_and_the_id_domains(
    tmp_path: Path,
) -> None:
    classes = ["ACmd", "BCmd"]
    decompiled, pck = _game(tmp_path, classes, classes)
    catalog = build_catalog(decompiled, pck, "1.2.3")
    assert sorted(catalog["commands"]) == ["acmd", "bcmd", "help"]
    assert catalog["game_version"] == "1.2.3" and catalog["game_commit"] == "abc"
    assert catalog["ids"] == {
        "cards": ["BASH"],
        "encounters": ["NIBBITS_WEAK"],
        "powers": ["VULNERABLE_POWER"],
        "relics": ["BURNING_BLOOD"],
        "piles": ["None", "Draw", "Hand"],
    }


def test_a_class_missing_from_the_subtype_list_fails_the_extraction(
    tmp_path: Path,
) -> None:
    decompiled, pck = _game(tmp_path, ["ACmd"], ["ACmd", "BCmd"])
    with pytest.raises(ExtractError, match=r"only declared \['BCmd'\]"):
        build_catalog(decompiled, pck, "1.2.3")
    decompiled, pck = _game(tmp_path / "again", ["ACmd", "BCmd"], ["ACmd"])
    with pytest.raises(ExtractError, match=r"only listed \['BCmd'\]"):
        build_catalog(decompiled, pck, "1.2.3")


def test_a_version_that_disagrees_with_release_info_fails(tmp_path: Path) -> None:
    decompiled, pck = _game(tmp_path, ["ACmd"], ["ACmd"])
    with pytest.raises(ExtractError, match="--game-version 9.9.9"):
        build_catalog(decompiled, pck, "9.9.9")


def _catalog(**commands: dict[str, Any]) -> dict[str, Any]:
    return {"game_version": "1", "commands": commands, "ids": {"powers": ["A_POWER"]}}


def _entry(**changes: Any) -> dict[str, Any]:
    entry = {
        "class": "CardConsoleCmd",
        "args": "<id:string>",
        "description": "Spawns.",
        "enters_room": False,
        "process_sha256": "aa",
    }
    return entry | changes


@pytest.mark.parametrize(
    ("change", "path"),
    [
        ({"args": "<id:string> <n:int>"}, "args"),
        ({"description": "Spawns two."}, "description"),
        ({"process_sha256": "bb"}, "process_sha256"),
        ({"enters_room": True}, "enters_room"),
    ],
)
def test_diff_names_the_changed_field(change: dict[str, Any], path: str) -> None:
    diff = diff_catalogs(_catalog(card=_entry()), _catalog(card=_entry(**change)))
    assert diff
    assert [p for p, _, _ in diff.changed["card"]] == [path]


def test_diff_lists_added_and_removed_commands_and_ids() -> None:
    old = _catalog(card=_entry(), gone=_entry())
    new = _catalog(card=_entry(), fresh=_entry())
    new["ids"] = {"powers": ["B_POWER"]}
    diff = diff_catalogs(old, new)
    assert diff.lines() == [
        "+ fresh",
        "+ ids.powers.B_POWER",
        "- gone",
        "- ids.powers.A_POWER",
    ]
    assert not diff_catalogs(old, json.loads(json.dumps(old)))


def test_cli_diff_exits_one_on_a_change_and_names_the_field(tmp_path: Path) -> None:
    old, new = tmp_path / "old.json", tmp_path / "new.json"
    old.write_text(dumps(_catalog(card=_entry())))
    new.write_text(dumps(_catalog(card=_entry(args="<id:string> [pile:string]"))))
    run = subprocess.run(
        [sys.executable, str(SCRIPT), "diff", str(old), str(new)],
        capture_output=True,
        text=True,
    )
    assert run.returncode == 1
    assert run.stdout.splitlines() == [
        "~ card.args: '<id:string>' -> '<id:string> [pile:string]'"
    ]
    same = subprocess.run(
        [sys.executable, str(SCRIPT), "diff", str(old), str(old)],
        capture_output=True,
        text=True,
    )
    assert (same.returncode, same.stdout.strip()) == (0, "no changes")


def test_the_committed_catalog_is_the_games_39_commands_plus_help() -> None:
    catalog = load_catalog(catalog_path("0.107.1"))
    commands = catalog["commands"]
    assert catalog["game_version"] == "0.107.1"
    assert len(commands) == 40
    assert {n for n, c in commands.items() if c["enters_room"]} == FIVE_ROOM_CHANGERS
    for name in (
        "fight",
        "card",
        "power",
        "damage",
        "block",
        "remove_card",
        "instant",
        "relic",
    ):
        assert name in commands
    assert commands["card"]["args"] == "<card-id:string> [pileName:string]"
    assert "NIBBITS_WEAK" in catalog["ids"]["encounters"]
    assert catalog["ids"]["piles"][1:5] == ["Draw", "Hand", "Discard", "Exhaust"]
    assert dumps(catalog) == catalog_path("0.107.1").read_text(encoding="utf-8")
