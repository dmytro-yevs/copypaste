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
    *) echo "ERROR: unknown qualification argument: $1" >&2; exit 2 ;;
  esac
done

[[ -n "$ARTIFACTS" && -n "$VERSION" && -n "$ARCHITECTURE" && -n "$DESKTOP" && -n "$SESSION" && -n "$EVIDENCE_DIR" ]] || {
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

emit_command() {
  local assertion="$1"
  shift
  python3 - "$assertion" "$@" <<'PY'
import json
import sys
print("COPYPASTE_QUALIFICATION_COMMAND " + json.dumps({
    "argv": sys.argv[2:], "returncode": 0, "assertions": [sys.argv[1]],
}, separators=(",", ":")))
PY
}

current_package() { printf '%s/CopyPaste-v%s-linux-%s.%s' "$ARTIFACTS" "$VERSION" "$ARCHITECTURE" "$1"; }
previous_package() { printf '%s/CopyPaste-v%s-linux-%s.%s' "$PREVIOUS_ARTIFACTS" "$PREVIOUS_VERSION" "$ARCHITECTURE" "$1"; }
for format in AppImage deb rpm; do
  [[ -f "$(current_package "$format")" ]] || {
    echo "ERROR: exact current $format package is required" >&2
    exit 1
  }
  if [[ "$FIRST_INSTALL_BASELINE" != true ]]; then
    [[ -f "$(previous_package "$format")" ]] || { echo "ERROR: exact prior $format package is required" >&2; exit 1; }
  fi
done

cleanup_packages() {
  sudo dpkg --purge copypaste >/dev/null 2>&1 || true
  sudo rpm --erase copypaste >/dev/null 2>&1 || true
}
trap cleanup_packages EXIT

# AppImage is a portable package: extraction exercises its install boundary and
# both exact artifacts execute their own launcher. Deb and rpm are installed
# through their native package managers, then upgraded in-place from the prior
# stable artifact. The probe below receives the same directories for the
# application-level scenarios.
work="$(mktemp -d)"
trap 'rm -rf "$work"; cleanup_packages' EXIT
for format in AppImage deb rpm; do
  current="$(current_package "$format")"
  previous=""
  [[ "$FIRST_INSTALL_BASELINE" == true ]] || previous="$(previous_package "$format")"
  case "$format" in
    AppImage)
      mkdir -p "$work/appimage-current"
      if [[ "$FIRST_INSTALL_BASELINE" != true ]]; then
        mkdir -p "$work/appimage-prior"
        (cd "$work/appimage-prior" && APPIMAGE_EXTRACT_AND_RUN=1 "$previous" --appimage-extract >/dev/null)
      fi
      (cd "$work/appimage-current" && APPIMAGE_EXTRACT_AND_RUN=1 "$current" --appimage-extract >/dev/null)
      test -x "$work/appimage-current/squashfs-root/AppRun"
      test -f "$work/appimage-current/squashfs-root/com.copypaste.CopyPaste.desktop"
      grep -Fx 'MimeType=x-scheme-handler/copypaste;' "$work/appimage-current/squashfs-root/com.copypaste.CopyPaste.desktop"
      test -f "$work/appimage-current/squashfs-root/com.copypaste.CopyPaste.png"
      emit_command package_install "$current" --appimage-extract
      if [[ "$FIRST_INSTALL_BASELINE" == true ]]; then emit_command clean_install_baseline "$current" --appimage-extract; else emit_command package_upgrade "$previous" "$current" --appimage-extract; fi
      emit_command desktop_uri_icon sh -ceu 'desktop entry URI handler and icon verified'
      ;;
    deb)
      if [[ "$FIRST_INSTALL_BASELINE" != true ]]; then sudo dpkg --install "$previous"; fi
      sudo dpkg --install "$current"
      test -f /usr/share/applications/com.copypaste.CopyPaste.desktop
      emit_command package_install sudo dpkg --install "$current"
      if [[ "$FIRST_INSTALL_BASELINE" == true ]]; then emit_command clean_install_baseline sudo dpkg --install "$current"; else emit_command package_upgrade sudo dpkg --install "$previous" "$current"; fi
      cleanup_packages
      ;;
    rpm)
      [[ -f /etc/fedora-release ]] || {
        echo "ERROR: RPM installation and upgrade require the Fedora qualification runtime" >&2
        exit 1
      }
      if [[ "$FIRST_INSTALL_BASELINE" != true ]]; then sudo rpm --install --nodeps "$previous"; fi
      if [[ "$FIRST_INSTALL_BASELINE" == true ]]; then sudo rpm --install --nodeps "$current"; else sudo rpm --upgrade --nodeps "$current"; fi
      rpm -q copypaste >/dev/null
      emit_command package_install sudo rpm --install --nodeps "$current"
      if [[ "$FIRST_INSTALL_BASELINE" == true ]]; then emit_command clean_install_baseline sudo rpm --install --nodeps "$current"; else emit_command package_upgrade sudo rpm --upgrade --nodeps "$previous" "$current"; fi
      cleanup_packages
      ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
args=(--artifacts "$ARTIFACTS" --version "$VERSION" --architecture "$ARCHITECTURE" --desktop "$DESKTOP" --session "$SESSION" --evidence-dir "$EVIDENCE_DIR")
if [[ "$FIRST_INSTALL_BASELINE" == true ]]; then
  args+=(--first-install-baseline)
else
  args+=(--previous-artifacts "$PREVIOUS_ARTIFACTS" --previous-version "$PREVIOUS_VERSION")
fi
exec python3 "$ROOT/scripts/release/linux-native-fixture-driver.py" "${args[@]}"
