#!/usr/bin/env bash
# Execute the shipped native Linux qualification probe inside one desktop session.
set -euo pipefail

ARTIFACTS=""
PREVIOUS_ARTIFACTS=""
VERSION=""
PREVIOUS_VERSION=""
ARCHITECTURE=""
DESKTOP=""
SESSION=""
EVIDENCE_DIR=""
MODULE_ARTIFACTS=""
MODULE_FIXTURES=""
FIRST_INSTALL_BASELINE=false
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --artifacts) ARTIFACTS="$2"; shift 2 ;;
    --previous-artifacts) PREVIOUS_ARTIFACTS="$2"; shift 2 ;;
    --version) VERSION="$2"; shift 2 ;;
    --previous-version) PREVIOUS_VERSION="$2"; shift 2 ;;
    --first-install-baseline) FIRST_INSTALL_BASELINE=true; shift ;;
    --architecture) ARCHITECTURE="$2"; shift 2 ;;
    --desktop) DESKTOP="$2"; shift 2 ;;
    --session) SESSION="$2"; shift 2 ;;
    --evidence-dir) EVIDENCE_DIR="$2"; shift 2 ;;
    --module-artifacts) MODULE_ARTIFACTS="$2"; shift 2 ;;
    --module-fixtures) MODULE_FIXTURES="$2"; shift 2 ;;
    *) echo "ERROR: unknown qualification argument: $1" >&2; exit 2 ;;
  esac
done

[[ -n "$ARTIFACTS" && -n "$VERSION" && -n "$ARCHITECTURE" && -n "$DESKTOP" && -n "$SESSION" && -n "$EVIDENCE_DIR" && -n "$MODULE_ARTIFACTS" && -n "$MODULE_FIXTURES" ]] || {
  echo "ERROR: incomplete Linux native qualification invocation" >&2
  exit 2
}
if [[ "$FIRST_INSTALL_BASELINE" == true ]]; then
  [[ -z "$PREVIOUS_ARTIFACTS" && -z "$PREVIOUS_VERSION" ]] || { echo "ERROR: baseline cannot name a previous artifact" >&2; exit 2; }
else
  [[ -n "$PREVIOUS_ARTIFACTS" && -n "$PREVIOUS_VERSION" ]] || { echo "ERROR: upgrade qualification needs a prior release" >&2; exit 2; }
fi
case "$ARCHITECTURE" in x86_64|aarch64) ;; *) exit 2 ;; esac
case "$DESKTOP" in GNOME|KDE) ;; *) exit 2 ;; esac
case "$SESSION" in x11|wayland) ;; *) exit 2 ;; esac
[[ "${XDG_SESSION_TYPE:-}" == "$SESSION" ]] || {
  echo "ERROR: expected a real $SESSION session, got ${XDG_SESSION_TYPE:-unset}" >&2
  exit 1
}
[[ "${XDG_CURRENT_DESKTOP:-}" == *"$DESKTOP"* ]] || {
  echo "ERROR: expected a $DESKTOP desktop, got ${XDG_CURRENT_DESKTOP:-unset}" >&2
  exit 1
}
if [[ "$SESSION" == x11 ]]; then
  command -v xprop >/dev/null
  xprop -root >/dev/null
else
  [[ -n "${WAYLAND_DISPLAY:-}" && -S "${XDG_RUNTIME_DIR:-}/$WAYLAND_DISPLAY" ]] || {
    echo "ERROR: Wayland socket is unavailable" >&2
    exit 1
  }
