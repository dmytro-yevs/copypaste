#!/usr/bin/env bash
# Start a disposable software-rendered GNOME or KDE desktop and run one command.
set -euo pipefail

DESKTOP=""
SESSION=""
COMPANION_SOURCE=""
COMPOSITOR_RUNTIME=""
COMPOSITOR_RUNTIME_BINDING=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --desktop) DESKTOP="$2"; shift 2 ;;
    --session) SESSION="$2"; shift 2 ;;
    --companion-source) COMPANION_SOURCE="$2"; shift 2 ;;
    --compositor-runtime) COMPOSITOR_RUNTIME="$2"; shift 2 ;;
    --compositor-runtime-binding) COMPOSITOR_RUNTIME_BINDING="$2"; shift 2 ;;
    --) shift; break ;;
    *) echo "ERROR: unknown desktop-session argument: $1" >&2; exit 2 ;;
  esac
done
[[ "$#" -gt 0 ]] || { echo "ERROR: missing command for desktop session" >&2; exit 2; }
[[ -d "$COMPANION_SOURCE" ]] || { echo "ERROR: exact packaged companion source is required" >&2; exit 2; }
[[ -d "$COMPOSITOR_RUNTIME" && -f "$COMPOSITOR_RUNTIME_BINDING" && ! -L "$COMPOSITOR_RUNTIME" && ! -L "$COMPOSITOR_RUNTIME_BINDING" ]] || { echo "ERROR: authenticated compositor runtime input is required" >&2; exit 2; }
[[ "$DESKTOP" == GNOME ]] || { echo "ERROR: KDE qualification must use the Fedora sidecar session" >&2; exit 2; }
case "$SESSION" in x11|wayland) ;; *) exit 2 ;; esac
runtime_package="$(jq -er '.package.name | select(type == "string" and test("^[A-Za-z0-9._+-]+\\.deb$"))' "$COMPOSITOR_RUNTIME_BINDING")"
runtime_id="$(jq -er '.runtime_id | select(type == "string" and test("^[a-z0-9.-]+$"))' "$COMPOSITOR_RUNTIME_BINDING")"
sudo apt-get install --yes "$COMPOSITOR_RUNTIME/$runtime_package"
dpkg-query --show "copypaste-compositor-runtime-$runtime_id" >/dev/null
python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/verify-linux-compositor-runtime.py" installed --binding "$COMPOSITOR_RUNTIME_BINDING" --root /
COMPOSITOR_LAUNCHER="/usr/lib/copypaste/compositor-runtime/bin/copypaste-compositor-session-$runtime_id"
runtime_root="/usr/lib/copypaste/compositor-runtime/$runtime_id"
qualification_entrypoint="$(jq -er '.qualification.entrypoint | select(type == "string" and test("^[A-Za-z0-9._/+@-]+$"))' "/usr/share/copypaste/compositor-runtime/$runtime_id.receipt.json")"
COMPOSITOR_QUALIFICATION_ENTRYPOINT="$runtime_root/$qualification_entrypoint"
[[ -x "$COMPOSITOR_QUALIFICATION_ENTRYPOINT" ]] || { echo "ERROR: receipt-listed GNOME qualification entrypoint is missing" >&2; exit 1; }

RUNTIME="$(mktemp -d)"
DISPLAY_NUMBER=99
cleanup() {
  [[ -n "${shell_pid:-}" ]] && kill "$shell_pid" 2>/dev/null || true
  [[ -n "${desktop_pid:-}" ]] && kill "$desktop_pid" 2>/dev/null || true
  [[ -n "${display_pid:-}" ]] && kill "$display_pid" 2>/dev/null || true
  [[ -n "${dbus_pid:-}" ]] && kill "$dbus_pid" 2>/dev/null || true
  rm -rf "$RUNTIME"
}
trap cleanup EXIT
chmod 700 "$RUNTIME"
export XDG_RUNTIME_DIR="$RUNTIME"
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe
export XDG_CURRENT_DESKTOP="$DESKTOP"
export XDG_SESSION_DESKTOP="$DESKTOP"
export XDG_SESSION_TYPE="$SESSION"
export XDG_DATA_HOME="$RUNTIME/data"
export XDG_CONFIG_HOME="$RUNTIME/config"
export XDG_CACHE_HOME="$RUNTIME/cache"
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"

if [[ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]]; then
  mapfile -t bus < <(dbus-daemon --session --fork --print-address=1 --print-pid=1)
  [[ "${#bus[@]}" == 2 ]] || { echo "ERROR: could not start session bus" >&2; exit 1; }
  export DBUS_SESSION_BUS_ADDRESS="${bus[0]}"
  dbus_pid="${bus[1]}"
fi

