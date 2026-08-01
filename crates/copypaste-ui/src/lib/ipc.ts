/**
 * Command names track `copypaste_ipc::Method`; field names are snake_case
 * because that is what serde emits.
 */
import { call, hasBridge } from "./ipcCall";

export { hasBridge };

/** INV-10: `content` is `null` for a sensitive item, never a mask. The bridge
 *  drops the plaintext before serialising, so the type enforces it rather than
 *  each component remembering to. `revealItem` is the one route back. */
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

/** `skipped_undecryptable` is not an error: it is the difference between a
 *  short page and a small history (finding 17). */
export interface ItemPage {
  readonly items: readonly Item[];
  /** Full live-history count, so a capped surface never silently truncates. */
  readonly total: number;
  readonly skipped_undecryptable: number;
  /** Opaque; pass it straight back to `listItems`. `null` is the **only**
   *  end-of-list test — a page shortened by `skipped_undecryptable` rows still
   *  has the rest of the history behind it. */
  readonly next_cursor: string | null;
}

export interface StatusData {
  readonly version: string;
  readonly protocol_version: number;
  readonly item_count: number;
  readonly capture_running: boolean;
  /** Surfaced so a fake backend cannot be mistaken for the real pasteboard. */
  readonly clipboard_backend: string;
}

/** `code` is the Noise pre-shared key in transferable form: anyone holding it
 *  can pair, so it is hidden until revealed, never logged, never toasted, and
 *  never retrievable again after this one response. */
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

/** `cursor` is a position, not a row number: the list grows at the top while it
 *  is read, so an offset taken for page 1 names a different boundary by page 2
 *  and a row repeats or is never seen (`CopyPaste-8ebg.57`). Never parse, build
 *  or persist a token. */
export function listItems(
  limit: number,
  cursor: string | null,
): Promise<ItemPage> {
  return call<ItemPage>("list", { limit, cursor });
}

/** Not paged: FTS5 rank is a score, not an order to seek on, so this returns
 *  the best `limit` matches and `next_cursor` is always `null`
 *  (AT-73 / `CopyPaste-crh3.106`). */
export function searchItems(query: string, limit: number): Promise<ItemPage> {
  return call<ItemPage>("search", { query, limit });
}

export function copyItem(id: string): Promise<Item> {
  return call<Item>("copy_item", { id });
}

/** Quick Paste's explicit ⌥Enter action. The item stays behind the native
 * boundary; only its id crosses the WebView bridge. */
