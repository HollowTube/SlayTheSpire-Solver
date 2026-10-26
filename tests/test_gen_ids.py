"""Staleness guard for the one generated file, ``sim/_registry.pyi``."""

import subprocess
import sys
from pathlib import Path

from sts_sim.sim.names import CardName

STUB = Path("python/sts_sim/sim/_registry.pyi")

# Resolved through the stub: mypy fails here if CardName loses its members.
_STRIKE: str = CardName.STRIKE.value


def _check(stub: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "scripts/gen_ids.py", "--check", "--stub", str(stub)],
        capture_output=True,
        text=True,
    )


def test_registry_stub_is_up_to_date():
    result = _check(STUB)
    assert result.returncode == 0, (
        "_registry.pyi is stale. Run `python scripts/gen_ids.py` to regenerate.\n"
        + result.stderr
    )


def test_check_fails_on_a_stale_stub(tmp_path):
    stale = tmp_path / "_registry.pyi"
    stale.write_text(STUB.read_text().replace('    STRIKE = "Strike"\n', ""))
    result = _check(stale)
    assert result.returncode == 1
    assert "STALE" in result.stderr
