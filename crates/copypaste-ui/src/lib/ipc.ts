/**
 * The Tauri bridge, and the only place `invoke` is called. Command names track
 * `copypaste_ipc::Method`, so this file and `src-tauri/src/commands/` name the
 * same things; field names are snake_case because that is what serde emits.
 *
 * A command the bridge does not route, and an operation a build cannot perform
 * (`BackendError::Unsupported` — Android has no pairing), both classify as the
 * `unavailable` kind. Screens render that as its own state: "this build cannot"
 * and "the service is down" are different things to be told, and only one of
 * them is worth retrying.
 */
import { invoke } from "@tauri-apps/api/core";

import { IpcFailure, classifyError } from "./errors";

/**
 * **`content` is `null` for a sensitive item** — not an empty string, not a
 * mask. The bridge drops the plaintext before serialising, so INV-10 is
 * enforced by the type rather than by each component remembering to hide
 * something. `revealItem` is the one route back.
 */
export interface Item {
  readonly id: string;
  readonly content: string | null;
  readonly content_type: string;
  /** Milliseconds since the Unix epoch. */
  readonly created_at: number;
  readonly pinned: boolean;
  /** Never indexed, so `search` cannot return one. */
  readonly is_sensitive: boolean;
}


export interface StatusData {
  readonly version: string;
  readonly protocol_version: number;
  readonly item_count: number;
  readonly capture_running: boolean;
  /** Surfaced so a fake backend cannot be mistaken for the real pasteboard. */
  readonly clipboard_backend: string;
}

/**
 * Returned once by `pair_create` and never retrievable again. `code` is the
 * Noise pre-shared key in transferable form: anyone holding it can pair, so it
 * is hidden until revealed, never logged, and never toasted.
 */
export interface PairingData {
  readonly code: string;
  readonly pairing_id: string;
  readonly listen_addr: string | null;
}


export interface PeerInfo {
  readonly pairing_id: string;
  readonly name: string;
  readonly last_addr: string | null;
  readonly last_seen_ms: number;
  /** Discovery is a convenience: `false` means "not seen", never
   *  "unreachable". */
  readonly online: boolean;
}


export interface SyncResult {
  readonly pairing_id: string;
  readonly name: string;
  readonly sent: number;
  readonly received: number;
  /** Present when this peer failed; the rest of the run still reports. */
  readonly error: string | null;
}

/** The IPC protocol version this build speaks. A daemon reporting anything
 *  else raises the mismatch banner (INV-17) rather than degrading silently. */
export const CURRENT_PROTOCOL_VERSION = 1;

/** No bridge is indistinguishable to a user from a service that is down, so it
 *  maps onto the same state rather than a third one. */
export function hasBridge(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(
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

/* --------------------------------------------------------------- history --- */

export function listItems(limit: number, offset: number): Promise<Item[]> {
  return call<Item[]>("list", { limit, offset });
}

export function searchItems(query: string, limit: number): Promise<Item[]> {
  return call<Item[]>("search", { query, limit });
}

export function copyItem(id: string): Promise<Item> {
  return call<Item>("copy_item", { id });
}

export function addItem(content: string): Promise<Item> {
  return call<Item>("add_item", { content });
}

/**
 * One item's plaintext, on demand. The result is held in component state and
 * dropped when the reveal expires (INV-11) — never in the query cache, which
 * outlives the row and would restore it on the next render.
 */
export function revealItem(id: string): Promise<string> {
  return call<string>("reveal_item", { id });
}

export function deleteItem(id: string): Promise<boolean> {
  return call<boolean>("delete_item", { id });
}

/** Every unpinned item; pinned ones survive, as with `copypaste clear`. */
export function deleteAll(): Promise<number> {
  return call<number>("delete_all");
}

export function setPinned(id: string, pinned: boolean): Promise<Item> {
  return call<Item>("set_pinned", { id, pinned });
}

export function getStatus(): Promise<StatusData> {
  return call<StatusData>("status");
}

/* --------------------------------------------------------------- devices --- */

export function listPeers(): Promise<PeerInfo[]> {
  return call<PeerInfo[]>("peers");
}

export function pairCreate(name: string): Promise<PairingData> {
  return call<PairingData>("pair_create", { name });
}

/** Returns the peer list after pairing, so the screen needs no re-list. */
export function pairAccept(code: string, addr: string): Promise<PeerInfo[]> {
  return call<PeerInfo[]>("pair_accept", { code, addr });
}

export function unpair(pairingId: string): Promise<void> {
  return call<void>("unpair", { pairingId });
}

export function syncNow(pairingId?: string): Promise<SyncResult[]> {
  return call<SyncResult[]>("sync_now", { pairingId: pairingId ?? null });
}

/* --------------------------------------------------------------- service --- */

/** Not routed yet: starting the service needs a daemon-lifecycle decision. The
 *  offline screen falls back to the manual instruction on `unavailable`. */
export function startService(): Promise<void> {
  return call<void>("start_service");
}

/* -------------------------------------------------------------- shortcut --- */

/** Not routed yet. The default must come from the backend rather than a TS
 *  constant, or the two drift (CopyPaste-sqw0); `DEFAULT_SHORTCUT` in
 *  `lib/accelerator.ts` is the fallback until this exists. */
export function getDefaultShortcut(): Promise<string> {
  return call<string>("get_default_shortcut");
}

/** Not routed yet. */
export function getShortcut(): Promise<string> {
  return call<string>("get_shortcut");
}

/** Not routed yet. The bridge refuses the five media keys with a test each;
 *  `captureAccelerator` refuses them here too so the user learns why before
 *  spending a round trip. */
export function setShortcut(accelerator: string): Promise<void> {
  return call<void>("set_shortcut", { accelerator });
}
