import sys
from pathlib import Path

# Ensure tests in this worktree import from the worktree's own python/
# directory rather than from the editable-install .pth that points at the
# main checkout. Without this, `uv pip install -e .` run from the main repo
# wins and local worktree changes to python/sts_sim/ are invisible to pytest.
sys.path.insert(0, str(Path(__file__).parent / "python"))
