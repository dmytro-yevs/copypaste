#!/usr/bin/env bash
# Run one command inside a disposable Fedora Plasma 6 KWin desktop session.
set -euo pipefail

SESSION=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --session) SESSION="$2"; shift 2 ;;
    --) shift; break ;;
    *) echo "ERROR: unknown Fedora session argument: $1" >&2; exit 2 ;;
  esac
done
[[ "$SESSION" == x11 || "$SESSION" == wayland ]] || { echo "ERROR: session must be x11 or wayland" >&2; exit 2; }
[[ "$#" -gt 0 ]] || { echo "ERROR: missing qualification command" >&2; exit 2; }

runtime="$(mktemp -d)"
cleanup() {
  [[ -n "${kwin_pid:-}" ]] && kill "$kwin_pid" 2>/dev/null || true
  [[ -n "${xserver_pid:-}" ]] && kill "$xserver_pid" 2>/dev/null || true
  rm -rf "$runtime"
}
trap cleanup EXIT
chmod 700 "$runtime"
export XDG_RUNTIME_DIR="$runtime"
export XDG_CURRENT_DESKTOP=KDE
export XDG_SESSION_DESKTOP=KDE
export XDG_SESSION_TYPE="$SESSION"
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe

wait_for() {
  local test_command="$1"
  for _ in $(seq 1 100); do
    eval "$test_command" && return 0
    sleep 0.1
  done
  echo "ERROR: Plasma 6 $SESSION session did not become ready" >&2
  for log in "$runtime"/*.log; do
    [[ -f "$log" ]] || continue
    echo "--- $(basename "$log") ---" >&2
    tail -n 80 "$log" >&2
  done
  return 1
}

case "$SESSION" in
  x11)
    Xvfb :99 -screen 0 1280x800x24 -nolisten tcp >"$runtime/xserver.log" 2>&1 &
    xserver_pid=$!
    export DISPLAY=:99
    wait_for 'xdpyinfo -display "$DISPLAY" >/dev/null 2>&1'
    kwin_x11 --replace >"$runtime/kwin.log" 2>&1 &
    kwin_pid=$!
    wait_for 'kill -0 "$kwin_pid" 2>/dev/null'
    xprop -root -display "$DISPLAY" >/dev/null
    ;;
  wayland)
    kwin_wayland --virtual --no-lockscreen >"$runtime/kwin.log" 2>&1 &
    kwin_pid=$!
    wait_for 'kill -0 "$kwin_pid" 2>/dev/null'
    wait_for 'find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name "wayland-*" | grep -q .'
    export WAYLAND_DISPLAY="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-*' -printf '%f\n' | head -n1)"
    ;;
esac

kpackagetool6 --version >/dev/null
if [[ "$SESSION" == x11 ]]; then
  kwin_version="$(kwin_x11 --version)"
else
  kwin_version="$(kwin_wayland --version)"
fi
printf 'desktop=%s\nsession=%s\nkwin=%s\n' "$XDG_CURRENT_DESKTOP" "$XDG_SESSION_TYPE" "$kwin_version"
exec "$@"
