// ---------------------------------------------------------------------------
// lib/ipc/transport.ts — mock-IPC gate, IpcError, ipcCall, protocol version.
//
// Mock-IPC gate — activated by VITE_MOCK=1 (env) or ?mock=1 (URL query param).
// When active, all invoke() calls are handled by the in-process mockInvoke()
// fixture so the full UI renders in a plain browser with no Tauri runtime.
// The real (non-mock) path is COMPLETELY unchanged — this is a dead-code branch
// in production builds where VITE_MOCK is not "1".
//
// Mock gate (DEV-only): the ?mock=1 escape hatch is only honoured in
// development / test builds. In production (import.meta.env.DEV === false)
// the entire branch is dead code — Rollup/Vite tree-shakes mockIpc.ts out of
// the bundle entirely, so fixture data (developer email, fixture secrets) never
// ships to end-users.
// ---------------------------------------------------------------------------

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { IpcReply } from "./types";

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

// `invoke` and `MOCK` are resolved below via a DEV-gated block. Both branches
// always assign them, so TypeScript can prove they are initialised before use.
let invoke: InvokeFn;
let MOCK: boolean;

if (import.meta.env.DEV) {
  // DEV / test: honour VITE_MOCK=1 (build-time) or ?mock=1 (runtime URL).
  const mockRequested =
    (import.meta.env?.VITE_MOCK === "1") ||
    (typeof window !== "undefined" &&
      new URLSearchParams(window.location.search).has("mock"));

  if (mockRequested) {
    // Dynamic import keeps mockIpc.ts out of the production module graph.
    // Top-level await is valid here: "module": "ESNext" + "moduleResolution":
    // "bundler" enable top-level await in TypeScript ESM modules.
    // Vite statically replaces import.meta.env.DEV with `false` in prod, so
    // this entire `if` block — including the dynamic import string — is dead
    // code that Rollup eliminates before bundling.
    const { mockInvoke } = await import("../mockIpc");
    MOCK = true;
    invoke = (cmd, args) => mockInvoke(cmd, args) as Promise<never>;
  } else {
    MOCK = false;
    invoke = tauriInvoke;
  }
} else {
  // Production: always the real Tauri bridge. mockIpc.ts is never referenced.
  MOCK = false;
  invoke = tauriInvoke;
}

export { MOCK, invoke };

/**
 * IPC wire protocol version this UI build was compiled against (ADR-007).
 * Bump this when a breaking wire change is shipped alongside a UI update.
 * The daemon emits `protocol_version` on every response; when the daemon's
 * version exceeds this value the client should surface an upgrade prompt.
 */
export const CURRENT_PROTOCOL_VERSION = 1;

/**
 * Optional callback invoked when a daemon response carries a `protocol_version`
 * that differs from {@link CURRENT_PROTOCOL_VERSION}. The default handler
 * emits a `console.warn`. Replace this at startup (e.g. in App.tsx) to surface
 * a richer banner instead.
 *
 * Signature: `(daemonVersion: number) => void`
 */
export let protocolMismatchHandler: ((daemonVersion: number) => void) | null = null;

/**
 * Replace the module-level {@link protocolMismatchHandler}. Call once at
 * app startup (App.tsx) to wire in a UI banner instead of the default
 * `console.warn`. Pass `null` to restore the default warn-only behaviour.
 */
export function setProtocolMismatchHandler(
  handler: ((daemonVersion: number) => void) | null
): void {
  protocolMismatchHandler = handler;
}

/** Error carrying the daemon's stable machine code (e.g. "daemon_offline"). */
export class IpcError extends Error {
  code: string | null;
  constructor(message: string, code: string | null) {
    super(message);
    this.name = "IpcError";
    this.code = code;
  }
}

/**
 * ro0r: Exponential-backoff retry parameters for `migration_in_progress`.
 *
 * When the daemon replies with error_code "migration_in_progress" the DB
 * migration is briefly in flight and the request should be retried shortly.
 * We retry up to MAX_MIGRATION_RETRIES times with the backoff schedule below
 * (250 ms → 500 ms → 1000 ms → 2000 ms → 2000 ms …) before giving up and
 * re-throwing the original IpcError so the caller sees it.
 *
 * Only "migration_in_progress" is retried — all other error codes propagate
 * immediately. This is intentional: retrying arbitrary errors would mask bugs
 * and create unpredictable behaviour.
 */
