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

- macOS posts Command-V only while Accessibility trust is confirmed. When
  Quick Paste opens with auto-paste enabled and trust absent, it automatically
  requests Accessibility.
- Windows restores the previously focused window and sends Control-V through
  `SendInput`.

If trust is lost at runtime, denied, or the native paste operation fails,
selection still succeeds as copy-only and the popup closes. Auto-paste must not
make a history item unusable after it has already been copied successfully.

## Window placement

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
