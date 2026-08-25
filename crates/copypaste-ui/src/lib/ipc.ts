import { call, hasBridge, hasWebBridge } from "./ipcCall";
import { DEFAULT_SHORTCUT } from "./accelerator";
import { UI_COMMANDS } from "@/generated/ipc";
import type {
  CloudStatusData,
  CloudSyncData,
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
  PairingCeremony,
  PeerInfo,
  PrivateModeData,
  ServiceState,
  StatusData,
  SyncResult,
} from "@/generated/ipc";

export type {
  CloudStatusData,
  CloudSyncData,
  ConfigApplied,
  ConfigData,
  ConfigPatch,
  DiagnosticCounters,
  DeviceClass,
  DeviceDetails,
  DeviceEndpointObservation,
  DeviceLatencyObservation,
  DeviceObservationProvenance,
  DeviceObservationTrust,
  DevicePlatform,
  DevicePresenceObservation,
  DeviceProfileObservation,
  DiscoveredDevice,
  ErrorCode,
  ExternalNetworkObservation,
  ExportReport,
  ImagePreview,
  InstalledSourceApp,
  SourceAppIcon,
  ImportData,
  ImportPreview,
  Item,
  ItemPage,
  Liveness,
  PairedDevice,
  PairingCeremony,
  PairingPresentationState,
  PairingRole,
  PairingState,
  PeerInfo,
  PrivateModeData,
  ServiceState,
  StatusData,
  SyncResult,
  UiError,
} from "@/generated/ipc";

export { CURRENT_PROTOCOL_VERSION } from "@/generated/ipc";

/** Stable UI name retained for the Rust `ImportData` response DTO. */
export type ImportReport = ImportData;

export { hasBridge, hasWebBridge };

export interface PreviewPairingInvite {
  ceremony: PairingCeremony;
  code: string;
  listen_addr: string;
  expires_in_secs: number;
  qr_svg: string;
}

/** `cursor` is a position, not a row number: the list grows at the top while it
 *  is read, so an offset taken for page 1 names a different boundary by page 2
 *  and a row repeats or is never seen (`CopyPaste-8ebg.57`). Never parse, build
 *  or persist a token. */
export function listItems(
  limit: number,
  cursor: string | null,
): Promise<ItemPage> {
  return call(UI_COMMANDS.list, { limit, cursor });
}

/** Not paged: FTS5 rank is a score, not an order to seek on, so this returns
 *  the best `limit` matches and `next_cursor` is always `null`
 *  (AT-73 / `CopyPaste-crh3.106`). */
export function searchItems(query: string, limit: number): Promise<ItemPage> {
  return call(UI_COMMANDS.search, { query, limit });
}

export function copyItem(id: string): Promise<Item> {
  return call(UI_COMMANDS.copy_item, { id });
}

/** Quick Paste's explicit ⌥Enter action. The item stays behind the native
 * boundary; only its id crosses the WebView bridge. */
export function copyItemAsPlainText(id: string): Promise<Item> {
  return call(UI_COMMANDS.copy_item_as_plain_text, { id });
}

/** One clipboard write for the whole selection, and the count that actually
 *  reached it — sensitive and binary rows are excluded by the backend, so a
 *  result below `ids.length` is a partial the caller must report rather than
 *  round up to "Copied". Rejects, writing nothing, if a row has gone. */
export function copyItems(ids: readonly string[]): Promise<number> {
  return call(UI_COMMANDS.copy_items, { ids });
}

export function addItem(content: string): Promise<Item> {
  return call(UI_COMMANDS.add_item, { content });
}

/** One item's plaintext, on demand. Held in component state and dropped when
 *  the reveal expires (INV-11) — never in the query cache, which outlives the
 *  row and would restore it on the next render. */
export function revealItem(id: string): Promise<string> {
  return call(UI_COMMANDS.reveal_item, { id });
}

/** Complete plaintext for a non-sensitive item. The native command rejects
 * sensitive rows before content crosses the WebView boundary. */
export function getItemBody(id: string): Promise<string> {
  return call(UI_COMMANDS.get_item_body, { id });
}

/** A bounded PNG thumbnail requested only for a visible history image. */
export function getImagePreview(id: string): Promise<ImagePreview> {
  return call(UI_COMMANDS.get_image_preview, { id });
}

/** A bounded native app icon resolved from a captured bundle/package id. */
export function getSourceAppIcon(bundleId: string): Promise<SourceAppIcon | null> {
  return call(UI_COMMANDS.get_source_app_icon, { bundleId });
}

/** Platform catalogue of user-launchable applications for exclusions. */
export function listInstalledSourceApps(): Promise<InstalledSourceApp[]> {
  return call(UI_COMMANDS.list_installed_source_apps);
}

export function deleteItem(id: string): Promise<boolean> {
  return call(UI_COMMANDS.delete_item, { id });
}

