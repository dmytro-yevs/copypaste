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
const LOCAL_BRIDGE_ORIGIN = "http://localhost:1420";
const BRIDGE_START_TIMEOUT_MS = 120_000;
const BRIDGE_RELOAD_INTERVAL_MS = 250;

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
 * Browser previews have no Tauri bridge. The local adapter publishes its
 * current loopback address and one-run credential through the runtime script.
 */
function webBridge(): WebBridgeConfig | null {
  if (hasNativeBridge() || !import.meta.env.DEV) return null;
  const runtime = window.__COPYPASTE_WEB_BRIDGE__;
  return validWebBridge(runtime) ? runtime : null;
}

function validWebBridge(value: unknown): value is WebBridgeConfig {
  if (
    typeof value !== "object" ||
    value === null ||
    !("url" in value) ||
    !("token" in value) ||
    typeof value.url !== "string" ||
    typeof value.token !== "string" ||
    !/^[a-f0-9]{32}$/.test(value.token)
  ) {
    return false;
  }
  try {
    const url = new URL(value.url);
    return (
      url.protocol === "http:" &&
      url.hostname === "127.0.0.1" &&
      url.username === "" &&
      url.password === "" &&
      url.pathname === "/" &&
      url.search === "" &&
      url.hash === ""
    );
  } catch {
    return false;
  }
}

function expectsLocalWebBridge(): boolean {
  return import.meta.env.DEV &&
    !("__TAURI_INTERNALS__" in window) &&
    window.location.origin === LOCAL_BRIDGE_ORIGIN;
}

async function bridgeIsHealthy(bridge: WebBridgeConfig): Promise<boolean> {
  const controller = new AbortController();
  const timer = globalThis.setTimeout(() => controller.abort(), 1_000);
  try {
    const response = await fetch(`${bridge.url}/health`, {
      method: "GET",
      headers: { Authorization: `Bearer ${bridge.token}` },
      cache: "no-store",
      signal: controller.signal,
    });
    return response.status === 204;
  } catch {
    return false;
  } finally {
    globalThis.clearTimeout(timer);
  }
}

function reloadBridgeRuntime(): Promise<void> {
  return new Promise((resolve) => {
    const script = document.createElement("script");
    script.src = `/copypaste-web-bridge.js?runtime=${Date.now()}`;
    script.async = true;
    script.onload = () => {
      script.remove();
      resolve();
    };
    script.onerror = () => {
      script.remove();
      resolve();
    };
    document.head.append(script);
  });
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => globalThis.setTimeout(resolve, ms));
}

let bridgeResolution: Promise<WebBridgeConfig | null> | null = null;

async function resolveWebBridge(): Promise<WebBridgeConfig | null> {
  if (bridgeResolution !== null) return bridgeResolution;
  bridgeResolution = (async () => {
    const current = webBridge();
    if (current !== null) {
      return await bridgeIsHealthy(current) ? current : null;
    }
    if (!expectsLocalWebBridge()) return null;

    const deadline = Date.now() + BRIDGE_START_TIMEOUT_MS;
    while (Date.now() < deadline) {
      await reloadBridgeRuntime();
      const refreshed = webBridge();
      if (refreshed !== null && await bridgeIsHealthy(refreshed)) return refreshed;
      await delay(BRIDGE_RELOAD_INTERVAL_MS);
    }
    return null;
  })();
  try {
    return await bridgeResolution;
  } finally {
    bridgeResolution = null;
  }
}

/** True for the configured adapter or the localhost preview waiting for it. */
export function hasWebBridge(): boolean {
  return webBridge() !== null || expectsLocalWebBridge();
}

/** True only when Tauri's native command surface is available. */
export function hasNativeBridge(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** No bridge is indistinguishable to a user from a service that is down, so it
 *  maps onto the same state rather than a third one. */
export function hasBridge(): boolean {
  return hasNativeBridge() ||
    (typeof window !== "undefined" && hasWebBridge());
}

export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
  options: IpcCallOptions = {},
): Promise<T> {
  if (import.meta.env.DEV && !hasNativeBridge()) {
    const { interceptPreviewCall } = await import("@/service/previewIpc");
    const preview = await interceptPreviewCall(command, args);
    if (preview.handled) return preview.value as T;
  }
  const bridge = await resolveWebBridge();
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

  if (!hasNativeBridge()) {
    throw new IpcFailure("offline", true);
  }
  try {
    // Tauri exposes no AbortSignal contract. Only this caller-facing promise
    // stops; the native command is allowed to finish and is safely ignored.
    return await bounded(() => invoke<T>(command, args), options, false);
  } catch (raw) {
    throw ipcFailure(raw);
  }
}
