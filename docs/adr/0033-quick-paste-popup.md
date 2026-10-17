# ADR-0033: Quick Paste uses a transient desktop window

Status: accepted

## Decision

The macOS and Windows clients provide a keyboard-first Quick Paste popup in a
dedicated transient Flutter window. The default global shortcut is
Shift-Command-C on macOS and Shift-Control-C on Windows. Users can replace the
shortcut in Settings.

Auto-paste is enabled by default and remains user-configurable. Selecting an
item always asks the existing History repository to place it on the system
clipboard first. The native adapter may then paste into the previously focused
application:

- macOS posts Command-V only while Accessibility trust is confirmed. Opening
  Quick Paste never prompts for permission. The first actual paste attempt may
  request Accessibility once; the request is recorded independently of the
  auto-paste setting and survives restarts. Later attempts remain copy-only
  until trust is granted. Users can request permission explicitly in Settings.
- Windows restores the previously focused window and sends Control-V through
  `SendInput`.

If trust is lost at runtime, denied, or the native paste operation fails,
selection still succeeds as copy-only and the popup closes. Auto-paste must not
make a history item unusable after it has already been copied successfully.

## Engine lifetime

The main engine registers the global shortcut without starting the Quick Paste
engine. A popup engine is created on demand. Its initialized Dart controller
signals readiness before the native host delivers the current presentation ID.
That acknowledgement prepares the presentation; it does not reveal an empty
native window. The host first positions the view while hidden. After the opened
handler finishes, Flutter waits for the mounted viewport's image previews and
source icons to decode, submits that frame, and sends `present` with the same
presentation ID. Offscreen rows are not preloaded. Missing media uses the normal
row fallback. Stale IDs cannot reveal a newer presentation, and a foreground
change during startup cancels the pending reveal instead of stealing focus.
Windows additionally waits for its next rasterized frame before showing and
activating the popup. The macOS Flutter view explicitly uses a clear background
instead of Flutter's default initial black surface.

The popup registers only the native plugins it uses: file selection and shared
preferences on macOS, and file selection on Windows. Other platform preferences
and path lookup use their existing Dart/FFI implementations. Camera, capture
notifications, tray, global shortcut, and main-window plugins remain owned by
the main engine. No hidden engine is prewarmed or retained for this handshake.
The first presentation reuses the initial watched History page instead of
clearing and fetching it again. Later presentations still refresh History and
preferences. Quick Paste does not request filter facets, which only the main
History surface uses. Independent preference reads and initial permission
checks may overlap with the initial History load.

Installed-module metadata initializes asynchronously when the inspector is
first requested. It does not delay `runApp`, ordinary clipboard rows, or the
native ready acknowledgement. OCR availability updates when that metadata
arrives. This does not prewarm or retain a hidden popup engine.
Closing, deactivation, and completed paste retire the hidden engine on the next
native event-loop turn. A reopened presentation cancels pending retirement.

Before shutdown, Dart disposes its owned History repository and awaits the Rust
watch lease cancellation. Native shutdown waits for that acknowledgement, with
a one-second deadline for a context that failed during startup. macOS explicitly
shuts down FlutterEngine and detaches its view; Windows destroys the retired
FlutterViewController outside its own window/message callbacks. A generation
keeps a delayed Windows reply from destroying a newer popup. Android has no
separate Quick Paste engine.

## Window placement

The primary tray-icon click opens this same Quick Paste presentation on macOS
and Windows. Flutter renders the same rows, media previews, search, pinned
items, inspector, keyboard commands, and footer actions for both entry points;
there is no text-only tray history or separate clipboard repository. Selection
uses the same copy, auto-paste preference, permission handling, and previous
application target. Right-click retains the native application controls.

On macOS, the native status button routes its primary action directly to the
same native opener used by the shortcut. It samples the foreground application
and Cocoa icon rectangle at the click, then leaves AppKit button tracking before
opening. It does not round-trip through a nativeapi FFI callback, the main Dart
isolate, and another platform-channel request. Control-click and right-click
forward to the original nativeapi target. Detaching the binding cancels queued
activation and restores only the target it owns, before Dart releases the icon.
No event monitor, polling timer, or extra engine is introduced.

Latency comparisons must use the same bundle, build mode, workload, and cache state;
co-running production and Dev instances can own different global shortcuts.

Tray presentations align to the icon's right edge and open on the side with
available work-area space. Their height is at most 600 logical pixels and is
further constrained by that space. Search and footer stay fixed while History
scrolls and paginates. Inspector expansion preserves the bounded height and
tray anchor. The shortcut retains its cursor placement and 800-pixel maximum.
Both paths create the existing transient engine only on demand and retire it
through the same close, deactivation, and completed-paste lifecycle. Native tray
bounds use global top-left coordinates in the channel fallback; macOS converts
those against the primary screen. Direct native macOS activation already uses
Cocoa coordinates. Windows selects the tray's monitor and
scales dimensions for its DPI before placement. Linux tray activation uses its
existing native Quick Paste backend and compositor placement capabilities.

The required regression matrix covers every clip kind in the bounded layout,
image pixels rather than placeholder labels, file names, search and pagination,
fixed footer visibility, inspector expansion, and unchanged copy/paste outcomes.
Native placement checks cover top/bottom trays, negative monitor origins, short
work areas, and Windows display scaling. Native resource qualification remains
separate from these functional checks.

