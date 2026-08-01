/**
 * Command names track `copypaste_ipc::Method`; field names are snake_case
 * because that is what serde emits.
 */
import { call, hasBridge, hasWebBridge } from "./ipcCall";
import type {
  ConfigApplied,
  ConfigPatch,
  DiscoveredDevice,
  ExportReport,
  ImagePreview,
  InstalledSourceApp,
  SourceAppIcon,
  ImportData,
  ImportPreview,
  Item,
  ItemPage,
  PeerInfo,
  ServiceState,
  StatusData,
  SyncResult,
} from "@/generated/ipc";

export type {
  ConfigApplied,
  ConfigData,
  ConfigPatch,
  DiagnosticCounters,
  DiscoveredDevice,
  ErrorCode,
  ExportReport,
  ImagePreview,
  InstalledSourceApp,
  SourceAppIcon,
  ImportData,
  ImportPreview,
  Item,
  ItemPage,
  Liveness,
  PeerInfo,
  ServiceState,
  StatusData,
  SyncResult,
  UiError,
} from "@/generated/ipc";

/** Stable UI name retained for the Rust `ImportData` response DTO. */
export type ImportReport = ImportData;

export interface PairingData {
  readonly code: string;
  readonly pairing_id: string;
  readonly listen_addr: string | null;
}

export { hasBridge };

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

/** A bounded PNG thumbnail requested only for a visible history image. */
export function getImagePreview(id: string): Promise<ImagePreview> {
  return call<ImagePreview>("get_image_preview", { id });
}

/** A bounded native app icon resolved from a captured bundle/package id. */
export function getSourceAppIcon(bundleId: string): Promise<SourceAppIcon | null> {
  return call<SourceAppIcon | null>("get_source_app_icon", { bundleId });
}

/** Android's PackageManager catalogue for Settings → Service exclusions. */
export function listInstalledSourceApps(): Promise<InstalledSourceApp[]> {
  return call<InstalledSourceApp[]>("list_installed_source_apps");
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

export function createPairing(name: string): Promise<PairingData> {
  return call<PairingData>("pair_create", { name });
}

export function acceptPairing(code: string, addr: string): Promise<PeerInfo[]> {
  return call<PeerInfo[]>("pair_accept", { code, addr });
}

export function scanPairingQr(): Promise<string | null> {
  return call<string | null>("scan_pairing_qr");
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
  if (hasWebBridge()) return navigator.clipboard.writeText(text);
  return call<void>("copy_text", { text });
}

/** Reads pairing credentials only after an explicit Paste action. The native
 * command keeps arbitrary page code from receiving permanent clipboard access. */
export function readPairingText(): Promise<string> {
  if (hasWebBridge()) return navigator.clipboard.readText();
  return call<string>("read_pairing_text");
}

export function syncNow(pairingId?: string): Promise<SyncResult[]> {
  return call<SyncResult[]>("sync_now", { pairingId: pairingId ?? null });
}

export function listDiscovered(): Promise<DiscoveredDevice[]> {
  return call<DiscoveredDevice[]>("discovered");
}

/** Multicast is unreliable exactly where people pair, so "nothing found" needs
 *  a retry that is not "quit the app". */
export function rescanDiscovered(): Promise<DiscoveredDevice[]> {
  return call<DiscoveredDevice[]>("rescan");
}

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

/** Browser Quick Paste has no native window to reveal. */
export function openSettingsFromQuickPaste(): Promise<void> {
  if (hasWebBridge()) {
    const url = new URL(window.location.href);
    url.searchParams.delete("surface");
    url.searchParams.set("view", "settings");
    window.location.assign(url.toString());
    return Promise.resolve();
  }
  return showMainWindow();
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

export function getConfig(): Promise<ConfigApplied> {
  return call<ConfigApplied>("get_config");
}

export function setConfig(patch: ConfigPatch): Promise<ConfigApplied> {
  return call<ConfigApplied>("set_config", { patch });
}

/** INV-12 held by the shape of the command: the platform's own panel asks and
 *  answers in Rust, so no path is passed in and none comes back. `null` means
 *  the user closed the panel, which is not a failure. */
export function exportHistory(
  includeSensitive: boolean,
): Promise<ExportReport | null> {
  return call<ExportReport | null>("export_history", { includeSensitive });
}

/** Choose, read and parse only. `null` means the panel was closed. */
export function prepareImportHistory(): Promise<ImportPreview | null> {
  return call<ImportPreview | null>("prepare_import_history");
}

/** Overwrites nothing: every item goes through the same ingest a copy does. */
export function applyImportHistory(token: string): Promise<ImportReport> {
  return call<ImportReport>("apply_import_history", { token });
}

/** Safe to repeat, including after the preview has already been replaced. */
export function cancelImportHistory(token: string): Promise<void> {
  return call<void>("cancel_import_history", { token });
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
