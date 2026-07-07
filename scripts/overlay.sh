#!/usr/bin/env bash
# overlay.sh — build the bridge mod and/or start the sts-sim analysis server.
#
# Usage:
#   ./scripts/overlay.sh          # build mod + start server
#   ./scripts/overlay.sh build    # build and install mod only
#   ./scripts/overlay.sh server   # start server only

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MOD_DIR="/mnt/d/SteamLibrary/steamapps/common/Slay the Spire 2/mods/stssimbridgemod"

build_mod() {
    echo "==> Building bridge mod..."
    (cd "$REPO_ROOT/bridge_mod" && dotnet build -c Release)

    echo "==> Installing to mod directory..."
    cp "$REPO_ROOT/bridge_mod/bin/Release/net9.0/stssimbridgemod.dll" "$MOD_DIR/stssimbridgemod.dll"
    echo "    Installed: $MOD_DIR/stssimbridgemod.dll"

    echo "==> Writing WSL IP to sts_sim_host.txt..."
    WSL_IP=$(ip addr show eth0 | grep 'inet ' | awk '{print $2}' | cut -d/ -f1)
    echo "$WSL_IP" > "$MOD_DIR/sts_sim_host.txt"
    echo "    sts_sim_host.txt <- $WSL_IP"
}

start_server() {
    echo "==> Updating WSL IP in sts_sim_host.txt..."
    WSL_IP=$(ip addr show eth0 | grep 'inet ' | awk '{print $2}' | cut -d/ -f1)
    echo "$WSL_IP" > "$MOD_DIR/sts_sim_host.txt"
    echo "    sts_sim_host.txt <- $WSL_IP"

    echo "==> Starting sts-sim analysis server on 0.0.0.0:8765..."
    exec "$REPO_ROOT/.venv/bin/sts-sim-server" --host 0.0.0.0
}

CMD="${1:-all}"
case "$CMD" in
    build)  build_mod ;;
    server) start_server ;;
    all)    build_mod && start_server ;;
    *)      echo "Usage: $0 [build|server|all]" >&2; exit 1 ;;
esac