case "$DESKTOP" in
  GNOME)
    extension="$COMPANION_SOURCE/gnome-shell-extension"
    [[ -f "$extension/metadata.json" ]] || { echo "ERROR: packaged GNOME companion is missing metadata" >&2; exit 1; }
    extension_id="$(python3 - "$extension/metadata.json" <<'PY'
import json
import sys
value = json.load(open(sys.argv[1], encoding='utf-8')).get('uuid')
if not isinstance(value, str) or not value:
    raise SystemExit(1)
print(value)
PY
)"
    install -d "$XDG_DATA_HOME/gnome-shell/extensions"
    cp -a "$extension" "$XDG_DATA_HOME/gnome-shell/extensions/$extension_id"
    native="$XDG_DATA_HOME/gnome-shell/extensions/$extension_id/native"
    [[ -d "$native/lib" && -d "$native/typelib" ]] || { echo "ERROR: packaged GNOME companion native runtime is incomplete" >&2; exit 1; }
    export GI_TYPELIB_PATH="$native/typelib${GI_TYPELIB_PATH:+:$GI_TYPELIB_PATH}"
    export LD_LIBRARY_PATH="$native/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    gsettings set org.gnome.shell enabled-extensions "['$extension_id']"
    ;;
  KDE)
    script="$COMPANION_SOURCE/kde-kwin-script"
    [[ -d "$script" ]] || { echo "ERROR: packaged KDE companion is missing" >&2; exit 1; }
    kpackagetool6 --type KWin/Script --install "$script"
    kwriteconfig6 --file "$XDG_CONFIG_HOME/kwinrc" --group Plugins --key copypaste-quick-pasteEnabled true
    ;;
esac

wait_for() {
  local predicate="$1"
  for _ in $(seq 1 100); do
    eval "$predicate" && return 0
    sleep 0.1
  done
  echo "ERROR: nested $DESKTOP $SESSION desktop did not become ready" >&2
  return 1
}

if [[ "$SESSION" == x11 ]]; then
  export COPYPASTE_COMPOSITOR_EXECUTION=stock-x11
  command -v Xvfb >/dev/null
  Xvfb ":$DISPLAY_NUMBER" -screen 0 1280x800x24 -nolisten tcp >"$RUNTIME/xserver.log" 2>&1 &
  display_pid=$!
  export DISPLAY=":$DISPLAY_NUMBER"
  wait_for 'xdpyinfo -display "$DISPLAY" >/dev/null 2>&1'
  gnome-shell --x11 --replace >"$RUNTIME/desktop.log" 2>&1 &
  desktop_pid=$!
  wait_for 'kill -0 "$desktop_pid" 2>/dev/null'
else
  export COPYPASTE_COMPOSITOR_EXECUTION=private-wayland
  export LD_LIBRARY_PATH="$runtime_root/lib:$runtime_root/lib64:$runtime_root/usr/lib:$runtime_root/usr/lib/x86_64-linux-gnu:$runtime_root/usr/lib/aarch64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  "$COMPOSITOR_QUALIFICATION_ENTRYPOINT" >"$RUNTIME/desktop.log" 2>&1 &
  desktop_pid=$!
  wait_for 'kill -0 "$desktop_pid" 2>/dev/null'
  wait_for 'find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name "wayland-*" | grep -q .'
  export WAYLAND_DISPLAY="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-*' -printf '%f\n' | head -n1)"
  export COPYPASTE_COMPOSITOR_SESSION_RECORD="$RUNTIME/compositor-session-record.json"
  python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/record-linux-compositor-session.py" \
    --binding "$COMPOSITOR_RUNTIME_BINDING" --root / --pid "$desktop_pid" --session wayland \
    --output "$COPYPASTE_COMPOSITOR_SESSION_RECORD"
fi

dbus-update-activation-environment \
  DBUS_SESSION_BUS_ADDRESS DISPLAY WAYLAND_DISPLAY XDG_RUNTIME_DIR XDG_DATA_HOME \
  XDG_CONFIG_HOME XDG_CURRENT_DESKTOP XDG_SESSION_DESKTOP XDG_SESSION_TYPE
keyring_password="$(openssl rand -hex 24)"
printf '%s\n' "$keyring_password" | gnome-keyring-daemon --login >/dev/null
eval "$(gnome-keyring-daemon --start --components=secrets)"
printf '%s' "$keyring_password" | secret-tool store --label='CopyPaste qualification keyring' copypaste-qualification default >/dev/null
secret-tool lookup copypaste-qualification default >/dev/null
unset keyring_password

if [[ "$DESKTOP" == GNOME ]]; then
  appindicator="$(gnome-extensions list 2>/dev/null | awk '/appindicator|ubuntu-appindicators/ {print; exit}')"
  [[ -n "$appindicator" ]] || { echo "ERROR: GNOME AppIndicator extension is unavailable" >&2; exit 1; }
  gnome-extensions enable "$appindicator"
fi

"$@"