The popup uses a compact menu layout: title and search share the top row,
source icons and content previews align with shortcuts, pinned clips follow
recent clips after a divider, and footer actions form a vertical list. The
header uses a small brand logo. The menu uses shared 13-point typography.
Recent clips use numeric shortcuts; pinned clips get unique, persisted letter
shortcuts that remain stable across search and restart. Reserved system and
menu keys are excluded. macOS uses Command and Windows uses Control.
The initial desktop size is 448 by 800 logical
pixels, clamped to the display work area.

The header inspector control expands the same native window to 816 logical
pixels and reuses the History inspector. The inspector follows the focused
clip, loads its full content through the History repository, and uses
11-point metadata text. Expanding, collapsing, and reopening clamp the window
to its current monitor's work area without changing the paste target.

The inspector exposes the same shared actions as History: copy, plain-text copy,
pin/unpin, secret/non-secret, delete with confirmation, file download, and image OCR when its module
is available. Inspector copy keeps the popup open; row and Enter activation keep
the copy/paste/close behavior. Actions remain outside the scrolling content.
Pin and delete updates reconcile the focused clip and keyboard shortcuts.
The popup engine owns its OCR controller and initializes installed-module
metadata on the first inspector request.
Native save sheets on macOS and owned save dialogs on Windows retain the popup
and paste target until the dialog returns, including cancellation. Android uses
the same History inspector actions in the main application.

Hovering a clip for 1500 milliseconds opens this same inspector automatically,
matching Maccy's default preview delay. Leaving the row before the delay cancels
the pending open. Once open, the inspector stays available and follows the
focused clip immediately. Manually closing it suppresses automatic reopening
for that clip until selection changes. Scrolling, search, paste, popup close,
reopen, and shutdown cancel pending hover work. Manual controls remain available
for keyboard and touch input.

Only explicit inspector toggles persist the expanded state across popup
presentations. Hover expansion lasts until that Quick Paste presentation closes;
it never overwrites the manually selected state. The native resize channel
receives visibility and persistence separately.

The shared OCR dialog reserves the same content area while loading, displaying
text, reporting errors, and showing an empty result. Text scrolls inside that
area and actions remain visible. OCR runs in the authenticated inference worker;
the host reaps the worker after every request so model allocator memory cannot
remain in the daemon or Android application process.

The header logo opens the main History destination. The main application also
accepts Command-Comma on macOS and Control-Comma on Windows and Android hardware
keyboards to open Settings. The native macOS Settings menu routes to that same
destination.

Reference: [Maccy preview controller](https://github.com/p0deje/Maccy/blob/a92c11ae3e7a86a57dc6359bad59e330e4625ede/Maccy/Observables/SlideoutController.swift)
and [default delay](https://github.com/p0deje/Maccy/blob/a92c11ae3e7a86a57dc6359bad59e330e4625ede/Maccy/Extensions/Defaults.Keys%2BNames.swift).

Each shortcut invocation reads the current pointer position, selects the monitor that
contains that point, and places the popup's top-left corner immediately below
the pointer. The result is clamped to that monitor's work area, including
negative coordinates, differing scale factors, menu bars, and taskbars. The
main window's monitor and saved geometry do not affect popup placement.

## Boundaries

The History repository remains the only owner of search, copy, plain-text copy,
pin, delete, and clear-unpinned operations. Flutter owns the shared themed UI
and settings state. Narrow native adapters own global shortcut registration,
transient-window lifecycle, focus restoration, permission checks, and input
synthesis.

Android keeps the same History actions and outcomes without desktop window or
global-shortcut integration.


## Manual secrecy

Both inspectors expose an Add spoiler / Remove spoiler action with a locked /
unlocked Lucide icon. Narrow layouts retain the icon, tooltip and semantic label. An explicit choice is persisted with a timestamp in the encrypted
payload metadata and travels with the existing item version to paired devices.
The newest explicit choice takes precedence over producer hints; ties prefer
secrecy. Recopying old content cannot undo that choice. Content identity and pin
state remain unchanged. Secret clips leave the full-text and semantic indexes;
non-secret text returns to full-text search and becomes eligible for semantic
indexing in the same storage transaction. A failed decrypt leaves the clip secret.
Receiving a secrecy change does not replace the system clipboard.

Spoiler masks use the content's actual layout, including text scaling and image
aspect ratio, without painting hidden content or exposing its focus, pointer
or accessibility targets. Mounted secret previews may load detail for this
layout; ordinary history queries continue to redact secret previews. Layout
content uses a bounded controller cache cleared when History suspends.

Temporary reveals are shared by clip ID across History, Quick Paste and both
inspectors through the local runtime. `GetSpoilerState` and
`SetSpoilerRevealed` return a monotonically revisioned snapshot of revealed IDs;
the existing item invalidation wakes each local client.
Reapplying an unchanged reveal/conceal state preserves the revision and emits
no item invalidation. Actual changes still notify all local clients. The snapshot contains
no content, is neither persisted nor sent to paired devices, and resets on
runtime restart. An explicit secrecy change conceals that clip. Reopening Quick
Paste or hiding/backgrounding a surface conceals the shared session; moving
focus between local windows does not. macOS and Windows use the daemon;
Android uses the same state contract in its embedded runtime.

The menu subtree and its recent/pinned scroll positions survive inspector
opening and closing. Its width remains stable during native expansion, and the
inspector enters with shared motion timing and reduced-motion support. Menu
footer rows use the same borderless style as clipboard rows.
