/**
 * The only place `invoke` is called. Command names track
 * `copypaste_ipc::Method`; field names are snake_case because that is what
 * serde emits.
 *
 * A command the bridge does not route, and an operation a build cannot perform
 * (`BackendError::Unsupported` — Android has no pairing), both classify as
 * `unavailable`. Screens render that as its own state: "this build cannot" and
 * "the service is down" are different things to be told, and only one of them
 * is worth retrying.
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


/**
 * `skipped_undecryptable` is not an error: it is the difference between a short
 * page and a small history, and without it the user sees fewer items and no
 * reason (finding 17).
 */
export interface ItemPage {
  readonly items: readonly Item[];
  readonly skipped_undecryptable: number;
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

export function listItems(limit: number, offset: number): Promise<ItemPage> {
  return call<ItemPage>("list", { limit, offset });
}

export function searchItems(query: string, limit: number): Promise<ItemPage> {
  return call<ItemPage>("search", { query, limit });
}

export function copyItem(id: string): Promise<Item> {
  return call<Item>("copy_item", { id });
}

export function addItem(content: string): Promise<Item> {
  return call<Item>("add_item", { content });
}

/** One item's plaintext, on demand. Held in component state and dropped when
 *  the reveal expires (INV-11) — never in the query cache, which outlives the
 *  row and would restore it on the next render. */
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

/** Not routed yet: `copypaste_ipc::Method` has no reorder verb, so the bridge
 *  refuses with `unavailable` and the drag handles stay hidden. */
export function reorderPinned(ids: readonly string[]): Promise<void> {
  return call<void>("reorder_pinned", { ids });
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

/**
 * A device seen on the LAN. **Nothing here is verified** — name, address and
 * pairing id are what an unauthenticated mDNS record claimed, and only the
 * Noise handshake proves any of it (INV-15).
 */
export interface DiscoveredDevice {
  readonly pairing_id: string;
  readonly name: string;
  /** `host:port`, ready to hand to `pairAccept`. */
  readonly addr: string;
  readonly last_seen_ms: number;
  readonly paired: boolean;
}

export function listDiscovered(): Promise<DiscoveredDevice[]> {
  return call<DiscoveredDevice[]>("discovered");
}

/** Advertise and browse again. Multicast is unreliable exactly where people
 *  pair, so "nothing found" needs a retry that is not "quit the app". */
export function rescanDiscovered(): Promise<DiscoveredDevice[]> {
  return call<DiscoveredDevice[]>("rescan");
}

/* --------------------------------------------------------------- service --- */

/** A tagged union rather than a boolean because four situations need four
 *  answers (ADR-0004): nothing to start, something to start, running, and
 *  running on a version this app did not ship with. */
export type ServiceState =
  | { readonly state: "running"; readonly version: string; readonly matches_app: boolean; readonly ours: boolean }
  | { readonly state: "unhealthy" }
  | { readonly state: "stopped" }
  | { readonly state: "not_installed" };

export function serviceState(): Promise<ServiceState> {
  return call<ServiceState>("service_state");
}

/** Starts it if it is not running, and resolves only once it answers. */
export function startService(): Promise<ServiceState> {
  return call<ServiceState>("start_service");
}

/** Stops what this app started and starts it again. Refuses for a service it
 *  did not start — see ADR-0004. */
export function restartService(): Promise<ServiceState> {
  return call<ServiceState>("restart_service");
}

/**
 * INV-25: through the backend, never `window.hide()`. On macOS the app is an
 * `Accessory`, so hiding hands activation back to whatever the user was in —
 * which is where they press ⌘V, since the app never synthesises a paste
 * (ADR-0001).
 */
export function hideWindow(): Promise<void> {
  return call<void>("hide_window");
}

/* --------------------------------------------------------------- capture --- */

/** The mechanism capturing right now — never the one that was asked for. */
export type CaptureRung = "desktop" | "in_app" | "shizuku";

export type NotGrantedReason =
  | "unsupported"
  | "not_installed"
  | "not_running"
  | "no_permission";

export type NotWorkingReason = "awaiting_first_copy" | "read_refused" | "not_armed";

/** `working` is reachable only through a read that happened without focus —
 *  a permission being present is not evidence (CopyPaste-qzhu). Nothing in this
 *  file may construct one. */
export type CaptureHealth =
  | { readonly state: "not_granted"; readonly reason: NotGrantedReason }
  | { readonly state: "disabled" }
  | { readonly state: "granted_not_working"; readonly reason: NotWorkingReason }
  | { readonly state: "working" };

export type CaptureNextStep =
  | "none"
  | "install_shizuku"
  | "start_shizuku"
  | "grant_permission"
  | "arm";

export type CaptureSource = "in_app" | "share" | "process_text" | "tile" | "background";

export interface ShizukuProbe {
  /** Wireless debugging can be paired on the phone itself from Android 11. */
  readonly supported: boolean;
  readonly installed: boolean;
  /** False after every reboot until the user starts it again. */
  readonly running: boolean;
  readonly permission: boolean;
  readonly toastSuppressed: boolean;
  readonly rearmRequested: boolean;
}

/**
 * **`headline` and `detail` are finished sentences, not codes.** They are
 * authored and tested in `capture::messages` (ADR-0005) so that the setup
 * screen, the status strip and the loss notification cannot disagree about what
 * state this device is in. Rendering them verbatim is the contract; deriving
 * replacements from `health` here would be the drift that split them.
 */
export interface CaptureSnapshot {
  readonly rung: CaptureRung;
  readonly health: CaptureHealth;
  readonly shizuku: ShizukuProbe;
  readonly nextStep: CaptureNextStep;
  readonly headline: string;
  readonly detail: string | null;
  readonly lastReadOkAt: number | null;
  readonly lastCaptureAt: number | null;
  /** Copies that were taken from the platform and never stored. */
  readonly droppedClips: number;
  readonly toastSuppressed: boolean;
  readonly toastAcknowledged: boolean;
  /** The app was opened from the "background capture stopped" notification. */
  readonly rearmRequested: boolean;
}

/** Carries no clipboard content: the list re-reads through `list`. */
export interface CapturedPayload {
  readonly id: string;
  readonly source: CaptureSource;
  readonly isSensitive: boolean;
}

export function captureState(): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_state");
}

/** Re-asks the platform. Called on every resume, not only at startup: a grant
 *  can lapse while the app is in the background, and a reboot is the ordinary
 *  case rather than the exception. */
export function captureRefresh(): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_refresh");
}

