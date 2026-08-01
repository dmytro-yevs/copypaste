/**
 * The menu-bar item's strings.
 *
 * They are rendered by Rust (`src-tauri/src/shell/tray/`), which cannot import
 * this file; `shell::tray::text` holds the same constants and a test there
 * reads this file and fails if the two disagree. The catalogue is the authority
 * either way, so a tray label is subject to the same rules as every other
 * string — including that none of them may carry a path (INV-12).
 *
 * Nothing here interpolates: a recent clipping's text becomes a menu label
 * directly, never an argument to a template.
 */
export const tray = {
  show: "Show CopyPaste",
  openAtLogin: "Open at Login",
  privateMode: "Private Mode",
  quit: "Quit CopyPaste",

  recent: "Recent clippings",
  recentEmpty: "Nothing copied yet",
  recentOffline: "Can't reach the background service",

  statusCapturing: "Capturing what you copy",
  statusStopped: "Not capturing",
  statusOffline: "Background service not running",
} as const;