/** Every unpinned item; pinned ones survive, as with `copypaste clear`. */
/** `through` names the set the user meant when they asked, so a clip captured
 *  during an undo window is not destroyed by an action that predates it. */
export function deleteAll(through?: number): Promise<number> {
  return call(UI_COMMANDS.delete_all, { through: through ?? null });
}

export function historyCeiling(): Promise<number> {
  return call(UI_COMMANDS.history_ceiling);
}

export function setPinned(id: string, pinned: boolean): Promise<Item> {
  return call(UI_COMMANDS.set_pinned, { id, pinned });
}

export function reorderPinned(ids: readonly string[]): Promise<void> {
  return call(UI_COMMANDS.reorder_pinned, { ids });
}

export function getStatus(): Promise<StatusData> {
  return call(UI_COMMANDS.status);
}

export function setDeviceName(name: string): Promise<void> {
  return call(UI_COMMANDS.set_device_name, { name });
}

export interface CloudCredentials {
  email: string;
  password: string;
  passphrase: string;
}

export function cloudSignIn(credentials: CloudCredentials): Promise<CloudStatusData> {
  return call(UI_COMMANDS.cloud_sign_in, { ...credentials });
}

export function cloudSignUp(credentials: CloudCredentials): Promise<CloudStatusData> {
  return call(UI_COMMANDS.cloud_sign_up, { ...credentials });
}

export function cloudSetEndpoint(url: string, anonKey: string): Promise<CloudStatusData> {
  return call(UI_COMMANDS.cloud_set_endpoint, { url, anonKey });
}

export function cloudSignOut(): Promise<CloudStatusData> {
  return call(UI_COMMANDS.cloud_sign_out);
}

export function getCloudStatus(): Promise<CloudStatusData> {
  return call(UI_COMMANDS.cloud_status);
}

export function syncCloudNow(): Promise<CloudSyncData> {
  return call(UI_COMMANDS.cloud_sync_now);
}

export function listPeers(): Promise<PeerInfo[]> {
  return call(UI_COMMANDS.peers);
}

export function unpair(pairingId: string): Promise<void> {
  return call(UI_COMMANDS.unpair, { pairingId });
}

/** Not `unpair` with a flag: an unpaired pairing can be enrolled again with the
 *  same code, and a revoked pairing id is refused for ever. */
export function revokeDevice(pairingId: string): Promise<void> {
  return call(UI_COMMANDS.revoke, { pairingId });
}

/** Text the screen already shows. Not the clipboard plugin:
 *  `capabilities/default.json` withholds `allow-write-text`. `copyItem` stays
 *  the route for an item — it takes an id, so a clipping's plaintext never
 *  enters the WebView. */
export function copyText(text: string): Promise<void> {
  if (hasWebBridge()) return navigator.clipboard.writeText(text);
  return call(UI_COMMANDS.copy_text, { text });
}

export function syncNow(pairingId?: string): Promise<SyncResult[]> {
  return call(UI_COMMANDS.sync_now, { pairingId: pairingId ?? null });
}

export function listDiscovered(): Promise<DiscoveredDevice[]> {
  return call(UI_COMMANDS.discovered);
}

/** Multicast is unreliable exactly where people pair, so "nothing found" needs
 *  a retry that is not "quit the app". */
export function rescanDiscovered(): Promise<DiscoveredDevice[]> {
  return call(UI_COMMANDS.rescan);
}

export function createPairingInvite(): Promise<PairingCeremony> {
  return call(UI_COMMANDS.pair_create_invite);
}

export function createPreviewPairingInvite(): Promise<PreviewPairingInvite> {
  return call(UI_COMMANDS.pair_preview_invite);
}

export function scanPairingInvite(): Promise<PairingCeremony> {
  return call(UI_COMMANDS.pair_scan_invite);
}

export function joinPreviewPairingInvite(
  code: string,
  addr: string,
): Promise<PairingCeremony> {
  return call(UI_COMMANDS.pair_preview_join, { code, addr });
}

export function getPairingProgress(): Promise<PairingCeremony> {
  return call(UI_COMMANDS.pair_progress);
}

export function presentPairing(): Promise<PairingCeremony> {
  return call(UI_COMMANDS.pair_present);
}

export function confirmPairing(): Promise<PairingCeremony> {
  return call(UI_COMMANDS.pair_confirm);
}

export function rejectPairing(): Promise<PairingCeremony> {
  return call(UI_COMMANDS.pair_reject);
}

export function cancelPairing(): Promise<PairingCeremony> {
  return call(UI_COMMANDS.pair_cancel);
}

export function serviceState(): Promise<ServiceState> {
  return call(UI_COMMANDS.service_state);
}

/** Starts it if it is not running, and resolves only once it answers. */
export function startService(): Promise<ServiceState> {
  return call(UI_COMMANDS.start_service);
}

