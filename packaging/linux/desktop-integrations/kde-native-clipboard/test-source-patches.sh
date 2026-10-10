#!/usr/bin/env bash
# Validate the maintained source-patch boundary without touching a live KWin.
set -euo pipefail

root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

make_fixture() {
  local version="$1"
  local tree="$work/$version"
  mkdir -p "$tree/src"
  printf 'set(PROJECT_VERSION "%s.0")\n' "$version" > "$tree/CMakeLists.txt"
  if [[ "$version" == 6.0 ]]; then
    cat > "$tree/src/CMakeLists.txt" <<'EOF'
target_sources(kwin PRIVATE
    client_machine.cpp
    colors/colordevice.cpp
    colors/colormanager.cpp
    compositor.cpp
    compositor_wayland.cpp
    compositor_x11.cpp
)
EOF
    cat > "$tree/src/wayland_server.cpp" <<'EOF'
/*
 */
#include "wayland_server.h"

#include <config-kwin.h>

bool WaylandServer::init(const QString &socketName)
{
    m_display->createShm();
    m_seat = new SeatInterface(m_display, m_display);
    new PointerGesturesV1Interface(m_display, m_display);
    new PointerConstraintsV1Interface(m_display, m_display);
    new RelativePointerManagerV1Interface(m_display, m_display);
    m_dataDeviceManager = new DataDeviceManagerInterface(m_display, m_display);
}
EOF
  else
    cat > "$tree/src/CMakeLists.txt" <<'EOF'
target_sources(kwin PRIVATE
    appmenu.cpp
    client_machine.cpp
    colors/colordevice.cpp
    colors/colormanager.cpp
    compositor.cpp
    compositor_wayland.cpp
    core/brightnessdevice.cpp
)
EOF
    cat > "$tree/src/wayland_server.cpp" <<'EOF'
/*
 */
#include "wayland_server.h"

#include "config-kwin.h"

bool WaylandServer::init(const QString &socketName)
{
    m_display->createShm();
    m_seat = new SeatInterface(m_display, kwinApp()->session()->seat(), m_display);
    new PointerGesturesV1Interface(m_display, m_display);
    new PointerConstraintsV1Interface(m_display, m_display);
    new RelativePointerManagerV1Interface(m_display, m_display);
    m_dataDeviceManager = new DataDeviceManagerInterface(m_display, m_display);
}
EOF
  fi
  "$root/apply-to-kwin-source.sh" "$version" "$tree"
  test -f "$tree/src/copypasteclipboardbridge.cpp"
  test -f "$tree/src/copypasteclipboardbridge.h"
  rg -F 'copypasteclipboardbridge.cpp' "$tree/src/CMakeLists.txt" >/dev/null
  rg -F 'new CopyPasteClipboardBridge(this);' "$tree/src/wayland_server.cpp" >/dev/null
}

bash -n "$root/apply-to-kwin-source.sh"
make_fixture 6.0
make_fixture 6.3

for forbidden in 'activeWindow' 'caption' 'executablePath' 'get_title'; do
  if rg -F "$forbidden" "$root/src/copypasteclipboardbridge.cpp"; then
    echo "ERROR: native KWin attribution must not use $forbidden" >&2
    exit 1
  fi
done
rg -F 'source->client()' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'window->surface()->client() == client' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'QStringLiteral("ambiguous")' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'QStringLiteral("no-app-id")' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'QStringLiteral("no-client")' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
