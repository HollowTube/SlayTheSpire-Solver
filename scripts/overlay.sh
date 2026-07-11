#!/usr/bin/env bash
# overlay.sh — manage the bridge mod and sts-sim analysis server.
#
# Usage:
#   ./scripts/overlay.sh              # refresh WSL IP and start server
#   ./scripts/overlay.sh build        # build and install mod (game must be off)
#   ./scripts/overlay.sh hot_reload   # build mod and hot-reload while game is running
#   ./scripts/overlay.sh server       # refresh WSL IP and start server
#   ./scripts/overlay.sh launch       # launch STS2 via Steam
#   ./scripts/overlay.sh stop         # close STS2 gracefully
#   ./scripts/overlay.sh fresh        # stop → build → launch → server

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GAME_DIR="/mnt/d/SteamLibrary/steamapps/common/Slay the Spire 2"
MOD_DIR="$GAME_DIR/mods/stssimbridgemod"
STEAM_EXE="/mnt/c/Program Files (x86)/Steam/steam.exe"
STS2_APP_ID=2868840

# Windows .exe interop requires binfmt_misc WSLInterop handler.
# It's registered by WSL on login but missing in non-interactive contexts.
_ensure_interop() {
    if [ ! -f /proc/sys/fs/binfmt_misc/WSLInterop ]; then
        echo "==> Registering WSL Windows interop handler..."
        echo ':WSLInterop:M::MZ::/init:PF' | sudo tee /proc/sys/fs/binfmt_misc/register > /dev/null
    fi
}

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
    _ensure_interop
    echo "==> Launching STS2 via Steam..."
    # Start Steam if not running, then use -applaunch for proper Steam context
    "$STEAM_EXE" -applaunch $STS2_APP_ID &
    echo "    Launched (Steam will open the game)"
}

stop_game() {
    _ensure_interop
    echo "==> Stopping STS2..."
    if taskkill.exe /IM SlayTheSpire2.exe 2>/dev/null; then
        echo "    Stopped gracefully"
    else
        echo "    Not running (or already closed)"
    fi
}

hot_reload_mod() {
    echo "==> Building bridge mod..."
    (cd "$REPO_ROOT/bridge_mod" && dotnet build -c Release)

    # Load the new DLL directly from the build output via WSL UNC path — the
    # installed DLL is locked by the running game so we can't overwrite it.
    local dll_wsl="$REPO_ROOT/bridge_mod/bin/Release/net9.0/stssimbridgemod.dll"
    local dll_win="\\\\wsl\$\\${WSL_DISTRO_NAME}$(echo "$dll_wsl" | sed 's|/|\\|g')"
    echo "==> Hot-reloading mod from build output..."
    echo "    $dll_win"
    sts2 console "bridge_hot_reload $dll_win"
}

CMD="${1:-server}"
case "$CMD" in
    build)      build_mod ;;
    hot_reload) hot_reload_mod ;;
    launch)     launch_game ;;
    stop)       stop_game ;;
    fresh)
        _ensure_interop
        stop_game
        echo "==> Waiting for game to close..."
        sleep 3
        build_mod
        launch_game
        echo "==> Waiting for game to start..."
        sleep 25
        _write_host_file
        echo "==> Starting sts-sim analysis server..."
        "$REPO_ROOT/scripts/start-server.sh" --no-pull
        ;;
    server)  start_server ;;
    *)
        echo "Usage: $0 [build|hot_reload|server|launch|stop|fresh]" >&2
        exit 1
        ;;
esac
