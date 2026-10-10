# Linux desktop integrations

These optional companions provide Wayland Quick Paste focus restoration. They
are shipped for GNOME Shell 46 and 47, and for Plasma 6 through KWin's
public JavaScript scripting API. They use the session bus and require a running
CopyPaste app with an active, user-consented RemoteDesktop portal session.

The package manager and AppImage installer place the companion files, but never
enable either integration. The user enables the companion after installation.

## Install locations

`manifest.json` is the distribution source of truth. A system package copies
each source directory to its `systemInstallPath`. An AppImage installer uses the
same entry's `userInstallPath`.

| Desktop | Package source | System path | User action |
| --- | --- | --- | --- |
| GNOME Shell | `gnome-shell-extension` | `/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app` | Enable `copypaste-quick-paste@copypaste.app` in Extensions, or run `gnome-extensions enable copypaste-quick-paste@copypaste.app`. |
| Plasma / KWin | `kde-kwin-script` | `/usr/share/kwin/scripts/copypaste-quick-paste` | Enable **CopyPaste Quick Paste** in System Settings → Window Management → KWin Scripts. For an AppImage user install, run `kpackagetool6 --type KWin/Script --install <source-directory>` first. |

The package also carries `kde-native-clipboard` under
`/usr/share/copypaste/desktop-integrations`. It is source for a maintained KWin
6.0 or 6.3 build patch, not a loadable Script or Effect. A distribution
maintainer applies it when building an authenticated KWin source package. It
never replaces or restarts a user compositor, and the Clipboard v2 transport is
unavailable until that patched KWin package is installed.

KWin owns the Quick Paste binding through the user-consented GlobalShortcuts
portal. GNOME Shell 46 and 47 do not provide that portal interface, so the
GNOME extension registers the same canonical GTK accelerator with Mutter. The
application settings still own one binding; no companion adds a separate
desktop-specific setting.

## GNOME shortcut bridge

The GNOME extension exports `app.copypaste.GnomeShortcuts` at
`/app/copypaste/GnomeShortcuts` on its
`app.copypaste.GnomeIntegration` session-bus name. `Version() -> (u)` returns
`1`. The host registers canonical GTK accelerators with
`RegisterShortcut(s id, s accelerator) -> (b registered, s triggerDescription)`
and removes them with `UnregisterShortcut(s id) -> (b removed)`. A successful
registration returns the exact accepted accelerator; a failed grab returns
`false` and an empty description. Mutter collisions are never reported as a
successful registration.

The bridge accepts register and unregister calls only from the current unique
owner of `app.copypaste.CopyPaste` with the extension's Unix UID. `Version` is
public. It emits `Activated(s id)` when Mutter activates a registered binding;
the native host resolves its existing `AwaitQuickPaste` request, preserving the
focus-restoration transaction. Loss of the host bus name and extension disable
release every Mutter grab and bus watcher.

## GNOME clipboard bridge

The bridge uses the `native/` GObject shim because Mutter 46 GJS rejects the
required asynchronous `Meta.SelectionSource` virtual method. Distribution builds
run `make -C native` and stage its library and typelib in `native/lib` and
`native/typelib` beside the extension; these generated binaries are not tracked.

`Version() -> (u)` returns `2`. `Snapshot() -> (t, as, (s u u s))` and
`OwnerChanged(t, as, (s u u s))` contain
the selection sequence, MIME names, and a classified writer identity. The
identity fields are status, PID, UID, and app ID. Status is exactly one of
`verified`, `no-client`, `no-app-id`, or `ambiguous`; app ID is non-empty only
for `verified`. `Read(t, s, u) -> ay` requires the current sequence and a
requested size from 1 through 4 MiB. `Write(a{say}) -> t` accepts at most 64
MIME names of at most 255 UTF-8 bytes. Each payload is at most 4 MiB and the
whole write is at most the 32 MiB IPC frame limit, so a maximal primary payload
can keep its required privacy marker or fallback representation. The bridge
rejects oversize data instead of dropping formats. A
selection owner change cancels in-flight reads; no clipboard bytes are logged
or included in signals.

The identity tuple is captured on the owner transition before any payload
transfer. The host rejects a non-empty exclusion list when the tuple is
unknown and accepts it only while the sequence still matches the `Read()`
request. The maintained Mutter export maps the exact Wayland source client to
Mutter's own windows and returns an app ID only when every matching window has
the same non-empty value.

## Bridge contract

The companion keeps the focused native window only in its process. It sends no
clipboard data, item content, source-app data, title, PID, or window identifier
over D-Bus.

| Direction | Method | Result and bound |
| --- | --- | --- |
| Companion → host | `AwaitQuickPaste()` | Deferred `(b triggered)` reply. `true` is a GlobalShortcuts portal activation; `false` is a real cancellation, replacement, timeout, or portal-session loss. The host holds an unavailable-portal replacement await instead of polling. |
| Companion → host | `BeginQuickPaste(s transactionId)` | Deferred `(b accepted)` reply when the selection is committed, cancelled, or expires. The host caps it at 120 seconds. |
| Companion → host | `PasteIntoRestoredWindow(s transactionId)` | `(b pasted)` and is valid once, within two seconds after a successful begin result. |
| Companion → host | `CancelQuickPaste(s transactionId)` | `(b cancelled)`; idempotently releases a pending transaction. |
| Host → companion | `TransactionCancelled(s transactionId)` | GNOME stops an accepted transaction if the host loses its portal session before paste. |

The host is `app.copypaste.CopyPaste` at
`/app/copypaste/WaylandIntegration`, interface
`app.copypaste.WaylandIntegration`. Transaction IDs are opaque ASCII UUIDs;
the host accepts only `[A-Za-z0-9-]` IDs up to 64 bytes.

The GNOME extension watches the host's session-bus name. It does not create an
await request until it owns its companion name and the host is present. Host
loss cancels every outstanding D-Bus call and invalidates its callbacks. A
restarted host starts one fresh await. Await, selection, and paste calls use
their 10-minute, 125-second, and 3-second contracts instead of GIO's default
25-second timeout. A failed await retries with a capped 1–60 second backoff.

KWin's public JavaScript `callDBus` API does not invoke its callback for a
D-Bus error. The script therefore recovers an unresolved await only when KWin
observes a new or activated window whose `appId` is the canonical
`app.copypaste.CopyPaste`; each recovery invalidates the old callback before
creating one replacement await. This is event-driven and creates no timer
poll. Before asking the host to paste, KWin verifies that
`workspace.activeWindow` is the saved window; a denied restore cancels the
transaction.

GNOME acquires `app.copypaste.GnomeIntegration`; the host verifies that this
name's session-bus owner and the host have the same Unix UID. KWin scripts
cannot acquire a custom bus name through their public API, so the host accepts
only a same-UID sender currently owned by `org.kde.KWin`. It rejects every
other sender, expired transaction, and out-of-order paste. Each companion has
at most one await outstanding. GNOME retries a completed false await after one
second. KWin immediately replaces it; native holds that request while the
portal is unavailable, so it never becomes a busy poll.

After a `true` await result, the companion captures the currently focused
window and begins Quick Paste. After a `true` begin result, it activates that
local window and immediately requests paste. If activation, D-Bus, or portal
input fails, it cancels and the selection remains copy-only. A `false` result
is a cancel/timeout and never restores or pastes.

## Linux capability boundary

The integrations cover X11 and Wayland Quick Paste focus restoration on GNOME
and Plasma. Linux does not provide a supported per-window block-screenshot API
for this Flutter/GTK window, so **Block screenshots** reports unavailable on
Linux. This is the approved platform exception.
