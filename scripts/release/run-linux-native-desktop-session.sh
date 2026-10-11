#!/usr/bin/env bash
# Start a disposable software-rendered GNOME or KDE desktop and run one command.
set -euo pipefail

DESKTOP=""
SESSION=""
COMPANION_SOURCE=""
COMPOSITOR_RUNTIME=""
COMPOSITOR_RUNTIME_BINDING=""
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
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
runtime_package_path="$(realpath "$COMPOSITOR_RUNTIME/$runtime_package")"
sudo apt-get install --yes "$runtime_package_path"
dpkg-query --show "copypaste-compositor-runtime-$runtime_id" >/dev/null
python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/verify-linux-compositor-runtime.py" installed --binding "$COMPOSITOR_RUNTIME_BINDING" --root /
COMPOSITOR_LAUNCHER="/usr/lib/copypaste/compositor-runtime/bin/copypaste-compositor-session-$runtime_id"
runtime_root="/usr/lib/copypaste/compositor-runtime/$runtime_id"
qualification_entrypoint="$(jq -er '.qualification.entrypoint | select(type == "string" and test("^[A-Za-z0-9._/+@-]+$"))' "/usr/share/copypaste/compositor-runtime/$runtime_id.receipt.json")"
COMPOSITOR_QUALIFICATION_ENTRYPOINT="$runtime_root/$qualification_entrypoint"
[[ -x "$COMPOSITOR_QUALIFICATION_ENTRYPOINT" ]] || { echo "ERROR: receipt-listed GNOME qualification entrypoint is missing" >&2; exit 1; }
private_shell="$runtime_root/usr/bin/gnome-shell"
[[ -x "$private_shell" ]] || { echo "ERROR: receipt-listed GNOME Shell is missing" >&2; exit 1; }
private_library_path="$runtime_root/lib:$runtime_root/lib64:$runtime_root/usr/lib:$runtime_root/usr/lib/x86_64-linux-gnu:$runtime_root/usr/lib/x86_64-linux-gnu/mutter-14:$runtime_root/usr/lib/x86_64-linux-gnu/gnome-shell:$runtime_root/usr/lib/aarch64-linux-gnu:$runtime_root/usr/lib/aarch64-linux-gnu/mutter-14:$runtime_root/usr/lib/aarch64-linux-gnu/gnome-shell"
private_typelib_path="$runtime_root/usr/lib/x86_64-linux-gnu/mutter-14:$runtime_root/usr/lib/x86_64-linux-gnu/gnome-shell:$runtime_root/usr/lib/aarch64-linux-gnu/mutter-14:$runtime_root/usr/lib/aarch64-linux-gnu/gnome-shell"
if ! loader_report="$(LD_LIBRARY_PATH="$private_library_path" ldd "$private_shell" 2>&1)"; then
  printf 'ERROR: could not inspect private GNOME Shell loader closure\n%s\n' "$loader_report" >&2
  exit 1
fi
unresolved_private_libraries="$(printf '%s\n' "$loader_report" | awk '/=> not found/ {print $1}')"
[[ -z "$unresolved_private_libraries" ]] || {
  printf 'ERROR: private GNOME Shell loader closure is unresolved: %s\n' "$(printf '%s\n' "$unresolved_private_libraries" | paste -sd, -)" >&2
  exit 1
}

