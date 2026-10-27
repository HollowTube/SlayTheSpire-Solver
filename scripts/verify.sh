#!/usr/bin/env bash
# The checks CI runs. Run scripts/dev-setup.sh first; works from any directory.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [ ! -x .venv/bin/python ]; then
  echo "verify.sh: .venv/bin/python not found; run scripts/dev-setup.sh first" >&2
  exit 1
fi

# Fail on Rust warnings like CI does.
export RUSTFLAGS="${RUSTFLAGS:--D warnings}"
# Absolute: pyo3's build script runs from another directory.
export PYO3_PYTHON="$root/.venv/bin/python"

step() {
  echo "==> $*"
  "$@"
}

step .venv/bin/python scripts/gen_ids.py --check
step cargo check
step .venv/bin/ruff check python/ tests/ integtests/
step .venv/bin/ruff format --check python/ tests/ integtests/
step .venv/bin/mypy python/ tests/ --ignore-missing-imports
step .venv/bin/pytest tests/ -q
step cargo test -p sts-ids -p sts-bridge
