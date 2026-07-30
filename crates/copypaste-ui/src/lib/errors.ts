/**
 * INV-12 and CLAUDE.md rule 4: **no raw error string is ever rendered.** The
 * socket path contains the local username, so a transport error carrying it
 * would leak that into the DOM, screenshots and the accessibility tree. The raw
 * value is logged and dropped; only an `ErrorKind` crosses into the view layer,
 * and the sentence the user reads comes from the catalogue.
 *
 * Vocabulary is "clipboard service" / "background service", never "daemon"
 * (bdac.34/36), American spelling.
 */
import { t } from "@/i18n";

/** `copypaste_ipc::ErrorCode`, plus the conditions the daemon cannot report
 *  about itself. */
export type ErrorKind =
  | "offline"
  | "not_ready"
  | "protocol_mismatch"
  | "not_found"
  | "invalid_request"
  | "unavailable"
  /** A CopyPaste 0.4 history, identified rather than guessed at. The old data
   *  is intact on disk — see `copypaste_ipc::ErrorCode::LegacyDatabase`. */
  | "legacy_database"
  /** The key store could not be read. Worth retrying, and the only reason
   *  `key_unusable` can afford to say that retrying is pointless. */
  | "key_locked"
  /** The device key is there and cannot be used, so the history it protects is
   *  not recoverable by anything. */
  | "key_unusable"
  /**
   * Pairing and sync failures, from `copypaste_p2p::NodeError`.
   *
   * Grouped by what the user does next rather than one per variant: a bad code
   * and a rejected handshake are the same next step, and so are a peer with no
   * address and one that stopped answering. All eight used to classify as
   * `unknown` and render "The background service returned an error", which made
   * a typo, a switched-off device and a full pairing list indistinguishable
   * (post-merge review, finding 4).
   */
  | "pairing_code"
  | "pairing_address"
  | "peer_unreachable"
  | "pairing_limit"
  | "peer_failed"
  /** A device this list no longer holds. Its own kind because `not_found` is
   *  worded for a clipboard item, and answering an unpairing with it told the
   *  user a clipping had gone (`copypaste_ipc::ErrorCode::PeerNotFound`). */
  | "peer_not_found"
  | "internal"
  | "unknown";

export class IpcFailure extends Error {
  readonly kind: ErrorKind;

  constructor(kind: ErrorKind) {
    // The message is the kind token, never the raw text: even an accidental
    // render of `error.message` cannot disclose a path.
    super(kind);
    this.name = "IpcFailure";
    this.kind = kind;
  }
}

/** Patterns are matched against the *raw* text, which is then discarded. */
const PATTERNS: ReadonlyArray<readonly [RegExp, ErrorKind]> = [
  // First, and matched on their own vocabulary rather than on a shared word
  // like "database": these three are the states that must not be rendered as
  // one of the generic ones below. Both the daemon's fixed sentences and the
  // in-process backend's wrapping of `StoreError` / `CryptoError` are covered,
  // because the app can meet either.
  [/copypaste 0\.4 history|legacy[_ ]database/i, "legacy_database"],
  [
    /key[_ ]unusable|device secret unusable|key is present and cannot be used/i,
    "key_unusable",
  ],
  [/key[_ ]locked|key ?store unavailable|key store could not be read/i, "key_locked"],
  // `NodeError`, whose sentences the daemon passes through verbatim and
  // `BackendError::from_code` keeps. Matched here rather than falling through to
  // `unknown`, which is what made a mistyped code, a switched-off device and a
  // full pairing list read identically.
  //
  // **This is the second-best mechanism and it is worth saying so.** Matching
  // text this app does not author is how these eight drifted out of the list in
  // the first place, when the node's vocabulary moved into `copypaste-p2p`.
  // `copypaste_ipc::ErrorCode` now carries one code per condition and
  // `BackendError::from_code` passes the sentence through unchanged, so these
  // patterns are pinned by a Rust test on each side; what would end the drift
  // for good is the *code* crossing the Tauri boundary, which needs a payload
  // `commands/` does not send yet.
  //
  // Before the pairing patterns, and matched on "device" rather than on
  // "found": this is the one that used to classify as `not_found` and render a
  // sentence about the clipboard.
  [/no such paired device|peer[_ ]not[_ ]found/i, "peer_not_found"],
  [/pairing code is not valid|did not accept this pairing code/i, "pairing_code"],
  [/address could not be resolved|expected host:port/i, "pairing_address"],
  [
    /never been reached|not visible on the network|stopped responding/i,
    "peer_unreachable",
  ],
  [/as many devices as it can hold|too[_ ]many[_ ]pairings/i, "pairing_limit"],
  [
    /sync session with the other device|paired-device list could not be updated/i,
    "peer_failed",
  ],
  // Ordered: a Tauri "command not found" also mentions the word "command",
  // so the unavailable test has to run before the generic ones.
  [
    /not allowed|command .* not found|unknown command|unimplemented|not implemented|not available in this build|unsupported/i,
    "unavailable",
  ],
  [/not[_ ]ready|starting up|initiali[sz]ing/i, "not_ready"],
  [/protocol[_ ]mismatch|incompatible version|different versions/i, "protocol_mismatch"],
  // "That item is no longer there." is the bridge's own NotFound sentence; the
  // patterns are matched against text this app does not author, so they track
  // what `BackendError` actually renders.
  [/not[_ ]found|no such item|no longer there|no longer in your/i, "not_found"],
  [/invalid[_ ]request/i, "invalid_request"],
  [
    /connection refused|econnrefused|enoent|no such file|isn't running|not running|socket|\.sock\b|broken pipe|timed? out/i,
    "offline",
  ],
  [/internal/i, "internal"],
];