RUNTIME="$(mktemp -d)"
DISPLAY_NUMBER=99
session_phase=prepare_session
cleanup() {
  local status=$?
  if [[ "$status" != 0 ]]; then
    printf 'ERROR: GNOME session phase %s exited %s\n' "$session_phase" "$status" >&2
    for startup_log in "$RUNTIME/desktop.log" "$RUNTIME/xserver.log"; do
      [[ -f "$startup_log" && ! -L "$startup_log" ]] || continue
      printf 'Startup log (last 1 MiB): %s\n' "$startup_log" >&2
      tail -c 1048576 -- "$startup_log" >&2
    done
  fi
  [[ -n "${geometry_broker_pid:-}" ]] && kill "$geometry_broker_pid" 2>/dev/null || true
  [[ -n "${shell_pid:-}" ]] && kill "$shell_pid" 2>/dev/null || true
  [[ -n "${desktop_pid:-}" ]] && kill "$desktop_pid" 2>/dev/null || true
  [[ -n "${display_pid:-}" ]] && kill "$display_pid" 2>/dev/null || true
  if mountpoint -q "$RUNTIME/doc"; then
    document_mount="$(awk -v target="$RUNTIME/doc" '
      $5 == target {
        split($0, fields, " - ")
        split(fields[2], filesystem, " ")
        print $5 " " filesystem[1]
        exit
      }
    ' /proc/self/mountinfo)"
    [[ "$document_mount" == "$RUNTIME/doc fuse.portal" ]] || {
      printf 'ERROR: unexpected fixture runtime document mount: %s\n' "$document_mount" >&2
      return 1
    }
    fusermount3 -u "$RUNTIME/doc"
    ! mountpoint -q "$RUNTIME/doc"
  fi
  [[ -n "${dbus_pid:-}" ]] && kill "$dbus_pid" 2>/dev/null || true
  rm -rf "$RUNTIME"
}
trap cleanup EXIT
chmod 700 "$RUNTIME"
export XDG_RUNTIME_DIR="$RUNTIME"
export LANG=C.UTF-8 LC_ALL=C.UTF-8
export NO_AT_BRIDGE=0
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
    native_typelib_path="$native/typelib${GI_TYPELIB_PATH:+:$GI_TYPELIB_PATH}"
    compositor_library_path="$native/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    if [[ "$SESSION" == wayland ]]; then
      geometry_extension="$SCRIPT_DIR/qualification/gnome-window-geometry-extension"
      geometry_extension_id="copypaste-qualification-window-geometry@copypaste.app"
      [[ -f "$geometry_extension/metadata.json" && -f "$geometry_extension/extension.js" ]] || { echo "ERROR: qualification GNOME geometry fixture is missing" >&2; exit 1; }
      cp -a "$geometry_extension" "$XDG_DATA_HOME/gnome-shell/extensions/$geometry_extension_id"
      gsettings set org.gnome.shell enabled-extensions "['$extension_id', '$geometry_extension_id']"
    else
      gsettings set org.gnome.shell enabled-extensions "['$extension_id']"
    fi
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
  session_phase="readiness: $predicate"
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
  GI_TYPELIB_PATH="$native_typelib_path" LD_LIBRARY_PATH="$compositor_library_path" \
    gnome-shell --x11 --replace >"$RUNTIME/desktop.log" 2>&1 &
  desktop_pid=$!
  wait_for 'kill -0 "$desktop_pid" 2>/dev/null'
else
  export COPYPASTE_COMPOSITOR_EXECUTION=private-wayland
  GI_TYPELIB_PATH="$private_typelib_path:$native_typelib_path" \
    LD_LIBRARY_PATH="$private_library_path" "$COMPOSITOR_QUALIFICATION_ENTRYPOINT" >"$RUNTIME/desktop.log" 2>&1 &
  desktop_pid=$!
  wait_for 'kill -0 "$desktop_pid" 2>/dev/null'
  export COPYPASTE_QUALIFICATION_GEOMETRY_OWNER_PID="$desktop_pid"
  wait_for 'find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name "wayland-*" | grep -q .'
  export WAYLAND_DISPLAY="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-*' -printf '%f\n' | head -n1)"
  export COPYPASTE_COMPOSITOR_SESSION_RECORD="$RUNTIME/compositor-session-record.json"
  python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/record-linux-compositor-session.py" \
    --binding "$COMPOSITOR_RUNTIME_BINDING" --root / --pid "$desktop_pid" --session wayland \
    --output "$COPYPASTE_COMPOSITOR_SESSION_RECORD"
  wait_for 'gdbus call --session --dest app.copypaste.QualificationGeometry --object-path /app/copypaste/QualificationGeometry --method app.copypaste.QualificationGeometry.Version >/dev/null 2>&1'
fi
wait_for 'gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Extensions.ListExtensions >/dev/null 2>&1'

session_phase=publish_session_environment
dbus-update-activation-environment \
  DBUS_SESSION_BUS_ADDRESS DISPLAY WAYLAND_DISPLAY XDG_RUNTIME_DIR XDG_DATA_HOME \
  XDG_CONFIG_HOME XDG_CURRENT_DESKTOP XDG_SESSION_DESKTOP XDG_SESSION_TYPE
session_phase=initialize_accessibility
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus --method org.a11y.Bus.GetAddress >/dev/null
busctl --user set-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled b true
[[ "$(busctl --user get-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled)" == "b true" ]]
keyring_password="$(openssl rand -hex 24)"
session_phase=keyring_login
printf '%s\n' "$keyring_password" | gnome-keyring-daemon --login >/dev/null
session_phase=keyring_start
eval "$(gnome-keyring-daemon --start --components=secrets)"
session_phase=keyring_unlock
printf '%s' "$keyring_password" | secret-tool store --label='CopyPaste qualification keyring' copypaste-qualification default >/dev/null
secret-tool lookup copypaste-qualification default >/dev/null
unset keyring_password

