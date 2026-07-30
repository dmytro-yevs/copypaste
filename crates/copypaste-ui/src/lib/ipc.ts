/**
 * The Tauri bridge, and the only place `invoke` is called.
 *
 * Shapes mirror `crates/copypaste-ipc/src/lib.rs` exactly — the Rust crate is
 * the single model of the wire contract and these declarations are its
 * TypeScript view. Field names are the Rust field names (snake_case) because
 * that is what serde emits.
 *
 * ## The command surface
 *
 * `crates/copypaste-ui/src-tauri` is owned by the bridge author, not by the
 * frontend. Command names track `copypaste_ipc::Method` on both sides, so this
 * file and `src-tauri/src/commands/` name the same things.
 *
 * | command         | `Method`     | notes                                    |
 * |-----------------|--------------|------------------------------------------|
 * | `list`          | `List`       |                                          |
 * | `search`        | `Search`     | sensitive items are never indexed        |
 * | `copy_item`     | `Copy`       | by id — the content never crosses        |
 * | `add_item`      | `Add`        |                                          |
 * | `reveal_item`   | —            | the one route back to plaintext          |
 * | `delete_item`   | `Delete`     |                                          |
 * | `delete_all`    | `DeleteAll`  |                                          |
 * | `set_pinned`    | `Pin`        |                                          |
 * | `status`        | `Status`     | answers when nothing else can            |
 * | `peers`         | `Peers`      |                                          |
 * | `pair_create`   | `PairCreate` | returns a secret, exactly once           |
 * | `pair_accept`   | `PairAccept` |                                          |
 * | `unpair`        | `Unpair`     |                                          |
 * | `sync_now`      | `SyncNow`    |                                          |
 * | `start_service` | —            | **not routed yet** — see `startService`  |
 *
 * A command the bridge does not route, and an operation a build cannot perform
 * (`BackendError::Unsupported` — Android has no pairing yet), both classify as
 * the `unavailable` kind. Screens render that as its own state rather than as a
 * daemon error, because "this build cannot" and "the service is down" are
 * different things to be told.
 */
import { invoke } from "@tauri-apps/api/core";

import { IpcFailure, classifyError } from "./errors";

/**
 * `src-tauri`'s `UiItem` — one history item as the WebView is allowed to see
 * it.
 *
 * **`content` is `null` for a sensitive item.** Not an empty string, not a
 * mask: the bridge drops the plaintext at the process boundary before
 * serialising, so it never enters this heap at all. That is INV-10 enforced
 * structurally rather than by a component remembering to hide something, and it
 * is why the type is nullable — "there is no content" is a state the type
 * checker sees rather than a value every caller has to test for.
 *
 * The one route back to plaintext is `revealItem`, which the user reaches by
 * pressing a button.
 */
export interface Item {
  readonly id: string;
  readonly content: string | null;
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
 * `copypaste_ipc::PairingData` — returned once by `pair_create` and never
 * retrievable again.
 *
 * `code` is the transferable form of the Noise pre-shared key. Anyone holding
 * it can pair with this device, so it is treated as a credential everywhere it
 * appears: hidden until deliberately revealed, never logged, never put in a
 * toast, and never written to the clipboard except by an explicit user action.
 */
export interface PairingData {
  readonly code: string;
  readonly pairing_id: string;
  readonly listen_addr: string | null;
}

/** `copypaste_ipc::PeerInfo`. */
export interface PeerInfo {
  readonly pairing_id: string;
  readonly name: string;
  readonly last_addr: string | null;
  readonly last_seen_ms: number;
  /** Discovery is a convenience: `false` means "not seen", never
   *  "unreachable". */
  readonly online: boolean;
}

/** `copypaste_ipc::SyncResult`. */
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

/**
 * Outside a Tauri webview there is no bridge at all. That is indistinguishable
 * to the user from the background service being down, so it maps onto the same
 * state rather than throwing something the view layer has to special-case.
 */
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
 * The deliberate reveal gesture: one item's plaintext, fetched on demand.
 *
 * The result is held in component state for as long as it is on screen and
 * dropped when the reveal expires (INV-11), never written into the query cache
 * — a cache entry outlives the row, is inspectable from the devtools, and would
 * be restored by the next render.
 */
export function revealItem(id: string): Promise<string> {
  return call<string>("reveal_item", { id });
}

export function deleteItem(id: string): Promise<boolean> {
  return call<boolean>("delete_item", { id });
}

/** Every unpinned item, as `copypaste clear` does. Pinned items survive. */
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

/** Returns the peer list as it stands after the pairing, so the screen can move
 *  to its success state without re-listing. */
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
