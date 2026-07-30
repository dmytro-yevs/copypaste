/**
 * The Tauri bridge, and the only place `invoke` is called.
 *
 * Shapes mirror `crates/copypaste-ipc/src/lib.rs` exactly — the Rust crate is
 * the single model of the wire contract, and these declarations are its
 * TypeScript view. Field names are the Rust field names (snake_case) because
 * that is what serde emits.
 */
import { invoke } from "@tauri-apps/api/core";

import { IpcFailure, classifyError } from "./errors";

/** `copypaste_ipc::Item`. `content` is plaintext: the daemon decrypts on the
 *  way out and the socket is 0600. */
export interface Item {
  readonly id: string;
  readonly content: string;
  readonly content_type: string;
  /** Milliseconds since the Unix epoch. */
  readonly created_at: number;
  readonly pinned: boolean;
  /** The detector matched. Such items are never in the search index, so they
   *  cannot come back from `search`. */
  readonly is_sensitive: boolean;
}

/** `copypaste_ipc::StatusData`. */
export interface StatusData {
  readonly version: string;
  readonly protocol_version: number;
  readonly item_count: number;
  readonly capture_running: boolean;
  /** The real pasteboard, or the fake used on non-macOS hosts and in tests.
   *  Surfaced in the status line so a demo cannot be mistaken for the real
   *  thing. */
  readonly clipboard_backend: string;
}

/**
 * Outside a Tauri webview there is no bridge at all. That is indistinguishable
 * to the user from the background service being down, so it maps onto the same
 * state rather than throwing something the view layer has to special-case.
 */
function hasBridge(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!hasBridge()) throw new IpcFailure("offline");
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    // classifyError logs the raw value and returns a safe token (INV-12).
    throw new IpcFailure(classifyError(raw));
  }
}

export function listItems(limit: number, offset: number): Promise<Item[]> {
  return call<Item[]>("list", { limit, offset });
}

export function searchItems(query: string, limit: number): Promise<Item[]> {
  return call<Item[]>("search", { query, limit });
}

export function copyItem(id: string): Promise<void> {
  return call<void>("copy_item", { id });
}

export function addItem(content: string): Promise<Item> {
  return call<Item>("add_item", { content });
}

export function deleteItem(id: string): Promise<void> {
  return call<void>("delete_item", { id });
}

export function setPinned(id: string, pinned: boolean): Promise<void> {
  return call<void>("set_pinned", { id, pinned });
}

export function getStatus(): Promise<StatusData> {
  return call<StatusData>("status");
}