if [[ "$DESKTOP" == GNOME ]]; then
  session_phase=enable_appindicator
  if [[ "${COPYPASTE_GNOME_EXTENSION_DIAGNOSTIC:-}" == 1 ]]; then
    diagnostic_output="${COPYPASTE_GNOME_EXTENSION_DIAGNOSTIC_OUTPUT:-}"
    [[ "$diagnostic_output" == /* ]] || { echo "ERROR: GNOME diagnostic output must be an absolute path" >&2; exit 2; }
    diagnostic_dir="$RUNTIME/extension-diagnostic"
    mkdir -p "$diagnostic_dir"

    capture_diagnostic_command() {
      local name="$1"
      shift
      set +e
      "$@" >"$diagnostic_dir/$name.stdout" 2>"$diagnostic_dir/$name.stderr"
      local status=$?
      set -e
      printf '%s\n' "$status" >"$diagnostic_dir/$name.status"
    }

    capture_diagnostic_command cli_service_list \
      gdbus call --session --dest org.gnome.Shell.Extensions --object-path /org/gnome/Shell/Extensions \
      --method org.gnome.Shell.Extensions.ListExtensions
    capture_diagnostic_command list gnome-extensions list
    appindicator="$(awk '/appindicator|ubuntu-appindicators/ {print; exit}' "$diagnostic_dir/list.stdout")"
    capture_diagnostic_command list_details gnome-extensions list --details
    capture_diagnostic_command enabled_before gsettings get org.gnome.shell enabled-extensions
    capture_diagnostic_command disabled_before gsettings get org.gnome.shell disabled-extensions
    if [[ -n "$appindicator" ]]; then
      capture_diagnostic_command info_before gnome-extensions info "$appindicator"
      capture_diagnostic_command extension_info_before \
        gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
        --method org.gnome.Shell.Extensions.GetExtensionInfo "$appindicator"
      capture_diagnostic_command extension_errors_before \
        gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
        --method org.gnome.Shell.Extensions.GetExtensionErrors "$appindicator"
      capture_diagnostic_command enable gnome-extensions enable "$appindicator"
      capture_diagnostic_command enabled_after gsettings get org.gnome.shell enabled-extensions
      capture_diagnostic_command disabled_after gsettings get org.gnome.shell disabled-extensions
      capture_diagnostic_command info_after gnome-extensions info "$appindicator"
      capture_diagnostic_command extension_info_after \
        gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
        --method org.gnome.Shell.Extensions.GetExtensionInfo "$appindicator"
      capture_diagnostic_command extension_errors_after \
        gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
        --method org.gnome.Shell.Extensions.GetExtensionErrors "$appindicator"
      capture_diagnostic_command extensions_after \
        gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
        --method org.gnome.Shell.Extensions.ListExtensions
    fi
    APPINDICATOR="$appindicator" DIAGNOSTIC_DIR="$diagnostic_dir" DIAGNOSTIC_OUTPUT="$diagnostic_output" \
      python3 - <<'PY'
import json
import os
from pathlib import Path

directory = Path(os.environ["DIAGNOSTIC_DIR"])
output = Path(os.environ["DIAGNOSTIC_OUTPUT"])

def read(name: str) -> str:
    path = directory / name
    if not path.is_file():
        return ""
    return path.read_text(encoding="utf-8", errors="replace")[:65536]

commands = {}
for status_path in sorted(directory.glob("*.status")):
    name = status_path.stem
    commands[name] = {
        "status": int(read(status_path.name).strip()),
        "stdout": read(f"{name}.stdout"),
        "stderr": read(f"{name}.stderr"),
    }

environment = {
    name: os.environ.get(name, "")
    for name in (
        "DBUS_SESSION_BUS_ADDRESS", "DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR",
        "XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP",
        "XDG_SESSION_TYPE",
    )
}
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps({
    "schema": 1,
    "appindicator_uuid": os.environ["APPINDICATOR"],
    "activation_order": [
        "XDG variables exported before dbus-daemon",
        "initial gsettings write before dbus-update-activation-environment",
        "dbus-update-activation-environment before extension probe",
    ],
    "environment": environment,
    "commands": commands,
}, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
    [[ -n "$appindicator" ]] || { echo "ERROR: GNOME AppIndicator extension is unavailable" >&2; exit 1; }
    enable_status="$(<"$diagnostic_dir/enable.status")"
    [[ "$enable_status" == 0 ]] || exit "$enable_status"
  else
    appindicator="$(gnome-extensions list 2>/dev/null | awk '/appindicator|ubuntu-appindicators/ {print; exit}')"
    [[ -n "$appindicator" ]] || { echo "ERROR: GNOME AppIndicator extension is unavailable" >&2; exit 1; }
    gnome-extensions enable "$appindicator"
  fi
fi

session_phase=run_product_scenarios
"$@"
