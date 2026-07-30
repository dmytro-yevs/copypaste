export const common = {
  /** The dialog close button's only accessible name — the glyph is
   *  `aria-hidden`, so losing this leaves the control unnamed (A11Y-4). */
  close: "Close",
  cancel: "Cancel",
  tryAgain: "Try again",
  dismiss: "Dismiss",
  on: "On",
  off: "Off",
  unknown: "Unknown",
  noValue: "—",
} as const;

export const nav = {
  primary: "Primary",
  history: "History",
  devices: "Devices",
  settings: "Settings",
} as const;

/** Keyed by `ErrorKind`. These are *our* sentences for a classified failure —
 *  the daemon's own text never reaches a user (INV-12), so there is exactly one
 *  authored message per kind and nothing to double-wrap. */
export const errors = {
  offline: "The background service is not running.",
  not_ready:
    "The clipboard service is initializing. This will only take a moment.",
  protocol_mismatch:
    "CopyPaste and the background service are on incompatible versions. Restart both to resolve.",
  not_found: "That item is no longer in your clipboard history.",
  invalid_request: "The background service rejected that request.",
  unavailable: "This build of CopyPaste cannot do that yet.",
  internal: "The background service returned an error.",
  unknown: "The background service returned an error.",
} as const;