const MAX_MIGRATION_RETRIES = 5;
const MIGRATION_BASE_DELAY_MS = 250;
const MIGRATION_MAX_DELAY_MS = 2000;

function migrationDelay(attempt: number): Promise<void> {
  // Exponential backoff: 250, 500, 1000, 2000, 2000, …
  const ms = Math.min(
    MIGRATION_BASE_DELAY_MS * Math.pow(2, attempt),
    MIGRATION_MAX_DELAY_MS
  );
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Call a daemon method over the Unix-socket bridge. Resolves to the daemon's
 * `data` payload on success; throws `IpcError` on a daemon error and on a
 * transport failure (e.g. the daemon being offline -> code "daemon_offline").
 *
 * Per ADR-007, checks the daemon's `protocol_version` on every reply. When the
 * daemon speaks a version higher than {@link CURRENT_PROTOCOL_VERSION} the
 * client may be unable to handle future field changes — a warning is surfaced
 * via {@link protocolMismatchHandler} (defaults to `console.warn`).
 *
 * ro0r: When the daemon replies with error_code "migration_in_progress", the
 * call is automatically retried with exponential backoff (up to 5 attempts,
 * 250 ms → 2 s cap) before propagating the error. No other error codes are
 * retried — only "migration_in_progress".
 */
export async function ipcCall<T = unknown>(
  method: string,
  params?: Record<string, unknown>
): Promise<T> {
  // ro0r: retry loop for migration_in_progress (only). All other errors fall
  // through immediately. `attempt` starts at 0; the loop runs until we either
  // succeed, hit an unretriable error, or exhaust MAX_MIGRATION_RETRIES.
  for (let attempt = 0; ; attempt++) {
    let reply: IpcReply;
    try {
      reply = await invoke<IpcReply>("ipc_call", { method, params: params ?? null });
    } catch (e) {
      // Transport-level failures come back as a string like "daemon_offline:/path".
      const raw = String(e);
      const code = raw.split(":", 1)[0] || null;
      throw new IpcError(raw, code);
    }

    // ADR-007: check protocol version on every reply. The field is optional
    // because (a) the Tauri bridge did not forward it before this fix and (b)
    // old daemon builds predate the field — both cases arrive as `undefined`,
    // which we treat as "no mismatch detected" rather than a false alarm.
    const daemonVersion = reply.protocol_version;
    if (daemonVersion !== undefined && daemonVersion !== CURRENT_PROTOCOL_VERSION) {
      const handler = protocolMismatchHandler;
      if (handler !== null) {
        handler(daemonVersion);
      } else {
        console.warn(
          `[copypaste] IPC protocol version mismatch: daemon speaks v${daemonVersion}, ` +
          `client expects v${CURRENT_PROTOCOL_VERSION}. ` +
          "Please upgrade the CopyPaste app or restart the daemon."
        );
      }
    }

    if (!reply.ok) {
      // Also fire protocolMismatchHandler on the daemon's explicit version-mismatch
      // error code ("n" / "version_mismatch"). This covers older daemons that reject
      // the request before emitting the protocol_version field in the reply.
      const code = reply.error_code ?? null;
      // ERR_CODE_VERSION_MISMATCH is the string "version_mismatch" (verified in
      // protocol.rs / copypaste-ipc error.rs). The earlier "n" alias was dead
      // code — "n" is the redacted-secret field marker, not an error code.
      if (code === "version_mismatch") {
        const handler = protocolMismatchHandler;
        if (handler !== null) {
          // Pass CURRENT_PROTOCOL_VERSION + 1 as a sentinel so the handler knows
          // the daemon is ahead (exact daemon version not available at error time).
          handler(CURRENT_PROTOCOL_VERSION + 1);
        } else {
          console.warn(
            "[copypaste] Daemon rejected request due to protocol version mismatch. " +
            "Please upgrade the CopyPaste app or restart the daemon."
          );
        }
      }

      // ro0r: retry only on migration_in_progress — a transient state where the
      // daemon's SQLite migration is briefly in flight. All other error codes
      // are not retried and propagate to the caller immediately.
      if (code === "migration_in_progress" && attempt < MAX_MIGRATION_RETRIES) {
        await migrationDelay(attempt);
        continue; // retry the request
      }

      throw new IpcError(reply.error ?? "unknown daemon error", code);
    }
    return reply.data as T;
  }
}
