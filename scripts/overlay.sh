#!/usr/bin/env bash
# overlay.sh — manage the bridge mod and sts-sim analysis server.
#
# Usage:
#   ./scripts/overlay.sh              # refresh WSL IP and start server
#   ./scripts/overlay.sh build        # build and install mod (game must be off)
#   ./scripts/overlay.sh server       # refresh WSL IP and start server
#   ./scripts/overlay.sh launch       # launch STS2
#   ./scripts/overlay.sh stop         # close STS2 gracefully
#   ./scripts/overlay.sh fresh        # stop → build → launch → server

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GAME_DIR="/mnt/d/SteamLibrary/steamapps/common/Slay the Spire 2"
MOD_DIR="$GAME_DIR/mods/stssimbridgemod"
GAME_EXE="$GAME_DIR/SlayTheSpire2.exe"

_wsl_ip() {
    ip addr show eth0 | grep 'inet ' | awk '{print $2}' | cut -d/ -f1
}

_write_host_file() {
    local ip
    ip=$(_wsl_ip)
    echo "$ip" > "$MOD_DIR/sts_sim_host.txt"
    echo "    sts_sim_host.txt <- $ip"
}

build_mod() {
    echo "==> Building bridge mod..."
    (cd "$REPO_ROOT/bridge_mod" && dotnet build -c Release)

    echo "==> Installing to mod directory..."
    cp "$REPO_ROOT/bridge_mod/bin/Release/net9.0/stssimbridgemod.dll" "$MOD_DIR/stssimbridgemod.dll"
    echo "    Installed: $MOD_DIR/stssimbridgemod.dll"

    echo "==> Writing WSL IP to sts_sim_host.txt..."
    _write_host_file
}

start_server() {
    echo "==> Updating WSL IP in sts_sim_host.txt..."
    _write_host_file

    echo "==> Starting sts-sim analysis server on 0.0.0.0:8765..."
    exec "$REPO_ROOT/.venv/bin/sts-sim-server" --host 0.0.0.0
}

launch_game() {
    echo "==> Launching STS2..."
    "$GAME_EXE" &
    echo "    Launched (PID $!)"
}

stop_game() {
    echo "==> Stopping STS2..."
    if taskkill.exe /IM SlayTheSpire2.exe 2>/dev/null; then
        echo "    Stopped gracefully"
    else
        echo "    Not running (or already closed)"
    fi
}

CMD="${1:-server}"
case "$CMD" in
    build)   build_mod ;;
    server)  start_server ;;
    launch)  launch_game ;;
    stop)    stop_game ;;
    fresh)
        stop_game
        echo "==> Waiting for game to close..."
        sleep 3
        build_mod
        launch_game
        echo "==> Waiting for game to start..."
        sleep 5
        start_server
        ;;
    *)
        echo "Usage: $0 [build|server|launch|stop|fresh]" >&2
        exit 1
        ;;
esac
