from __future__ import annotations

from typing import Any


def pytest_terminal_summary(terminalreporter: Any) -> None:
    from sts_sim import card_spec
    from sts_sim.parity.fixture import GAME_VERSION, fixture_path, load_fixture
    from sts_sim.parity.report import coverage, format_report

    fixture = load_fixture(fixture_path(GAME_VERSION))
    terminalreporter.write_sep("-", "card parity coverage")
    terminalreporter.write_line(
        format_report(coverage(fixture, card_spec), f"Ironclad, game {GAME_VERSION}")
    )