export function copyItemAsPlainText(id: string): Promise<Item> {
  return call<Item>("copy_item_as_plain_text", { id });
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

export function listPeers(): Promise<PeerInfo[]> {
  return call<PeerInfo[]>("peers");
}

export function unpair(pairingId: string): Promise<void> {
  return call<void>("unpair", { pairingId });
}

/** Not `unpair` with a flag: an unpaired pairing can be enrolled again with the
 *  same code, and a revoked pairing id is refused for ever. */
export function revokeDevice(pairingId: string): Promise<void> {
  return call<void>("revoke", { pairingId });
}

/** Text the screen already shows. Not the clipboard plugin:
 *  `capabilities/default.json` withholds `allow-write-text`. `copyItem` stays
 *  the route for an item — it takes an id, so a clipping's plaintext never
 *  enters the WebView. */
export function copyText(text: string): Promise<void> {
  return call<void>("copy_text", { text });
}

export function syncNow(pairingId?: string): Promise<SyncResult[]> {
  return call<SyncResult[]>("sync_now", { pairingId: pairingId ?? null });
}

/** INV-15: nothing here is verified. Name, address and pairing id are what an
 *  unauthenticated mDNS record claimed; only the Noise handshake proves any of
 *  it. */
export interface DiscoveredDevice {
  readonly pairing_id: string;
  readonly name: string;
  /** `host:port`, retained for peer reachability diagnostics. */
  readonly addr: string;
  readonly last_seen_ms: number;
  readonly paired: boolean;
}

export function listDiscovered(): Promise<DiscoveredDevice[]> {
  return call<DiscoveredDevice[]>("discovered");
}

/** Multicast is unreliable exactly where people pair, so "nothing found" needs
 *  a retry that is not "quit the app". */
export function rescanDiscovered(): Promise<DiscoveredDevice[]> {
  return call<DiscoveredDevice[]>("rescan");
}

/** Four situations need four answers (ADR-0004): nothing to start, something to
 *  start, running, and running on a version this app did not ship with. */
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

/** INV-25: through the backend, never `window.hide()`. On macOS the app is an
 *  `Accessory`, so hiding hands activation back to whatever the user was in —
 *  which is where they press ⌘V, since the app never synthesises a paste
 *  (ADR-0001). */
export function hideWindow(): Promise<void> {
  return call<void>("hide_window");
}

/** Show the full application surface from the compact Quick Paste popup. */
export function showMainWindow(): Promise<void> {
  return call<void>("show_main_window");
}

/** INV-35. The window is created protected on both platforms, so this only
 *  carries the user's opt-out across; a rejection can be logged and dropped. */
export function setAllowScreenshots(allow: boolean): Promise<void> {
  return call<void>("set_allow_screenshots", { allow });
}

export type {
  CaptureHealth,
  CaptureNextStep,
  CaptureRung,
  CaptureSnapshot,
  CaptureSource,
  CapturedPayload,
  NotGrantedReason,
  NotWorkingReason,
  ShizukuProbe,
} from "./ipcCapture";
export {
  captureArm,
  captureDisarm,
  captureNow,
  captureRefresh,
  captureSetEnabled,
  captureSetToastSuppressed,
  captureState,
  captureToastExplanation,
} from "./ipcCapture";

/** `copypaste_ipc::ConfigData`. `excluded_app_bundle_ids` is carried so the
 *  shape matches the wire and is deliberately offered as no control: the
 *  service stores it but does not yet enforce it, and a switch that does
 *  nothing is worse than none. */
export interface ConfigData {
  readonly poll_interval_ms: number;
  readonly history_limit: number;
  /** Maximum bytes retained in unpinned live ciphertext. */
  readonly storage_quota_bytes: number;
  /** `0` disables age-based eviction. */
  readonly retention_days: number;
  readonly dedup_window_secs: number;
  readonly max_item_bytes: number;
  /** `0` is **off**, not "delete immediately" (`CopyPaste-8ebg.1`). */
  readonly sensitive_ttl_secs: number;
  readonly excluded_app_bundle_ids: readonly string[];
  readonly lan_visibility: boolean;
  readonly sync_enabled: boolean;
  readonly notify_on_copy: boolean;
  readonly sound_on_copy: boolean;
}

/** A patch names only what changed, so two screens editing different settings
 *  cannot overwrite each other. */
export type ConfigPatch = Partial<{
  -readonly [K in Exclude<keyof ConfigData, "excluded_app_bundle_ids">]: ConfigData[K];
}>;

/** `restart_required` names the fields the service kept but has not yet acted
 *  on. It comes back from the write rather than being derived here, so the two
 *  cannot disagree about which those are. */
export interface ConfigApplied {
  readonly config: ConfigData;
  readonly restart_required: readonly string[];
}

export function getConfig(): Promise<ConfigApplied> {
  return call<ConfigApplied>("get_config");
}

export function setConfig(patch: ConfigPatch): Promise<ConfigApplied> {
  return call<ConfigApplied>("set_config", { patch });
}

/** The three skip counts are always present, including when zero: an export
 *  withholds every flagged item unless it was asked twice, and a user who is
 *  not told the number believes they exported everything. */
export interface ExportReport {
  readonly exported: number;
  readonly skipped_sensitive: number;
  readonly skipped_non_text: number;
  readonly skipped_undecryptable: number;
}

/** `skipped` counts items the service already held, not failures: a malformed
 *  file is refused whole, before anything is written. */
export interface ImportReport {
  readonly inserted: number;
  readonly skipped: number;
}

/** INV-12 held by the shape of the command: the platform's own panel asks and
 *  answers in Rust, so no path is passed in and none comes back. `null` means
 *  the user closed the panel, which is not a failure. */
export function exportHistory(
  includeSensitive: boolean,
): Promise<ExportReport | null> {
  return call<ExportReport | null>("export_history", { includeSensitive });
}

/** Overwrites nothing: every item goes through the same ingest a copy does. */
export function importHistory(): Promise<ImportReport | null> {
  return call<ImportReport | null>("import_history");
}

/** The size of the backup in bytes, or `null` if the panel was closed. */
export function backupDatabase(): Promise<number | null> {
  return call<number | null>("backup_database");
}

/** **Replaces this device's history.** `false` means the panel was closed. */
export function restoreDatabase(): Promise<boolean> {
  return call<boolean>("restore_database");
}

/** The default is native-owned so the capture UI cannot drift from the binding
 * actually registered by the desktop shell. */
export function getDefaultShortcut(): Promise<string> {
  return call<string>("get_default_shortcut");
}

export function getShortcut(): Promise<string> {
  return call<string>("get_shortcut");
}

/** The bridge refuses the five media keys with a test each;
 *  `captureAccelerator` refuses them here too so the user learns why before
 *  spending a round trip. */
export function setShortcut(accelerator: string): Promise<void> {
  return call<void>("set_shortcut", { accelerator });
}
