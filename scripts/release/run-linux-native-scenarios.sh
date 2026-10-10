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
mkdir -p "$work/appimage-current"
(
  cd "$work/appimage-current"
  run_traced package_install env APPIMAGE_EXTRACT_AND_RUN=1 "$(current_package AppImage)" --appimage-extract
)
run_traced desktop_uri_icon test -x "$work/appimage-current/squashfs-root/AppRun"
run_traced desktop_uri_icon test -f "$work/appimage-current/squashfs-root/com.copypaste.CopyPaste.desktop"
run_traced desktop_uri_icon grep -Fx 'MimeType=x-scheme-handler/copypaste;' "$work/appimage-current/squashfs-root/com.copypaste.CopyPaste.desktop"
run_traced desktop_uri_icon test -f "$work/appimage-current/squashfs-root/com.copypaste.CopyPaste.png"

if [[ -f /etc/fedora-release ]]; then
  native_format=rpm
  run_traced package_install sudo dnf --assumeyes install "$(current_package rpm)"
  run_traced package_install rpm -q copypaste
else
  native_format=deb
  run_traced package_install sudo apt-get install --yes "$(current_package deb)"
  run_traced package_install dpkg-query --show copypaste
fi
export COPYPASTE_INSTALLED_FORMATS="AppImage,$native_format"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
args=(--artifacts "$ARTIFACTS" --version "$VERSION" --architecture "$ARCHITECTURE" --desktop "$DESKTOP" --session "$SESSION" --evidence-dir "$EVIDENCE_DIR" --module-artifacts "$MODULE_ARTIFACTS" --module-fixtures "$MODULE_FIXTURES")
if [[ "$FIRST_INSTALL_BASELINE" == true ]]; then
  args+=(--first-install-baseline)
else
  args+=(--previous-artifacts "$PREVIOUS_ARTIFACTS" --previous-version "$PREVIOUS_VERSION")
fi
exec python3 "$ROOT/scripts/release/linux-native-fixture-driver.py" "${args[@]}"
