#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
manifest="$root/manifest.json"
gnome="$root/gnome-shell-extension"
kde="$root/kde-kwin-script"

jq -e '
  .schemaVersion == 1 and
  .quickPasteBridge.busName == "app.copypaste.CopyPaste" and
  .quickPasteBridge.objectPath == "/app/copypaste/WaylandIntegration" and
  .quickPasteBridge.interface == "app.copypaste.WaylandIntegration" and
  ([.integrations[].id] | sort == ["gnome-shell", "kde-kwin"]) and
  (.integrations[] | select(.id == "gnome-shell") |
    .extensionId == "copypaste-quick-paste@copypaste.app" and
    .enable == "user" and
    .supportedShellVersions == ["46", "47", "48"]) and
  (.integrations[] | select(.id == "kde-kwin") |
    .packageId == "copypaste-quick-paste" and
    .hostDesktopFileName == "com.copypaste.CopyPaste" and
    .enable == "user" and
    .supportedPlasmaVersions == ["6"])
' "$manifest" >/dev/null

jq -e '
  .uuid == "copypaste-quick-paste@copypaste.app" and
  .["shell-version"] == ["46", "47", "48"]
' "$gnome/metadata.json" >/dev/null
jq -e '
  .KPlugin.Id == "copypaste-quick-paste" and
  .["X-Plasma-API"] == "javascript" and
  .KPackageStructure == "KWin/Script"
' "$kde/metadata.json" >/dev/null

for method in AwaitQuickPaste BeginQuickPaste PasteIntoRestoredWindow CancelQuickPaste; do
  rg -F "$method" "$gnome/extension.js" "$kde/contents/code/main.js" >/dev/null
done
rg -F "app.copypaste.GnomeIntegration" "$gnome/extension.js" >/dev/null
rg -F "workspace.activateWindow(pending.window)" "$kde/contents/code/main.js" >/dev/null
rg -F "workspace.activeWindow !== pending.window" "$kde/contents/code/main.js" >/dev/null
rg -F "window.desktopFileName === HOST_DESKTOP_FILE_NAME" "$kde/contents/code/main.js" >/dev/null
rg -F "pending.window.activate" "$gnome/extension.js" >/dev/null
rg -F "Gio.bus_watch_name_on_connection" "$gnome/extension.js" >/dev/null
rg -F "AWAIT_TIMEOUT_MS = 605000" "$gnome/extension.js" >/dev/null
clipboard_bridge="$gnome/clipboard_bridge.js"
rg -F "app.copypaste.Daemon" "$clipboard_bridge" >/dev/null
rg -F "const MAX_BYTES = 4 * 1024 * 1024" "$clipboard_bridge" >/dev/null
rg -F "const MAX_WRITE_TOTAL_BYTES = 32 * 1024 * 1024" "$clipboard_bridge" >/dev/null
rg -F "const MAX_MIMES = 64" "$clipboard_bridge" >/dev/null
for error in AccessDenied StaleSelection UnsupportedMime TooLarge Unavailable; do
  rg -F "$error" "$clipboard_bridge" >/dev/null
done
if rg -F "registerShortcut(" "$kde/contents/code/main.js"; then
  echo "KWin must use the GlobalShortcuts portal binding." >&2
  exit 1
fi
if rg -F "addKeybinding(" "$gnome/extension.js"; then
  echo "GNOME must use the GlobalShortcuts portal binding." >&2
  exit 1
fi

if rg -i 'clipboard|primary selection|window title|\.get_title\(' \
  "$kde/contents/code/main.js"; then
  echo "The KWin focus companion must not transport clipboard or window metadata." >&2
  exit 1
fi

gnome_activate=$(rg -n -F "pending.window.activate" "$gnome/extension.js" | head -n 1 | cut -d: -f1)
gnome_paste=$(rg -n -F "'PasteIntoRestoredWindow'" "$gnome/extension.js" | cut -d: -f1)
kde_activate=$(rg -n -F "workspace.activateWindow(pending.window)" "$kde/contents/code/main.js" | head -n 1 | cut -d: -f1)

[ "$gnome_activate" -lt "$gnome_paste" ]
[ "$kde_activate" -gt 0 ]

node "$root/test_runtime.mjs"
