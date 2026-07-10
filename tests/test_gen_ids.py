"""Staleness guard: fail if committed files diverge from generator output."""

import subprocess
import sys


def test_generated_regions_are_up_to_date():
    result = subprocess.run(
        [sys.executable, "scripts/gen_ids.py", "--check"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, (
        "Generated regions are stale. Run `python scripts/gen_ids.py` to regenerate.\n"
        + result.stderr
    )
