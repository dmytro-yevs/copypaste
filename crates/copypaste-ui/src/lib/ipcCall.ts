/**
 * The only place `invoke` is called. Its own module so the surfaces split out
 * of `./ipc` for size share one gateway.
 *
 * A command the bridge does not route and an operation a build cannot perform
 * (`Unsupported` — Android has no pairing) both classify as `unavailable`:
 * "this build cannot" and "the service is down" are different things to be
 * told, and only one is worth retrying.
 */
import { invoke } from "@tauri-apps/api/core";

import { IpcFailure, classifyError } from "./errors";

/** No bridge is indistinguishable to a user from a service that is down, so it
 *  maps onto the same state rather than a third one. */
export function hasBridge(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!hasBridge()) throw new IpcFailure("offline");
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    // classifyError logs the raw value and returns a safe token (INV-12).
    throw new IpcFailure(classifyError(raw));
  }
}
