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

/**
 * Browser previews have no Tauri bridge. A local adapter is available only
 * from `npm run dev:web:daemon`; both values are injected by that script and
 * Vite erases this branch from a production build.
 */
function webBridge(): WebBridgeConfig | null {
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
): Promise<T> {
  const bridge = webBridge();
  if (bridge !== null) {
    let response: Response;
    try {
      response = await fetch(`${bridge.url}/v1/call`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${bridge.token}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ command, args: args ?? {} }),
      });
    } catch {
      throw new IpcFailure("offline", true);
    }

    const body: unknown = await response.json().catch(() => null);
    if (!response.ok) throw ipcFailure(body);
    return body as T;
  }

  if (!hasBridge()) throw new IpcFailure("offline", true);
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    throw ipcFailure(raw);
  }
}
