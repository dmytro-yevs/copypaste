/**
 * The only place `invoke` is called.
 *
 * A command the bridge does not route and an operation a build cannot perform
 * (`Unsupported` — no backend reorders pinned items) both classify as
 * `unavailable`: "this build cannot" and "the service is down" are different
 * things to be told, and only one is worth retrying.
 */
import { invoke } from "@tauri-apps/api/core";

import { IpcFailure, ipcFailure } from "./errors";

interface WebBridgeConfig {
  readonly url: string;
  readonly token: string;
}

declare global {
  interface Window {
    __COPYPASTE_WEB_BRIDGE__?: WebBridgeConfig | null;
  }
}

export interface IpcCallOptions {
  readonly signal?: AbortSignal;
  readonly timeoutMs?: number;
}

const DEFAULT_TIMEOUT_MS = 5 * 60_000;

type BoundaryFailure = "cancelled" | "timeout";

function boundaryFailure(code: BoundaryFailure): IpcFailure {
  return new IpcFailure(code, code === "timeout");
}

function bounded<T>(
  start: (signal: AbortSignal | undefined) => Promise<T>,
  options: IpcCallOptions,
  abortUnderlying: boolean,
): Promise<T> {
  const callerSignal = options.signal;
  if (callerSignal?.aborted) {
    return Promise.reject(boundaryFailure("cancelled"));
  }

  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  if (!Number.isFinite(timeoutMs) || timeoutMs < 0) {
    return Promise.reject(new IpcFailure("invalid_request", false));
  }

  return new Promise<T>((resolve, reject) => {
    const controller = abortUnderlying ? new AbortController() : null;
    let settled = false;
    let timer: ReturnType<typeof globalThis.setTimeout> | undefined;

    const cleanup = () => {
      if (timer !== undefined) globalThis.clearTimeout(timer);
      callerSignal?.removeEventListener("abort", onCancel);
    };
    const finish = (complete: () => void) => {
      if (settled) return;
      settled = true;
      cleanup();
      complete();
    };
    const stop = (code: BoundaryFailure) => {
      finish(() => {
        controller?.abort();
        reject(boundaryFailure(code));
      });
    };
    const onCancel = () => stop("cancelled");

    callerSignal?.addEventListener("abort", onCancel, { once: true });
    timer = globalThis.setTimeout(() => stop("timeout"), timeoutMs);

    let pending: Promise<T>;
    try {
      pending = start(controller?.signal);
    } catch (raw) {
      finish(() => reject(raw));
      return;
    }

    pending.then(
      (value) => finish(() => resolve(value)),
      (raw) => finish(() => reject(raw)),
    );
  });
}

/**
 * Browser previews have no Tauri bridge. A local adapter is available only
 * from `npm run dev:web:daemon`; both values are either loaded at page time
 * from `/copypaste-web-bridge.js` or injected before Vite starts.
 */
function webBridge(): WebBridgeConfig | null {
  const runtime = window.__COPYPASTE_WEB_BRIDGE__;
  if (
    typeof runtime?.url === "string" &&
    typeof runtime.token === "string" &&
    runtime.url.length > 0 &&
    runtime.token.length > 0
  ) {
    return runtime;
  }
  if (!import.meta.env.DEV) return null;
  const url = import.meta.env.VITE_COPYPASTE_WEB_BRIDGE_URL;
  const token = import.meta.env.VITE_COPYPASTE_WEB_BRIDGE_TOKEN;
  return url && token ? { url, token } : null;
}

/** True only for the explicitly configured local browser-preview adapter. */
export function hasWebBridge(): boolean {
  return webBridge() !== null;
}

/** No bridge is indistinguishable to a user from a service that is down, so it
 *  maps onto the same state rather than a third one. */
export function hasBridge(): boolean {
  return typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || webBridge() !== null);
}

export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
  options: IpcCallOptions = {},
): Promise<T> {
  const bridge = webBridge();
  if (bridge !== null) {
    try {
      return await bounded(async (signal) => {
        const response = await fetch(`${bridge.url}/v1/call`, {
          method: "POST",
          headers: {
            Authorization: `Bearer ${bridge.token}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({ command, args: args ?? {} }),
          signal,
        });

        const body: unknown = await response.json().catch(() => null);
        if (!response.ok) throw ipcFailure(body);
        return body as T;
      }, options, true);
    } catch (raw) {
      if (raw instanceof IpcFailure) throw raw;
      throw new IpcFailure("offline", true);
    }
  }

  if (!hasBridge()) throw new IpcFailure("offline", true);
  try {
    // Tauri exposes no AbortSignal contract. Only this caller-facing promise
    // stops; the native command is allowed to finish and is safely ignored.
    return await bounded(() => invoke<T>(command, args), options, false);
  } catch (raw) {
    throw ipcFailure(raw);
  }
}
