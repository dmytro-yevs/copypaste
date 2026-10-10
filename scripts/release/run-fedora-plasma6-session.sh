#!/usr/bin/env bash
# Run one command inside a disposable Fedora Plasma 6 KWin desktop session.
set -euo pipefail

SESSION=""
COMPANION_SOURCE=""
COMPOSITOR_RUNTIME=""
COMPOSITOR_RUNTIME_BINDING=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --session) SESSION="$2"; shift 2 ;;
    --companion-source) COMPANION_SOURCE="$2"; shift 2 ;;
    --compositor-runtime) COMPOSITOR_RUNTIME="$2"; shift 2 ;;
    --compositor-runtime-binding) COMPOSITOR_RUNTIME_BINDING="$2"; shift 2 ;;
    --) shift; break ;;
    *) echo "ERROR: unknown Fedora session argument: $1" >&2; exit 2 ;;
  esac
done
[[ "$SESSION" == x11 || "$SESSION" == wayland ]] || { echo "ERROR: session must be x11 or wayland" >&2; exit 2; }
[[ "$#" -gt 0 ]] || { echo "ERROR: missing qualification command" >&2; exit 2; }
[[ -d "$COMPANION_SOURCE" ]] || { echo "ERROR: exact packaged companion source is required" >&2; exit 2; }
[[ -d "$COMPOSITOR_RUNTIME" && -f "$COMPOSITOR_RUNTIME_BINDING" && ! -L "$COMPOSITOR_RUNTIME" && ! -L "$COMPOSITOR_RUNTIME_BINDING" ]] || { echo "ERROR: authenticated compositor runtime input is required" >&2; exit 2; }
runtime_package="$(jq -er '.package.name | select(type == "string" and test("^[A-Za-z0-9._+-]+\\.rpm$"))' "$COMPOSITOR_RUNTIME_BINDING")"
runtime_id="$(jq -er '.runtime_id | select(type == "string" and test("^[a-z0-9.-]+$"))' "$COMPOSITOR_RUNTIME_BINDING")"
dnf --assumeyes install "$COMPOSITOR_RUNTIME/$runtime_package"
rpm -q "copypaste-compositor-runtime-$runtime_id" >/dev/null
python3 /work/scripts/release/verify-linux-compositor-runtime.py installed --binding "$COMPOSITOR_RUNTIME_BINDING" --root /
COMPOSITOR_LAUNCHER="/usr/lib/copypaste/compositor-runtime/bin/copypaste-compositor-session-$runtime_id"
runtime_root="/usr/lib/copypaste/compositor-runtime/$runtime_id"
qualification_entrypoint="$(jq -er '.qualification.entrypoint | select(type == "string" and test("^[A-Za-z0-9._/+@-]+$"))' "/usr/share/copypaste/compositor-runtime/$runtime_id.receipt.json")"
COMPOSITOR_QUALIFICATION_ENTRYPOINT="$runtime_root/$qualification_entrypoint"
[[ -x "$COMPOSITOR_QUALIFICATION_ENTRYPOINT" ]] || { echo "ERROR: receipt-listed KWin qualification entrypoint is missing" >&2; exit 1; }

runtime="$(mktemp -d)"
cleanup() {
  [[ -n "${shell_pid:-}" ]] && kill "$shell_pid" 2>/dev/null || true
  [[ -n "${kwin_pid:-}" ]] && kill "$kwin_pid" 2>/dev/null || true
  [[ -n "${xserver_pid:-}" ]] && kill "$xserver_pid" 2>/dev/null || true
  [[ -n "${dbus_pid:-}" ]] && kill "$dbus_pid" 2>/dev/null || true
  rm -rf "$runtime"
}
trap cleanup EXIT
chmod 700 "$runtime"
export XDG_RUNTIME_DIR="$runtime"
export XDG_CURRENT_DESKTOP=KDE
export XDG_SESSION_DESKTOP=KDE
export XDG_SESSION_TYPE="$SESSION"
export XDG_DATA_HOME="$runtime/data"
export XDG_CONFIG_HOME="$runtime/config"
export XDG_CACHE_HOME="$runtime/cache"
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe

if [[ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]]; then
  mapfile -t bus < <(dbus-daemon --session --fork --print-address=1 --print-pid=1)
  [[ "${#bus[@]}" == 2 ]] || { echo "ERROR: could not start session bus" >&2; exit 1; }
  export DBUS_SESSION_BUS_ADDRESS="${bus[0]}"
  dbus_pid="${bus[1]}"
fi

script="$COMPANION_SOURCE/kde-kwin-script"
[[ -d "$script" ]] || { echo "ERROR: packaged KDE companion is missing" >&2; exit 1; }
kpackagetool6 --type KWin/Script --install "$script"
kwriteconfig6 --file "$XDG_CONFIG_HOME/kwinrc" --group Plugins --key copypaste-quick-pasteEnabled true

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
    export COPYPASTE_COMPOSITOR_EXECUTION=stock-x11
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
    export COPYPASTE_COMPOSITOR_EXECUTION=private-wayland
    export LD_LIBRARY_PATH="$runtime_root/lib:$runtime_root/lib64:$runtime_root/usr/lib:$runtime_root/usr/lib64${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    "$COMPOSITOR_QUALIFICATION_ENTRYPOINT" >"$runtime/kwin.log" 2>&1 &
    kwin_pid=$!
    wait_for 'kill -0 "$kwin_pid" 2>/dev/null'
    wait_for 'find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name "wayland-*" | grep -q .'
    export WAYLAND_DISPLAY="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-*' -printf '%f\n' | head -n1)"
    export COPYPASTE_COMPOSITOR_SESSION_RECORD="$runtime/compositor-session-record.json"
    python3 /work/scripts/release/record-linux-compositor-session.py \
      --binding "$COMPOSITOR_RUNTIME_BINDING" --root / --pid "$kwin_pid" --session wayland \
      --output "$COPYPASTE_COMPOSITOR_SESSION_RECORD"
    ;;
esac

dbus-update-activation-environment \
  DBUS_SESSION_BUS_ADDRESS DISPLAY WAYLAND_DISPLAY XDG_RUNTIME_DIR XDG_DATA_HOME \
  XDG_CONFIG_HOME XDG_CURRENT_DESKTOP XDG_SESSION_DESKTOP XDG_SESSION_TYPE
keyring_password="$(openssl rand -hex 24)"
printf '%s\n' "$keyring_password" | gnome-keyring-daemon --login >/dev/null
eval "$(gnome-keyring-daemon --start --components=secrets)"
printf '%s' "$keyring_password" | secret-tool store --label='CopyPaste qualification keyring' copypaste-qualification default >/dev/null
secret-tool lookup copypaste-qualification default >/dev/null
unset keyring_password
plasmashell --replace >"$runtime/plasmashell.log" 2>&1 &
shell_pid=$!
wait_for 'kill -0 "$shell_pid" 2>/dev/null'

kpackagetool6 --version >/dev/null
printf 'desktop=%s\nsession=%s\ncompositor_launcher=%s\n' "$XDG_CURRENT_DESKTOP" "$XDG_SESSION_TYPE" "$COMPOSITOR_LAUNCHER"
exec "$@"
