/**
 * INV-12: no raw error string is ever rendered. Tauri returns a deliberately
 * small object and the view chooses localized copy from its stable code.
 * Unknown future codes keep their retry flag while falling back to generic
 * copy, so adding a backend code does not break an older UI.
 */
import type { ErrorCode, UiError } from "@/generated/ipc";
import { t } from "@/i18n";

/** Rust error codes plus boundary conditions owned by the desktop app. */
export type ErrorKind = ErrorCode
  | "offline"
  | "timeout"
  | "unavailable"
  | "update_busy"
  | "update_unconfigured"
  | "update_unsupported"
  | "update_signature_invalid"
  | "update_network_failed"
  | "update_check_failed"
  | "update_install_failed"
  | "unknown";

const FRIENDLY = {
  offline: "errors.offline",
  timeout: "errors.timeout",
  not_ready: "errors.not_ready",
  protocol_mismatch: "errors.protocol_mismatch",
  not_found: "errors.not_found",
  invalid_request: "errors.invalid_request",
  unavailable: "errors.unavailable",
  auth_failed: "errors.auth_failed",
  key_locked: "errors.key_locked",
  key_unusable: "errors.key_unusable",
  pairing_code: "errors.pairing_code",
  pairing_address: "errors.pairing_address",
  rate_limited: "errors.rate_limited",
  peer_unreachable: "errors.peer_unreachable",
  pairing_limit: "errors.pairing_limit",
  peer_failed: "errors.peer_failed",
  peer_version: "errors.peer_version",
  peer_not_found: "errors.peer_not_found",
  update_busy: "errors.update_busy",
  update_unconfigured: "errors.update_unconfigured",
  update_unsupported: "errors.update_unsupported",
  update_signature_invalid: "errors.update_signature_invalid",
  update_network_failed: "errors.update_network_failed",
  update_check_failed: "errors.update_check_failed",
  update_install_failed: "errors.update_install_failed",
  internal: "errors.internal",
  unknown: "errors.unknown",
} as const satisfies Record<ErrorKind, string>;

function safeCode(code: string): string {
  return code.length > 0 &&
    code.length <= 64 &&
    /^[a-z0-9_]+$/.test(code)
    ? code
    : "unknown";
}

function errorKind(code: string): ErrorKind {
  // Not `Object.hasOwn`, which the Chromium 74 WebView baseline does not have
  // (`scripts/check-webview-baseline.mjs`).
  return Object.prototype.hasOwnProperty.call(FRIENDLY, code)
    ? (code as ErrorKind)
    : "unknown";
}

function isUiError(raw: unknown): raw is UiError {
  return (
    typeof raw === "object" &&
    raw !== null &&
    "code" in raw &&
    typeof raw.code === "string" &&
    "retryable" in raw &&
    typeof raw.retryable === "boolean"
  );
}

export class IpcFailure extends Error {
  /** Preserves a safe future code for telemetry/debugging. */
  readonly code: string;
  /** The localized-copy key; unknown future codes deliberately collapse. */
  readonly kind: ErrorKind;
  readonly retryable: boolean;

  constructor(code: string, retryable: boolean) {
    const safe = safeCode(code);
    const kind = errorKind(safe);
    // Never retain the backend's text: even an accidental render of
    // `error.message` can expose only this fixed token.
    super(kind);
    this.name = "IpcFailure";
    this.code = safe;
    this.kind = kind;
    this.retryable = retryable;
  }
}

/** Convert the Tauri rejection into the only error shape used by the UI. */
export function ipcFailure(raw: unknown): IpcFailure {
  if (raw instanceof IpcFailure) return raw;

  if (isUiError(raw)) {
    const failure = new IpcFailure(raw.code, raw.retryable);
    console.error("[copypaste] IPC failure", {
      code: failure.code,
      retryable: failure.retryable,
    });
    return failure;
  }

  // The rejected value may come from an older bridge and contain a socket or
  // database path. Record only that the contract was malformed, never the
  // value itself.
  console.error("[copypaste] malformed IPC failure");

  // A malformed rejection has no trustworthy retry policy. Keeping the
  // historical unknown retry action is safe and does not reconstruct a code.
  return new IpcFailure("unknown", true);
}

export function classifyError(raw: unknown): ErrorKind {
  return ipcFailure(raw).kind;
}

export function friendlyError(kind: ErrorKind): string {
  return t(FRIENDLY[kind]);
}

/** Retryability is supplied by Rust; TypeScript has no parallel policy map. */
export function isRetryable(raw: unknown): boolean {
  return ipcFailure(raw).retryable;
}

export function toFriendly(raw: unknown): string {
  return friendlyError(classifyError(raw));
}

/** True when this build does not route or implement the requested command. */
export function isUnavailable(raw: unknown): boolean {
  return raw !== null && raw !== undefined && classifyError(raw) === "unavailable";
}
