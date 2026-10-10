#!/usr/bin/env bash
# Validate the maintained source-patch boundary without touching a live KWin.
set -euo pipefail

root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

make_fixture() {
  local version="$1"
  local tree="$work/$version" header_padding cpp_padding wayland_padding wayland_display_line
  if [[ "$version" == 6.0 ]]; then
    header_padding=123
    cpp_padding=488
    wayland_padding=366
    wayland_display_line=375
  else
    header_padding=133
    cpp_padding=519
    wayland_padding=370
    wayland_display_line=379
  fi
  mkdir -p "$tree/src"
  printf 'set(PROJECT_VERSION "%s.0")\n' "$version" > "$tree/CMakeLists.txt"
  if [[ "$version" == 6.0 ]]; then
    {
      for _ in $(seq 1 31); do printf '\n'; done
      cat <<'EOF'
target_compile_definitions(kwin PRIVATE
    -DTRANSLATION_DOMAIN=\"kwin\"
)

target_sources(kwin PRIVATE
    client_machine.cpp
    colors/colordevice.cpp
    colors/colormanager.cpp
    compositor.cpp
    compositor_wayland.cpp
    compositor_x11.cpp
)
EOF
    } > "$tree/src/CMakeLists.txt"
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
target_compile_definitions(kwin PRIVATE
    -DTRANSLATION_DOMAIN=\"kwin\"
)

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
  awk -v padding="$wayland_padding" '
    { print }
    /bool WaylandServer::init/ { in_init = 1; next }
    in_init && $0 == "{" {
      for (line_number = 0; line_number < padding; ++line_number) print ""
      in_init = 0
    }
  ' "$tree/src/wayland_server.cpp" > "$tree/src/wayland_server.cpp.padded"
  mv "$tree/src/wayland_server.cpp.padded" "$tree/src/wayland_server.cpp"
  test "$(rg -n -F 'm_display->createShm();' "$tree/src/wayland_server.cpp" | cut -d: -f1)" = "$wayland_display_line"
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
  if [[ "$version" == 6.0 ]]; then
    rg -F 'COPYPASTE_KWIN_6_0' "$tree/src/CMakeLists.txt" >/dev/null
  elif rg -F 'COPYPASTE_KWIN_6_0' "$tree/src/CMakeLists.txt"; then
    echo "ERROR: KWin 6.3 must use the serial selection API" >&2
    exit 1
  fi
}

bash -n "$root/apply-to-kwin-source.sh"
rg -F 'patch --batch --forward --fuzz=0 -p1 --directory "$source_root"' "$root/apply-to-kwin-source.sh" >/dev/null
bash -n "$root/verify-build.sh"
bash -n "$root/run-fedora-build.sh"
rg -F 'FROM fedora:40@sha256:' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F 'dnf builddep' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F 'ARG KWIN_FAMILY' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F '6.0)' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F 'dnf config-manager --set-disabled updates updates-source' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F '6.3)' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F 'dnf config-manager --set-enabled fedora-source updates-source' "$root/Dockerfile.fedora40-build" >/dev/null
rg -F -- '--build-arg "KWIN_FAMILY=$selected"' "$root/run-fedora-build.sh" >/dev/null
rg -F 'prepare_runtime_output()' "$root/run-fedora-build.sh" >/dev/null
rg -F 'runtime output must be empty' "$root/run-fedora-build.sh" >/dev/null
rg -F 'runtime output must be an ordinary mounted directory' "$root/verify-build.sh" >/dev/null
rg -F 'runtime output must be empty' "$root/verify-build.sh" >/dev/null
rg -F 'cmake --build' "$root/verify-build.sh" >/dev/null
rg -F 'Version() == 2' "$root/DISTRIBUTION.md" >/dev/null
fake_engine="$work/container-engine"
fake_log="$work/container-engine.log"
cat > "$fake_engine" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >> "$COPYPASTE_FAKE_ENGINE_LOG"
if [[ "$1" == run ]]; then
  output=""
  previous=""
  for argument in "$@"; do
    if [[ "$previous" == --volume && "$argument" == *:/output ]]; then
      output="${argument%:/output}"
    fi
    previous="$argument"
  done
  [[ -d "$output" && -z "$(find "$output" -mindepth 1 -maxdepth 1 -print -quit)" ]]
fi
EOF
chmod +x "$fake_engine"
runtime_output="$work/runtime-output"
COPYPASTE_CONTAINER_ENGINE="$fake_engine" COPYPASTE_FAKE_ENGINE_LOG="$fake_log" \
  "$root/run-fedora-build.sh" linux/amd64 6.0 "$runtime_output"
[[ -d "$runtime_output" && -z "$(find "$runtime_output" -mindepth 1 -maxdepth 1 -print -quit)" ]]
test "$(wc -l < "$fake_log" | tr -d '[:space:]')" = 2
touch "$runtime_output/prepopulated"
if COPYPASTE_CONTAINER_ENGINE="$fake_engine" COPYPASTE_FAKE_ENGINE_LOG="$fake_log" \
  "$root/run-fedora-build.sh" linux/amd64 6.0 "$runtime_output" >/dev/null 2>&1; then
  echo "ERROR: runtime handoff accepted prepopulated output" >&2
  exit 1
fi
test "$(wc -l < "$fake_log" | tr -d '[:space:]')" = 2
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
rg -F '#if defined(COPYPASTE_KWIN_6_0)' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'setSelection(source);' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
rg -F 'setSelection(source, waylandServer()->display()->nextSerial());' "$root/src/copypasteclipboardbridge.cpp" >/dev/null
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
