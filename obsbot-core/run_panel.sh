#!/bin/bash
# Launch (or relaunch) the OBSBOT panel under WSLg with auto-restart.
# WSLg's software GL stack can get its Wayland connection reset during odd
# window-manager events; the panel then dies "Connection reset by peer".
# This wrapper respawns it after 2 s unless it exits cleanly (rc 0), and
# refuses to start a second copy while one is already running.
#   usage: from this directory, after `cargo build --offline --example panel --features panel`:
#          bash run_panel.sh
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="$DIR/target/debug/examples/panel"
export WAYLAND_DISPLAY=wayland-0 XDG_RUNTIME_DIR=/mnt/wslg/runtime-dir DISPLAY=:0

if pgrep -x panel >/dev/null; then
    echo "panel already running (pid $(pgrep -x panel | head -1))"
    exit 0
fi
[ -x "$BIN" ] || { echo "missing $BIN — run: cargo build --features panel,codec --example panel"; exit 1; }

while :; do
    "$BIN" >>/tmp/panel_gui.log 2>&1
    rc=$?
    echo "$(date +%T) panel exited rc=$rc" >>/tmp/panel_gui.log
    [ "$rc" -eq 0 ] && break        # deliberate close: stay down
    sleep 2                          # crash: respawn
done
