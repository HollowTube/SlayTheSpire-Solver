#!/usr/bin/env python3
"""Ironclad card parity: extract game card data, diff two extractions, report coverage.

    python scripts/card_parity.py extract --game-version 0.107.1 \\
        --decompiled ~/projects/sts2-modding-mcp/decompiled \\
        --pck "<Steam>/Slay the Spire 2/SlayTheSpire2.pck"
    python scripts/card_parity.py diff tests/fixtures/game/0.107.1/ironclad_cards.json \\
        tests/fixtures/game/<new>/ironclad_cards.json
    python scripts/card_parity.py report [--game-version 0.107.1]

`--decompiled` and `--pck` default to $STS2_DECOMPILED and $STS2_PCK. Both
inputs are local and read-only; the game doesn't need to run.
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

from sts_sim.parity.fixture import (  # noqa: E402
    GAME_VERSION,
    build_fixture,
    diff_fixtures,
    fixture_path,
    load_fixture,
    write_fixture,
)


def _required_path(value: str | None, flag: str, env: str) -> Path:
    if not value:
        sys.exit(f"card_parity: pass {flag} or set ${env}")
    path = Path(value).expanduser()
    if not path.exists():
        sys.exit(f"card_parity: {flag} {path} does not exist")
    return path


def cmd_extract(args: argparse.Namespace) -> int:
    decompiled = _required_path(args.decompiled, "--decompiled", "STS2_DECOMPILED")
    pck = _required_path(args.pck, "--pck", "STS2_PCK")
    fixture = build_fixture(decompiled, pck, args.game_version)
    out = Path(args.out) if args.out else fixture_path(args.game_version)
    write_fixture(fixture, out)
    print(f"wrote {len(fixture['cards'])} cards to {out}")
    return 0


def cmd_diff(args: argparse.Namespace) -> int:
    diff = diff_fixtures(load_fixture(args.old), load_fixture(args.new))
    for line in diff.lines():
        print(line)
    if not diff:
        print("no changes")
    return 1 if diff else 0


def cmd_report(args: argparse.Namespace) -> int:
    from sts_sim import card_spec
    from sts_sim.parity.report import coverage, format_report

    fixture = load_fixture(fixture_path(args.game_version))
    print(format_report(coverage(fixture, card_spec), f"card parity, game {args.game_version}"))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)
    ex = sub.add_parser("extract", help="write tests/fixtures/game/<version>/ironclad_cards.json")
    ex.add_argument("--game-version", required=True, help="must match release_info.json next to the PCK")
    ex.add_argument("--decompiled", default=os.environ.get("STS2_DECOMPILED"))
    ex.add_argument("--pck", default=os.environ.get("STS2_PCK"))
    ex.add_argument("--out", help="output path (default: the fixture path for the version)")
    ex.set_defaults(func=cmd_extract)
    df = sub.add_parser("diff", help="changed fields and added/removed cards; exit 1 if any")
    df.add_argument("old")
    df.add_argument("new")
    df.set_defaults(func=cmd_diff)
    rp = sub.add_parser("report", help="print the coverage buckets")
    rp.add_argument("--game-version", default=GAME_VERSION)
    rp.set_defaults(func=cmd_report)
    args = parser.parse_args()
    return int(args.func(args))


if __name__ == "__main__":
    sys.exit(main())
