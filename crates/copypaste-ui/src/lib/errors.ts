/**
 * INV-12: no raw error string is ever rendered. The socket path contains the
 * local username, so a transport error carrying it would leak that into the
 * DOM, screenshots and the accessibility tree. The raw value is logged and
 * dropped; only an `ErrorKind` crosses into the view layer.
 *
 * Vocabulary is "clipboard service", never "daemon" (bdac.34/36).
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
  /** The key store could not be read. Worth retrying, which is what lets
   *  `key_unusable` say that retrying is pointless. */
  | "key_locked"
  /** The device key is there and cannot be used, so the history it protects is
   *  not recoverable by anything. */
  | "key_unusable"
  /** `copypaste_p2p::NodeError`, grouped by what the user does next rather than
   *  one kind per variant. All eight used to classify as `unknown`, which made
   *  a typo, a switched-off device and a full pairing list indistinguishable
   *  (finding 4). */
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
  // like "database": these three must not fall through to a generic kind. Both
  // the daemon's sentences and the in-process backend's wrapping of
  // `StoreError` / `CryptoError` are covered, because the app can meet either.
  [/copypaste 0\.4 history|legacy[_ ]database/i, "legacy_database"],
  [
    /key[_ ]unusable|device secret unusable|key is present and cannot be used/i,
    "key_unusable",
  ],
  [/key[_ ]locked|key ?store unavailable|key store could not be read/i, "key_locked"],
  // Matching sentences this app does not author is the second-best mechanism,
  // and is how these eight drifted out of the list once already. A Rust test on
  // each side pins them; the drift ends for good only when the *code* crosses
  // the Tauri boundary, which needs a payload `commands/` does not send yet.
  //
  // `peer_not_found` goes before the pairing patterns and matches "device"
  // rather than "found": it used to classify as `not_found` and render a
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
  [/not[_ ]found|no such item|no longer there|no longer in your/i, "not_found"],
  [/invalid[_ ]request/i, "invalid_request"],
  [
    /connection refused|econnrefused|enoent|no such file|isn't running|not running|socket|\.sock\b|broken pipe|timed? out/i,
    "offline",
  ],
  [/internal/i, "internal"],
];

/** The raw value is logged to the console — a developer surface, not a rendered
 *  one — and never returned. */
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
 * of `copypaste_ipc::ErrorCode::retryable`. A **Try again** button in front of
 * a condition that can never change is the defect this prevents, and the reason
 * it is a total record: adding an `ErrorKind` does not compile until someone
 * has decided which of the two it is.
 *
 * `false` where the next step is a human action — a fresh code, a corrected
 * address, a downgrade, refreshing a stale device list — rather than a repeat
 * of the same request.
 */
const RETRYABLE = {
  offline: true,
  not_ready: true,
  key_locked: true,
  internal: true,
  unknown: true,
  peer_unreachable: true,
  peer_failed: true,
  protocol_mismatch: false,
  not_found: false,
  invalid_request: false,
  unavailable: false,
  legacy_database: false,
  key_unusable: false,
  pairing_code: false,
  pairing_address: false,
  pairing_limit: false,
  peer_not_found: false,
} as const satisfies Record<ErrorKind, boolean>;

export function isRetryable(kind: ErrorKind): boolean {
  return RETRYABLE[kind];
}

export function toFriendly(raw: unknown): string {
  return friendlyError(classifyError(raw));
}

/** True when the failure means "the bridge does not route this command yet",
 *  which is a different screen from "the service is down". */
export function isUnavailable(raw: unknown): boolean {
  return raw !== null && raw !== undefined && classifyError(raw) === "unavailable";
}
