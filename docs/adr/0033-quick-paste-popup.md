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
The popup engine initializes the installed modules and owns its OCR controller.
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

Reference: [Maccy preview controller](https://github.com/p0deje/Maccy/blob/a92c11ae3e7a86a57dc6359bad59e330e4625ede/Maccy/Observables/SlideoutController.swift)
and [default delay](https://github.com/p0deje/Maccy/blob/a92c11ae3e7a86a57dc6359bad59e330e4625ede/Maccy/Extensions/Defaults.Keys%2BNames.swift).

Each invocation reads the current pointer position, selects the monitor that
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

Both inspectors expose one icon-only eye control to make a clip secret or
non-secret. An explicit choice is persisted with a timestamp in the encrypted
payload metadata and travels with the existing item version to paired devices.
The newest explicit choice takes precedence over producer hints; ties prefer
secrecy. Recopying old content cannot undo that choice. Content identity and pin
state remain unchanged. Secret clips leave the full-text and semantic indexes;
non-secret text returns to full-text search and becomes eligible for semantic
indexing in the same storage transaction. A failed decrypt leaves the clip secret.
Receiving a secrecy change does not replace the system clipboard.

Quick Paste's inline spoiler matches one line of its inherited typography,
including text scaling. Revealing it does not add button padding or change the
menu row height.