/** Stops what this app started and starts it again. Refuses for a service it
 *  did not start — see ADR-0004. */
export function restartService(): Promise<ServiceState> {
  return call(UI_COMMANDS.restart_service);
}

/** INV-25: through the backend, never `window.hide()`. On macOS the app is an
 *  `Accessory`, so hiding hands activation back to whatever the user was in —
 *  which is where they press ⌘V, since the app never synthesises a paste
 *  (ADR-0001). */
export function hideWindow(): Promise<void> {
  return call(UI_COMMANDS.hide_window);
}

/** Show the full application surface from the compact Quick Paste popup. */
export function showMainWindow(): Promise<void> {
  return call(UI_COMMANDS.show_main_window);
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
  if (hasWebBridge()) return Promise.resolve();
  return call(UI_COMMANDS.set_allow_screenshots, { allow });
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
  captureOpenDeveloperOptions,
  captureOpenShizuku,
  captureRefresh,
  captureRequestBatteryExemption,
  captureSetEnabled,
  captureSetToastSuppressed,
  captureState,
  captureToastExplanation,
} from "./ipcCapture";
export {
  permissionOpenSettings,
  permissionRequest,
  permissionSnapshot,
} from "./ipcPermissions";
export type {
  OnboardingPermissionId,
  OnboardingPermissionItem,
  OnboardingPermissionStatus,
  OnboardingPermissions,
  PermissionHost,
} from "./ipcPermissions";

export function getConfig(): Promise<ConfigApplied> {
  return call(UI_COMMANDS.get_config);
}

export function setConfig(patch: ConfigPatch): Promise<ConfigApplied> {
  return call(UI_COMMANDS.set_config, { patch });
}

export function getPrivateMode(): Promise<PrivateModeData> {
  return call(UI_COMMANDS.get_private_mode);
}

export function setPrivateMode(enabled: boolean): Promise<PrivateModeData> {
  return call(UI_COMMANDS.set_private_mode, { enabled });
}

/** INV-12 held by the shape of the command: the platform's own panel asks and
 *  answers in Rust, so no path is passed in and none comes back. `null` means
 *  the user closed the panel, which is not a failure. */
export function exportHistory(
  includeSensitive: boolean,
): Promise<ExportReport | null> {
  return call(UI_COMMANDS.export_history, { includeSensitive });
}

/** Choose, read and parse only. `null` means the panel was closed. */
export function prepareImportHistory(): Promise<ImportPreview | null> {
  return call(UI_COMMANDS.prepare_import_history);
}

/** Overwrites nothing: every item goes through the same ingest a copy does. */
export function applyImportHistory(token: string): Promise<ImportReport> {
  return call(UI_COMMANDS.apply_import_history, { token });
}

/** Safe to repeat, including after the preview has already been replaced. */
export function cancelImportHistory(token: string): Promise<void> {
  return call(UI_COMMANDS.cancel_import_history, { token });
}

/** The size of the backup in bytes, or `null` if the panel was closed. */
export function backupDatabase(): Promise<number | null> {
  return call(UI_COMMANDS.backup_database);
}

/** **Replaces this device's history.** `false` means the panel was closed. */
export function restoreDatabase(): Promise<boolean> {
  return call(UI_COMMANDS.restore_database);
}

/** The default is native-owned so the capture UI cannot drift from the binding
 * actually registered by the desktop shell. */
export function getDefaultShortcut(): Promise<string> {
  if (hasWebBridge()) return Promise.resolve(DEFAULT_SHORTCUT);
  return call(UI_COMMANDS.get_default_shortcut);
}

let webBridgeShortcut = DEFAULT_SHORTCUT;

export function getShortcut(): Promise<string> {
  if (hasWebBridge()) return Promise.resolve(webBridgeShortcut);
  return call(UI_COMMANDS.get_shortcut);
}

/** The bridge refuses the media keys its platform cannot bind, with a test
 *  each; `captureAccelerator` refuses the same set here so the user learns why
 *  before spending a round trip. */
export function setShortcut(accelerator: string): Promise<void> {
  if (hasWebBridge()) {
    webBridgeShortcut = accelerator;
    return Promise.resolve();
  }
  return call(UI_COMMANDS.set_shortcut, { accelerator });
}

export function getOpenAtLogin(): Promise<boolean> {
  if (hasWebBridge()) return Promise.resolve(false);
  return call(UI_COMMANDS.get_open_at_login);
}

/** Answers with the state the system reports afterwards, not the one asked
 *  for — Windows can accept the registry write while Task Manager's Startup
 *  list still holds the app off. */
export function setOpenAtLogin(enabled: boolean): Promise<boolean> {
  if (hasWebBridge()) return Promise.resolve(enabled);
  return call(UI_COMMANDS.set_open_at_login, { enabled });
}
