"""Fixture diff tests. Both fixtures here are small and synthetic."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

from sts_sim.parity.fixture import diff_fixtures, dumps

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "card_parity.py"


def _card(cost: int, damage: int, text: str) -> dict[str, Any]:
    return {
        "class": "X",
        "cost": cost,
        "vars": {
            "Damage": {"class": "DamageVar", "base": damage, "upgraded": damage + 2}
        },
        "text": {"base": text, "upgraded": text},
    }


OLD = {
    "game_version": "1",
    "cards": {
        "BASH": _card(2, 8, "Deal 8 damage."),
        "ANGER": _card(0, 6, "Deal 6 damage."),
        "GONE": _card(1, 1, "x"),
    },
}
NEW = {
    "game_version": "2",
    "cards": {
        "BASH": _card(2, 9, "Deal 9 damage."),
        "ANGER": _card(0, 6, "Deal 6 damage."),
        "FRESH": _card(1, 1, "y"),
    },
}


def test_lists_added_removed_and_changed_fields() -> None:
    diff = diff_fixtures(OLD, NEW)
    assert diff.added == ["FRESH"]
    assert diff.removed == ["GONE"]
    assert diff.changed == {
        "BASH": [
            ("text.base", "Deal 8 damage.", "Deal 9 damage."),
            ("text.upgraded", "Deal 8 damage.", "Deal 9 damage."),
            ("vars.Damage.base", 8, 9),
            ("vars.Damage.upgraded", 10, 11),
        ]
    }


def test_identical_fixtures_have_no_diff() -> None:
    assert not diff_fixtures(OLD, json.loads(json.dumps(OLD)))


def test_cli_prints_the_diff_and_exits_nonzero(tmp_path: Path) -> None:
    old, new = tmp_path / "old.json", tmp_path / "new.json"
    old.write_text(dumps(OLD))
    new.write_text(dumps(NEW))
    run = subprocess.run(
        [sys.executable, str(SCRIPT), "diff", str(old), str(new)],
        capture_output=True,
        text=True,
    )
    assert run.returncode == 1
    assert run.stdout.splitlines() == [
        "+ FRESH",
        "- GONE",
        "~ BASH.text.base: 'Deal 8 damage.' -> 'Deal 9 damage.'",
        "~ BASH.text.upgraded: 'Deal 8 damage.' -> 'Deal 9 damage.'",
        "~ BASH.vars.Damage.base: 8 -> 9",
        "~ BASH.vars.Damage.upgraded: 10 -> 11",
    ]