fi
command -v busctl >/dev/null
busctl --user status >/dev/null
ARTIFACTS="$(cd "$ARTIFACTS" && pwd)"
[[ -d "$MODULE_ARTIFACTS" && ! -L "$MODULE_ARTIFACTS" ]] || { echo "ERROR: module artifact staging directory is unavailable" >&2; exit 1; }
[[ -d "$MODULE_FIXTURES" && ! -L "$MODULE_FIXTURES" ]] || { echo "ERROR: module fixture staging directory is unavailable" >&2; exit 1; }
MODULE_ARTIFACTS="$(cd "$MODULE_ARTIFACTS" && pwd)"
MODULE_FIXTURES="$(cd "$MODULE_FIXTURES" && pwd)"
if [[ -n "$PREVIOUS_ARTIFACTS" ]]; then
  PREVIOUS_ARTIFACTS="$(cd "$PREVIOUS_ARTIFACTS" && pwd)"
fi

run_traced() {
  local assertion="$1"
  shift
  local returncode=0
  "$@" || returncode=$?
  python3 - "$assertion" "$returncode" "$@" <<'PY'
import json
import sys
print("COPYPASTE_QUALIFICATION_COMMAND " + json.dumps({
    "argv": sys.argv[3:], "returncode": int(sys.argv[2]), "assertions": [sys.argv[1]],
}, separators=(",", ":")))
PY
  return "$returncode"
}

current_package() { printf '%s/CopyPaste-v%s-linux-%s.%s' "$ARTIFACTS" "$VERSION" "$ARCHITECTURE" "$1"; }
previous_package() { printf '%s/CopyPaste-v%s-linux-%s.%s' "$PREVIOUS_ARTIFACTS" "$PREVIOUS_VERSION" "$ARCHITECTURE" "$1"; }
for format in AppImage deb rpm; do [[ -f "$(current_package "$format")" ]] || { echo "ERROR: exact current $format package is required" >&2; exit 1; }; done
if [[ "$FIRST_INSTALL_BASELINE" == false ]]; then
  for format in AppImage deb rpm; do [[ -f "$(previous_package "$format")" ]] || { echo "ERROR: exact prior $format package is required" >&2; exit 1; }; done
fi

cleanup_packages() {
  sudo dpkg --purge copypaste >/dev/null 2>&1 || true
  sudo rpm --erase copypaste >/dev/null 2>&1 || true
}
trap cleanup_packages EXIT

# AppImage is exercised as a portable launch boundary. Its updater is not
# qualified here: extraction does not replace a user-owned executable or prove
# preserved history. The native system format is installed by its own package
# manager with dependency resolution; there is no --nodeps escape hatch.
work="$(mktemp -d)"
trap 'rm -rf "$work"; cleanup_packages' EXIT
if [[ -f /etc/fedora-release ]]; then
  native_format=rpm
else
  native_format=deb
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
args=(--artifacts "$ARTIFACTS" --version "$VERSION" --architecture "$ARCHITECTURE" --desktop "$DESKTOP" --session "$SESSION" --evidence-dir "$EVIDENCE_DIR" --module-artifacts "$MODULE_ARTIFACTS" --module-fixtures "$MODULE_FIXTURES")
if [[ "$FIRST_INSTALL_BASELINE" == true ]]; then
  args+=(--first-install-baseline)
else
  args+=(--previous-artifacts "$PREVIOUS_ARTIFACTS" --previous-version "$PREVIOUS_VERSION")
fi
record_seed_source() {
  local executable="$1"
  local profile="$2"
  python3 - "$executable" "$profile/prior-source.json" <<'PY'
import hashlib
import json
import os
import sys
from pathlib import Path

executable, output = sys.argv[1:]
digest = hashlib.sha256()
with open(executable, "rb") as source:
    for block in iter(lambda: source.read(1024 * 1024), b""):
        digest.update(block)
Path(output).write_text(json.dumps({
    "path": os.path.realpath(executable),
    "sha256": digest.hexdigest(),
    "size_bytes": os.path.getsize(executable),
}, separators=(",", ":")), encoding="utf-8")
PY
}