/**
 * Reduce anything thrown by the bridge to a safe kind. The raw value is logged
 * to the console (a developer surface, not a rendered one) and never returned.
 */
export function classifyError(raw: unknown): ErrorKind {
  if (raw instanceof IpcFailure) return raw.kind;

  console.error("[copypaste] IPC failure", raw);

  const text =
    typeof raw === "string"
      ? raw
      : raw instanceof Error
        ? raw.message
        : typeof raw === "object" && raw !== null && "message" in raw
          ? String((raw as { message: unknown }).message)
          : "";

  for (const [pattern, kind] of PATTERNS) {
    if (pattern.test(text)) return kind;
  }
  return "unknown";
}

const FRIENDLY = {
  offline: "errors.offline",
  not_ready: "errors.not_ready",
  protocol_mismatch: "errors.protocol_mismatch",
  not_found: "errors.not_found",
  invalid_request: "errors.invalid_request",
  unavailable: "errors.unavailable",
  legacy_database: "errors.legacy_database",
  key_locked: "errors.key_locked",
  key_unusable: "errors.key_unusable",
  pairing_code: "errors.pairing_code",
  pairing_address: "errors.pairing_address",
  peer_unreachable: "errors.peer_unreachable",
  pairing_limit: "errors.pairing_limit",
  peer_failed: "errors.peer_failed",
  peer_not_found: "errors.peer_not_found",
  internal: "errors.internal",
  unknown: "errors.unknown",
} as const satisfies Record<ErrorKind, string>;

export function friendlyError(kind: ErrorKind): string {
  return t(FRIENDLY[kind]);
}

/**
 * Whether asking again could plausibly answer differently — the frontend half
 * of `copypaste_ipc::ErrorCode::retryable`.
 *
 * A **Try again** button in front of a condition that can never change is the
 * defect this exists to prevent, and the reason it is a total record rather
 * than a list of exceptions: adding an `ErrorKind` does not compile until
 * someone has decided which of the two it is.
 */
const RETRYABLE = {
  offline: true,
  not_ready: true,
  key_locked: true,
  internal: true,
  unknown: true,
  // The device may come back; the run may go through next time.
  peer_unreachable: true,
  peer_failed: true,
  // A downgrade, a sign-in, a different build — all human actions, none of
  // them a repeat of the same request.
  protocol_mismatch: false,
  not_found: false,
  invalid_request: false,
  unavailable: false,
  legacy_database: false,
  key_unusable: false,
  // A fresh code, a corrected address, an unpairing: each is something the
  // user has to do before the same request could go differently.
  pairing_code: false,
  pairing_address: false,
  pairing_limit: false,
  // The list was stale, not the device. Refreshing it is the next step, and
  // repeating the same request against the same id is not.
  peer_not_found: false,
} as const satisfies Record<ErrorKind, boolean>;

export function isRetryable(kind: ErrorKind): boolean {
  return RETRYABLE[kind];
}

/** Convenience for the toast paths: classify and map in one step. */
export function toFriendly(raw: unknown): string {
  return friendlyError(classifyError(raw));
}

/** True when the failure means "the bridge does not route this command yet",
 *  which is a different screen from "the service is down". */
export function isUnavailable(raw: unknown): boolean {
  return raw !== null && raw !== undefined && classifyError(raw) === "unavailable";
}
