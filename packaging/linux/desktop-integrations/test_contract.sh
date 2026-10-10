#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
manifest="$root/manifest.json"
gnome="$root/gnome-shell-extension"
kde="$root/kde-kwin-script"
native_kde="$root/kde-native-clipboard"

jq -e '
  .schemaVersion == 1 and
  .quickPasteBridge.busName == "app.copypaste.CopyPaste" and
  .quickPasteBridge.objectPath == "/app/copypaste/WaylandIntegration" and
  .quickPasteBridge.interface == "app.copypaste.WaylandIntegration" and
  ([.integrations[].id] | sort == ["gnome-shell", "kde-kwin"]) and
  (.integrations[] | select(.id == "gnome-shell") |
    .extensionId == "copypaste-quick-paste@copypaste.app" and
    .enable == "user" and
    .supportedShellVersions == ["46", "47"]) and
  (.integrations[] | select(.id == "kde-kwin") |
    .packageId == "copypaste-quick-paste" and
    .hostDesktopFileName == "com.copypaste.CopyPaste" and
    .enable == "user" and
    .supportedPlasmaVersions == ["6"] and
    .nativeClipboard.source == "kde-native-clipboard" and
    .nativeClipboard.supportedKWinSourceVersions == ["6.0", "6.3"] and
    .nativeClipboard.install == "distribution-source-package-only")
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
rg -F 'type="(suus)"' "$clipboard_bridge" >/dev/null
rg -F "const PROTOCOL_VERSION = 2" "$clipboard_bridge" >/dev/null
rg -F "this._identity = this._writerIdentity(this._currentOwner())" "$clipboard_bridge" >/dev/null
rg -F "['no-client', 0, 0, '']" "$clipboard_bridge" >/dev/null
rg -F "const [status, pid, uid, appId]" "$clipboard_bridge" >/dev/null
for status in verified no-client no-app-id ambiguous; do
  rg -F "\"$status\"" "$gnome/mutter/mutter-46-writer-identity.patch" >/dev/null
done
rg -F "meta_selection_source_get_writer_identity" "$gnome/native/copypaste-clipboard-source.c" >/dev/null
rg -F "meta_selection_get_current_owner" "$gnome/native/copypaste-clipboard-source.c" >/dev/null
rg -F "copypaste_clipboard_source_is_available" "$gnome/native/copypaste-clipboard-source.h" >/dev/null
rg -F "return ensure_source_type (error) && resolve_mutter_api (error);" "$gnome/native/copypaste-clipboard-source.c" >/dev/null
rg -F "clipboard_source_is_available(this._selection) === true" "$clipboard_bridge" >/dev/null
rg -F "wl_resource_get_client" "$gnome/mutter/mutter-46-writer-identity.patch" >/dev/null
rg -F "meta_wayland_surface_get_resource" "$gnome/mutter/mutter-46-writer-identity.patch" >/dev/null
rg -F "window_app_id = meta_window_get_wm_class (window);" "$gnome/mutter/mutter-46-writer-identity.patch" >/dev/null
if rg -F "meta_window_get_gtk_application_id" "$gnome/mutter/mutter-46-writer-identity.patch"; then
  echo "Mutter writer bridge must use the Wayland app-id WM_CLASS field." >&2
  exit 1
fi
rg -F "const MAX_MIMES = 64" "$clipboard_bridge" >/dev/null
for error in AccessDenied StaleSelection UnsupportedMime TooLarge Unavailable; do
  rg -F "$error" "$clipboard_bridge" >/dev/null
done
if rg -F "registerShortcut(" "$kde/contents/code/main.js"; then
  echo "KWin must use the GlobalShortcuts portal binding." >&2
  exit 1
fi
if rg -F "addKeybinding(" "$gnome/extension.js"; then
  echo "GNOME must not use an extension-local keybinding setting." >&2
  exit 1
fi
shortcut_bridge="$gnome/shortcut_bridge.js"
rg -F "app.copypaste.GnomeShortcuts" "$shortcut_bridge" >/dev/null
rg -F "/app/copypaste/GnomeShortcuts" "$shortcut_bridge" >/dev/null
rg -F "global.display.grab_accelerator" "$shortcut_bridge" >/dev/null
rg -F "global.display.ungrab_accelerator" "$shortcut_bridge" >/dev/null
rg -F "Meta.external_binding_name_for_action" "$shortcut_bridge" >/dev/null
rg -F "Main.wm.allowKeybinding" "$shortcut_bridge" >/dev/null
rg -F "Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW" "$shortcut_bridge" >/dev/null
rg -F "<signal name=\"Activated\"" "$shortcut_bridge" >/dev/null
rg -F "new ShortcutBridge" "$gnome/extension.js" >/dev/null

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
"$native_kde/test-source-patches.sh"
