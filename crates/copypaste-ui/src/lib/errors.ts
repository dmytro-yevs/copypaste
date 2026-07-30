/**
 * Error classification and the code -> copy mapping.
 *
 * INV-12 (manifest 06 §2) and CLAUDE.md rule 4: **no raw error string may ever
 * be rendered.** The daemon's Unix socket path contains the local username, and
 * a transport error carrying it would leak that into the DOM, into screenshots
 * and into the accessibility tree. Raw errors are `console.error`'d and thrown
 * away; only a `ErrorKind` crosses into the view layer, and only
 * `friendlyError()` turns one into text.
 */

/** The bridge surfaces these; they mirror `copypaste_ipc::ErrorCode` plus the
 *  two transport conditions the daemon itself cannot report. */
export type ErrorKind =
  | "offline"
  | "not_ready"
  | "protocol_mismatch"
  | "not_found"
  | "invalid_request"
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
  [/not[_ ]ready|starting up|initiali[sz]ing/i, "not_ready"],
  [/protocol[_ ]mismatch|incompatible version/i, "protocol_mismatch"],
  [/not[_ ]found|no such item/i, "not_found"],
  [/invalid[_ ]request/i, "invalid_request"],
  [
    /connection refused|econnrefused|enoent|no such file|not running|socket|broken pipe|timed? out/i,
    "offline",
  ],
  [/internal/i, "internal"],
];

/**
 * Reduce anything thrown by the bridge to a safe kind. The raw value is logged
 * to the console (developer surface, not a rendered one) and never returned.
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

/** Vocabulary note (manifest 06 §3.2.5): "clipboard service" / "background
 *  service". Never "daemon". */
const FRIENDLY: Record<ErrorKind, string> = {
  offline: "The background service is not running.",
  not_ready:
    "The clipboard service is initialising. History will appear in a moment.",
  protocol_mismatch:
    "CopyPaste and the background service are on incompatible versions. Restart both to resolve.",
  not_found: "That item is no longer in your clipboard history.",
  invalid_request: "The background service rejected that request.",
  internal: "The background service returned an error.",
  unknown: "The background service returned an error.",
};

export function friendlyError(kind: ErrorKind): string {
  return FRIENDLY[kind];
}

/** Convenience for the toast paths: classify and map in one step. */
export function toFriendly(raw: unknown): string {
  return friendlyError(classifyError(raw));
}
