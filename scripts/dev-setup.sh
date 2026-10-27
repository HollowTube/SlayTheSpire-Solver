#!/usr/bin/env bash
# Create the repo's Python 3.12 venv and install it with the dev extras.
# PyO3 0.23 rejects Python 3.14, which a bare `uv venv` may pick.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Same flags as verify.sh and CI, so the maturin build shares their artifacts.
export RUSTFLAGS="${RUSTFLAGS:--D warnings}"

uv venv --python 3.12 --clear
# Explicit target: uv would otherwise honour an already-active VIRTUAL_ENV.
uv pip install --python "$root/.venv/bin/python" -e ".[dev]"