if [[ "$FIRST_INSTALL_BASELINE" == false ]]; then
  mkdir -p "$work/appimage-prior"
  prior_appimage_extract_argv=(env APPIMAGE_EXTRACT_AND_RUN=1 "$(previous_package AppImage)" --appimage-extract)
  (
    cd "$work/appimage-prior"
    run_traced package_upgrade "${prior_appimage_extract_argv[@]}"
  )
  prior_appimage_prefix="$work/appimage-prior/squashfs-root/usr/lib/copypaste"
  for executable in copypaste copypaste-daemon copypaste-cli; do
    test -x "$prior_appimage_prefix/$executable"
  done
  appimage_profile="$work/appimage-upgrade"
  install -d -m 700 "$appimage_profile"
  python3 "$ROOT/scripts/release/linux-native-fixture-driver.py" "${args[@]}" \
    --runtime-format AppImage --runtime-prefix "$prior_appimage_prefix" \
    --qualification-root "$appimage_profile" --seed-upgrade-canary
  record_seed_source "$prior_appimage_prefix/copypaste" "$appimage_profile"
fi

mkdir -p "$work/appimage-current"
appimage_extract_argv=(env APPIMAGE_EXTRACT_AND_RUN=1 "$(current_package AppImage)" --appimage-extract)
(
  cd "$work/appimage-current"
  run_traced package_install "${appimage_extract_argv[@]}"
)
run_traced desktop_uri_icon test -x "$work/appimage-current/squashfs-root/AppRun"
run_traced desktop_uri_icon test -f "$work/appimage-current/squashfs-root/com.copypaste.CopyPaste.desktop"
run_traced desktop_uri_icon grep -Fx 'MimeType=x-scheme-handler/copypaste;' "$work/appimage-current/squashfs-root/com.copypaste.CopyPaste.desktop"
run_traced desktop_uri_icon test -f "$work/appimage-current/squashfs-root/com.copypaste.CopyPaste.png"
appimage_prefix="$work/appimage-current/squashfs-root/usr/lib/copypaste"
for executable in copypaste copypaste-daemon copypaste-cli; do
  test -x "$appimage_prefix/$executable"
done

if [[ "$FIRST_INSTALL_BASELINE" == false ]]; then
  if [[ "$native_format" == rpm ]]; then
    prior_native_install_argv=(sudo dnf --assumeyes install "$(previous_package rpm)")
  else
    prior_native_install_argv=(sudo apt-get install --yes "$(previous_package deb)")
  fi
  run_traced package_upgrade "${prior_native_install_argv[@]}"
  if [[ "$native_format" == rpm ]]; then
    run_traced package_upgrade rpm -q copypaste
  else
    run_traced package_upgrade dpkg-query --show copypaste
  fi
  native_profile="$work/native-upgrade"
  install -d -m 700 "$native_profile"
  python3 "$ROOT/scripts/release/linux-native-fixture-driver.py" "${args[@]}" \
    --runtime-format "$native_format" --runtime-prefix /usr/lib/copypaste \
    --qualification-root "$native_profile" --seed-upgrade-canary
  record_seed_source /usr/lib/copypaste/copypaste "$native_profile"
fi

if [[ "$native_format" == rpm ]]; then
  native_install_argv=(sudo dnf --assumeyes install "$(current_package rpm)")
  run_traced package_install "${native_install_argv[@]}"
  run_traced package_install rpm -q copypaste
else
  native_format=deb
  native_install_argv=(sudo apt-get install --yes "$(current_package deb)")
  run_traced package_install "${native_install_argv[@]}"
  run_traced package_install dpkg-query --show copypaste
fi
python3 - "$(current_package AppImage)" "$(current_package "$native_format")" "$native_format" "${appimage_extract_argv[@]}" -- "${native_install_argv[@]}" <<'PY'
import hashlib
import json
import sys

appimage, native, native_format, *argv = sys.argv[1:]
divider = argv.index("--")
appimage_argv = argv[:divider]
native_argv = argv[divider + 1:]
def package(path, format_name):
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return {"format": format_name, "name": path.rsplit("/", 1)[-1], "path": __import__("os").path.realpath(path), "sha256": digest.hexdigest(), "size_bytes": __import__("os").path.getsize(path)}
print("COPYPASTE_QUALIFICATION_INSTALL " + json.dumps({
    "formats": ["AppImage", native_format],
    "packages": [package(appimage, "AppImage"), package(native, native_format)],
    "appimage_extract_argv": appimage_argv,
    "native_install_argv": native_argv,
}, separators=(",", ":")))
PY

