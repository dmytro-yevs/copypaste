#!/usr/bin/env bash
# Verify an exact Linux release package inside a real desktop session.
set -euo pipefail

ARTIFACT="${1:-}"
VERSION="${2:-}"
ARCHITECTURE="${3:-}"
DESKTOP="${4:-}"
SESSION="${5:-}"
[[ -f "$ARTIFACT" ]] || { echo "ERROR: artifact is missing" >&2; exit 1; }
ARTIFACT="$(cd "$(dirname "$ARTIFACT")" && pwd)/$(basename "$ARTIFACT")"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "ERROR: invalid version" >&2; exit 1; }
case "$ARCHITECTURE" in x86_64|aarch64) ;; *) echo "ERROR: invalid architecture" >&2; exit 1 ;; esac
case "$DESKTOP" in GNOME|KDE) ;; *) echo "ERROR: desktop must be GNOME or KDE" >&2; exit 1 ;; esac
case "$SESSION" in x11|wayland) ;; *) echo "ERROR: session must be x11 or wayland" >&2; exit 1 ;; esac

case "$(uname -m)" in
  x86_64) [[ "$ARCHITECTURE" == x86_64 ]] ;;
  aarch64|arm64) [[ "$ARCHITECTURE" == aarch64 ]] ;;
  *) echo "ERROR: unsupported native architecture" >&2; exit 1 ;;
esac
[[ "${XDG_SESSION_TYPE:-}" == "$SESSION" ]] || {
  echo "ERROR: expected a genuine $SESSION session, found ${XDG_SESSION_TYPE:-unset}" >&2
  exit 1
}
[[ "${XDG_CURRENT_DESKTOP:-}" == *"$DESKTOP"* ]] || {
  echo "ERROR: expected a $DESKTOP desktop, found ${XDG_CURRENT_DESKTOP:-unset}" >&2
  exit 1
}
if [[ "$SESSION" == x11 ]]; then
  [[ -n "${DISPLAY:-}" && -z "${XVFB_RUN:-}" ]] || { echo "ERROR: X11 qualification forbids Xvfb" >&2; exit 1; }
else
  [[ -n "${WAYLAND_DISPLAY:-}" ]] || { echo "ERROR: Wayland socket is unavailable" >&2; exit 1; }
fi
command -v dbus-run-session >/dev/null
command -v gnome-keyring-daemon >/dev/null

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
case "$ARTIFACT" in
  *.deb)
    dpkg-deb --info "$ARTIFACT" | grep -F "Version: $VERSION"
    dpkg-deb -x "$ARTIFACT" "$WORK/root"
    ;;
  *.rpm)
    rpm -qpi "$ARTIFACT" | grep -E "^Version[[:space:]]*:[[:space:]]*$VERSION$"
    mkdir -p "$WORK/root"
    (cd "$WORK/root" && rpm2cpio "$ARTIFACT" | cpio -idm --quiet)
    ;;
  *.AppImage)
    (cd "$WORK" && APPIMAGE_EXTRACT_AND_RUN=1 "$ARTIFACT" --appimage-extract >/dev/null)
    mv "$WORK/squashfs-root" "$WORK/root"
    ;;
  *) echo "ERROR: unsupported Linux artifact format" >&2; exit 1 ;;
esac

