#!/usr/bin/env bash
# Start (or restart) the sts_sim analysis server.
#
# Usage:
#   ./scripts/start-server.sh            # pull latest, rebuild if needed, start
#   ./scripts/start-server.sh --no-pull  # skip git pull (useful mid-dev)
#
# Logs go to ~/.local/share/sts-sim/server.log

set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="$HOME/.local/share/sts-sim"
LOG_FILE="$LOG_DIR/server.log"
VENV="$REPO/.venv"

mkdir -p "$LOG_DIR"

# ── helpers ──────────────────────────────────────────────────────────────────

log() { echo "[start-server] $*"; }

kill_existing() {
    local pid
    pid=$(ss -tlnp 2>/dev/null | awk '/8765/ {match($0,/pid=([0-9]+)/,a); if(a[1]) print a[1]}' | head -1)
    if [[ -n "$pid" ]]; then
        log "killing existing server (pid $pid)"
        kill "$pid" 2>/dev/null || true
        sleep 1
    fi
}

rust_so() {
    # Path to the compiled extension inside the venv
    find "$VENV" -name "_sts_sim*.so" 2>/dev/null | head -1
}

rust_sources_newer_than_so() {
    local so
    so=$(rust_so)
    [[ -z "$so" ]] && return 0  # no .so → need build
    # True if any src/*.rs or Cargo.toml is newer than the .so
    find "$REPO/src" "$REPO/Cargo.toml" -newer "$so" -name "*.rs" -o -newer "$so" -name "Cargo.toml" \
        2>/dev/null | grep -q .
}

# ── main ─────────────────────────────────────────────────────────────────────

cd "$REPO"

if [[ "${1:-}" != "--no-pull" ]]; then
    log "pulling latest main…"
    git pull --rebase origin main
fi

if rust_sources_newer_than_so; then
    log "Rust sources changed — rebuilding extension…"
    source "$VENV/bin/activate"
    maturin develop --release 2>&1 | tee -a "$LOG_FILE"
else
    log "Rust sources unchanged — skipping rebuild"
    source "$VENV/bin/activate"
fi

kill_existing

log "starting server (logs → $LOG_FILE)"
nohup python -m sts_sim.server >> "$LOG_FILE" 2>&1 &

# Wait up to 5 s for the port to open
for i in $(seq 1 10); do
    sleep 0.5
    if ss -tlnp 2>/dev/null | grep -q 8765; then
        log "server is up on port 8765 ✓"
        exit 0
    fi
done

log "ERROR: server did not start within 5 s — check $LOG_FILE"
exit 1