/** One call for two steps: it asks for the permission when that is what is
 *  missing, and registers the listener when it is not. */
export function captureArm(): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_arm");
}

export function captureDisarm(): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_disarm");
}

export function captureSetEnabled(enabled: boolean): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_set_enabled", { enabled });
}

/** Rung 0: save whatever is on the clipboard right now. `null` means there was
 *  nothing to save, which is not a failure. */
export function captureNow(source: CaptureSource): Promise<Item | null> {
  return call<Item | null>("capture_now", { source });
}

/** The exact text `authorise_toast` gates on. Fetched rather than copied into
 *  the catalogue: a second copy is a second thing to keep true. */
export function captureToastExplanation(): Promise<string> {
  return call<string>("capture_toast_explanation");
}

/**
 * `acknowledged` may only be `true` when the user has read
 * `captureToastExplanation` and agreed to it. The gate is enforced in Rust so
 * it cannot be bypassed from here; passing `true` without having shown the text
 * would be lying to it. Turning suppression **off** is never gated.
 */
export function captureSetToastSuppressed(
  suppressed: boolean,
  acknowledged: boolean,
): Promise<CaptureSnapshot> {
  return call<CaptureSnapshot>("capture_set_toast_suppressed", {
    suppressed,
    acknowledged,
  });
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
