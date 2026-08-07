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
  saving: "Saving…",
  /** A destructive dialog's affirmative. Deliberately not "OK": the button
   *  should say what it does, so it can be read without the sentence above it
   *  (A11Y-3). */
  replace: "Replace",
  continue: "Continue",
  back: "Back",
  done: "Done",
  copy: "Copy",
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
  offline: "The clipboard service is not running.",
  not_ready:
    "The clipboard service is initializing. This will only take a moment.",
  protocol_mismatch:
    "CopyPaste and the clipboard service are on incompatible versions. Restart both to resolve.",
  not_found: "That item is no longer in your clipboard history.",
  invalid_request: "CopyPaste couldn't complete that action. Try again.",
  unavailable: "That action is unavailable.",
  auth_failed: "Sign in again, then check the account details and try once more.",
  key_locked:
    "CopyPaste couldn't reach this device's key store, so your history stayed locked.",
  key_unusable:
    "This device's encryption key can't be used, so its clipboard history can't be unlocked.",
  /** Pairing and sync. Each names the next step, because that is the whole
   *  difference between these and the one sentence they used to share. */
  pairing_code:
    "That pairing code isn't valid, or the other device didn't accept it. Get a fresh code and try again.",
  pairing_address:
    "That address isn't in the expected form. Use the host and port shown on the other device.",
  peer_unreachable:
    "That device can't be reached — it isn't visible on this network and isn't responding. Start the sync from the other device, or pair again with an address.",
  pairing_limit:
    "This device is already paired with as many devices as it can hold. Unpair one, then try again.",
  peer_failed: "Syncing with the other device didn't finish. Try again in a moment.",
  peer_version:
    "These two devices are running versions of CopyPaste that can't sync with each other. Update both to the latest version, then try again.",
  /** Says "device", where `not_found` says "item". The daemon used to answer a
   *  vanished device with `not_found`, so unpairing one reported a missing
   *  clipping (post-merge review, finding 4). */
  peer_not_found:
    "That device is no longer paired with this one. Refresh the device list to see what's still paired.",
  internal: "The clipboard service returned an error.",
  unknown: "The clipboard service returned an error.",
} as const;
