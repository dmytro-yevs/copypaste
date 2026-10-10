#!/usr/bin/env bash
# Validate the maintained source-patch boundary without touching a live KWin.
set -euo pipefail

root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

make_fixture() {
  local version="$1"
  local tree="$work/$version" header_padding cpp_padding
  if [[ "$version" == 6.0 ]]; then
    header_padding=123
    cpp_padding=488
  else
    header_padding=133
    cpp_padding=519
  fi
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
  {
    for _ in $(seq 1 "$header_padding"); do printf '\n'; done
    cat <<'EOF'
class XdgToplevelInterface;
class XdgToplevelWindow {
public:
    XdgToplevelInterface *shellSurface() const;

    MaximizeMode maximizeMode() const;
    MaximizeMode requestedMaximizeMode() const;
    QSizeF minSize() const;
};
EOF
  } > "$tree/src/xdgshellwindow.h"
  {
    for _ in $(seq 1 "$cpp_padding"); do printf '\n'; done
    cat <<'EOF'
XdgToplevelInterface *XdgToplevelWindow::shellSurface() const
{
    return m_shellSurface;
}

MaximizeMode XdgToplevelWindow::maximizeMode() const
{
    return m_maximizeMode;
}
EOF
  } > "$tree/src/xdgshellwindow.cpp"
  "$root/apply-to-kwin-source.sh" "$version" "$tree"
  test -f "$tree/src/copypasteclipboardbridge.cpp"
  test -f "$tree/src/copypasteclipboardbridge.h"
  rg -F 'copypasteclipboardbridge.cpp' "$tree/src/CMakeLists.txt" >/dev/null
  rg -F 'new CopyPasteClipboardBridge(this);' "$tree/src/wayland_server.cpp" >/dev/null
  rg -F 'QString rawAppId() const;' "$tree/src/xdgshellwindow.h" >/dev/null
  rg -F 'return m_shellSurface->windowClass();' "$tree/src/xdgshellwindow.cpp" >/dev/null
}

bash -n "$root/apply-to-kwin-source.sh"
rg -F 'patch --batch --forward --fuzz=0 -p1 --directory "$source_root"' "$root/apply-to-kwin-source.sh" >/dev/null
bash -n "$root/verify-build.sh"
bash -n "$root/run-fedora-build.sh"
rg -F 'FROM fedora:40@sha256:' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F 'dnf builddep' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F 'dnf config-manager --set-enabled fedora-source' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F 'dnf config-manager --set-disabled updates updates-source' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F 'cmake --build' "$root/verify-build.sh" >/dev/null
rg -F 'Version() == 2' "$root/DISTRIBUTION.md" >/dev/null
if [[ "${COPYPASTE_VERIFY_QT_WIRE:-0}" == 1 ]]; then
  wire_build="$work/wire-build"
  cmake -S "$root/test-wire" -B "$wire_build" -DCMAKE_PREFIX_PATH="${COPYPASTE_QT_PREFIX:?set COPYPASTE_QT_PREFIX to a Qt 6 SDK prefix}"
  cmake --build "$wire_build"
  "$wire_build/copypaste-kwin-clipboard-wire-fixture"
fi
make_fixture 6.0
make_fixture 6.3

for forbidden in 'activeWindow' 'caption' 'executablePath' 'get_title' 'desktopFileName'; do
  if rg -F "$forbidden" "$root/src/copypasteclipboardbridge.cpp"; then
    echo "ERROR: native KWin attribution must not use $forbidden" >&2
    exit 1
  fi
done
rg -F 'source->client()' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'toplevel->surface()->client() != client' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'toplevel->rawAppId()' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'kAuthorizerTimeoutMs = 250' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'cancelPendingTransfers();' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F '++m_daemonOwnerEpoch;' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'bridge->m_daemonOwnerEpoch != daemonOwnerEpoch' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
for export in 'Q_SCRIPTABLE void Version(uint &version);' 'Q_SCRIPTABLE void Snapshot(' 'Q_SCRIPTABLE QByteArray Read(' \
    'Q_SCRIPTABLE qulonglong Write(' 'Q_SCRIPTABLE void OwnerChanged('; do
  rg -F "$export" "$root/src/copypasteclipboardbridge.h" >/dev/null
done
rg -F 'bool CopyPasteClipboardBridge::authorize(bool allowGui)' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'if (!authorize(true))' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'kGuiBusName = "app.copypaste.CopyPaste"' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
test "$(rg -F -c 'if (!authorize())' "$root/src/copypasteclipboardbridge.cpp")" = 3
rg -F 'auto bus = QDBusConnection::sessionBus();' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
if rg -F 'const auto bus = QDBusConnection::sessionBus();' "$root/src/copypasteclipboardbridge.cpp"; then
  echo "ERROR: the KWin bridge must keep its exported D-Bus connection mutable" >&2
  exit 1
fi
rg -F 'const auto sender = message().service();' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'qDBusRegisterMetaType<QMap<QString, QByteArray>>();' "$root/test-wire/main.cpp" >/dev/null
rg -F 'qDBusRegisterMetaType<QMap<QString, QByteArray>>();' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'QDBusConnection::connectToPeer(server.address(), connectionName)' "$root/test-wire/main.cpp" >/dev/null
rg -F 'QDBusConnection::disconnectFromPeer(connectionName);' "$root/test-wire/main.cpp" >/dev/null
rg -F 'hasMissingAppId' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'QStringLiteral("ambiguous")' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'QStringLiteral("no-app-id")' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'QStringLiteral("no-client")' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