PREFIX="$WORK/root/usr/lib/copypaste"
[[ -x "$PREFIX/copypaste" && -x "$PREFIX/copypaste-daemon" && -x "$PREFIX/copypaste-cli" ]] || {
  echo "ERROR: installed package is missing application, daemon, or CLI" >&2
  exit 1
}
DESKTOP_FILE="$WORK/root/usr/share/applications/com.copypaste.CopyPaste.desktop"
[[ -f "$DESKTOP_FILE" ]] || { echo "ERROR: package has no desktop entry" >&2; exit 1; }
grep -Fx 'MimeType=x-scheme-handler/copypaste;' "$DESKTOP_FILE"
grep -Fx 'X-GNOME-Autostart-enabled=true' "$DESKTOP_FILE"
[[ -f "$WORK/root/usr/share/icons/hicolor/256x256/apps/com.copypaste.CopyPaste.png" ]] || {
  echo "ERROR: package has no application icon" >&2
  exit 1
}
[[ -f "$WORK/root/usr/share/copypaste/autostart/com.copypaste.CopyPaste.desktop" ]] || {
  echo "ERROR: package has no opt-in autostart template" >&2
  exit 1
}
GNOME_EXTENSION="$WORK/root/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app"
KDE_SCRIPT="$WORK/root/usr/share/kwin/scripts/copypaste-quick-paste"
[[ -f "$GNOME_EXTENSION/metadata.json" && -d "$KDE_SCRIPT/contents" ]] || {
  echo "ERROR: package is missing a desktop integration companion" >&2
  exit 1
}
case "$DESKTOP" in
  GNOME)
    command -v gnome-extensions >/dev/null
    export XDG_DATA_DIRS="$WORK/root/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
    gnome-extensions info copypaste-quick-paste@copypaste.app >/dev/null
    gnome-extensions enable copypaste-quick-paste@copypaste.app
    gsettings get org.gnome.shell enabled-extensions | grep -F "copypaste-quick-paste@copypaste.app"
    ;;
  KDE)
    command -v kpackagetool6 >/dev/null
    command -v kwriteconfig6 >/dev/null
    command -v qdbus6 >/dev/null
    export XDG_DATA_HOME="$WORK/kde-data" XDG_CONFIG_HOME="$WORK/kde-config"
    kpackagetool6 --type KWin/Script --install "$KDE_SCRIPT"
    kwriteconfig6 --file "$XDG_CONFIG_HOME/kwinrc" --group Plugins --key copypaste-quick-pasteEnabled true
    qdbus6 org.kde.KWin /KWin reconfigure
    grep -Fx 'copypaste-quick-pasteEnabled=true' "$XDG_CONFIG_HOME/kwinrc"
    ;;
esac
"$PREFIX/copypaste-daemon" --version | grep -F "$VERSION"
"$PREFIX/copypaste-cli" --help >/dev/null

dbus-run-session -- bash -ceu '
  eval "$(gnome-keyring-daemon --start --components=secrets)"
  gdbus introspect --session --dest org.freedesktop.secrets --object-path /org/freedesktop/secrets >/dev/null
  export XDG_DATA_HOME="$1/data" XDG_CONFIG_HOME="$1/config" COPYPASTE_SOCKET="$1/copypaste.sock"
  "$2/copypaste-daemon" --foreground --data-dir "$1/runtime" --port 0 >"$1/daemon.log" 2>&1 &
  daemon=$!
  trap "kill $daemon 2>/dev/null || true; wait $daemon 2>/dev/null || true" EXIT
  for _ in $(seq 1 50); do
    "$2/copypaste-cli" status >/dev/null 2>&1 && break
    sleep 0.1
  done
  "$2/copypaste-cli" status >/dev/null
  "$2/copypaste-cli" shutdown >/dev/null
  wait "$daemon"
' _ "$WORK" "$PREFIX"

# Keep the runtime ceiling explicit. The release receipt records the actual
# highest symbol requirement for each architecture.
MAX_GLIBC="${COPYPASTE_MAX_GLIBC:-2.39}"
for executable in "$PREFIX/copypaste" "$PREFIX/copypaste-daemon" "$PREFIX/copypaste-cli"; do
  max_glibc="$(readelf --version-info "$executable" | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -V | tail -n1)"
  [[ -n "$max_glibc" ]] && [[ "$(printf '%s\n%s\n' "$max_glibc" "$MAX_GLIBC" | sort -V | head -n1)" == "$max_glibc" ]] || {
    echo "ERROR: $executable exceeds the glibc $MAX_GLIBC runtime ceiling" >&2
    exit 1
  }
done
echo "verified $ARTIFACT on native $ARCHITECTURE $DESKTOP $SESSION"
