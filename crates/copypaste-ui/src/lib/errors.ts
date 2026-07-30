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
  internal: "errors.internal",
  unknown: "errors.unknown",
} as const satisfies Record<ErrorKind, string>;

export function friendlyError(kind: ErrorKind): string {
  return t(FRIENDLY[kind]);
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