native_prefix="/usr/lib/copypaste"
for executable in copypaste copypaste-daemon copypaste-cli; do
  test -x "$native_prefix/$executable"
done

appimage_driver_args=(--runtime-format AppImage --runtime-prefix "$appimage_prefix")
native_driver_args=(--runtime-format "$native_format" --runtime-prefix "$native_prefix")
if [[ "$FIRST_INSTALL_BASELINE" == false ]]; then
  appimage_driver_args+=(--qualification-root "$appimage_profile")
  native_driver_args+=(--qualification-root "$native_profile")
fi
python3 "$ROOT/scripts/release/linux-native-fixture-driver.py" "${args[@]}" "${appimage_driver_args[@]}"
python3 "$ROOT/scripts/release/linux-native-fixture-driver.py" "${args[@]}" "${native_driver_args[@]}"

if [[ "$FIRST_INSTALL_BASELINE" == false ]]; then
  python3 - "$VERSION" "$PREVIOUS_VERSION" "$native_format" \
    "$(current_package AppImage)" "$(current_package "$native_format")" \
    "$(previous_package AppImage)" "$(previous_package "$native_format")" \
    "$appimage_profile" "$native_profile" <<'PY'
import hashlib
import json
import os
import sys

(version, previous_version, native_format, current_appimage, current_native,
 previous_appimage, previous_native, appimage_profile, native_profile) = sys.argv[1:]

def package(path, format_name):
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return {"format": format_name, "name": os.path.basename(path), "path": os.path.realpath(path), "sha256": digest.hexdigest(), "size_bytes": os.path.getsize(path)}

def seed(format_name, profile):
    source_path = os.path.join(profile, "prior-source.json")
    with open(source_path, encoding="utf-8") as source_file:
        source = json.load(source_file)
    if (not isinstance(source, dict) or set(source) != {"path", "sha256", "size_bytes"}
            or not isinstance(source["path"], str) or not os.path.isabs(source["path"])
            or not isinstance(source["sha256"], str) or len(source["sha256"]) != 64
            or not isinstance(source["size_bytes"], int) or source["size_bytes"] <= 0):
        raise SystemExit("ERROR: prior source executable record is invalid")
    receipt_path = os.path.join(profile, "upgrade-canary.json")
    with open(receipt_path, encoding="utf-8") as receipt_file:
        receipt = json.load(receipt_file)
    if (set(receipt) != {"id", "content_sha256", "executable_sha256"}
            or receipt["executable_sha256"] != source["sha256"]):
        raise SystemExit("ERROR: prior canary receipt does not bind its source executable")
    return {"format": format_name, "source_executable": source, "canary": receipt}

def transition(release_version, appimage, native):
    appimage_record = package(appimage, "AppImage")
    native_record = package(native, native_format)
    return {
        "version": release_version,
        "packages": [appimage_record, native_record],
        "appimage_extract_argv": ["env", "APPIMAGE_EXTRACT_AND_RUN=1", appimage_record["path"], "--appimage-extract"],
        "native_install_argv": (["sudo", "apt-get", "install", "--yes", native_record["path"]]
                               if native_format == "deb" else ["sudo", "dnf", "--assumeyes", "install", native_record["path"]]),
    }

print("COPYPASTE_QUALIFICATION_UPGRADE " + json.dumps({
    "prior": transition(previous_version, previous_appimage, previous_native),
    "current": transition(version, current_appimage, current_native),
    "seeds": [
        seed("AppImage", appimage_profile),
        seed(native_format, native_profile),
    ],
}, separators=(",", ":")))
PY
fi
