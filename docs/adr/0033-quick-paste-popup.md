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
