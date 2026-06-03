import { invoke } from "@tauri-apps/api/core";

/** Raw daemon reply, mirrored from the Rust `ipc_call` bridge. */
export interface IpcReply {
  ok: boolean;
  data: unknown | null;
  error: string | null;
  error_code: string | null;
}

/** Error carrying the daemon's stable machine code (e.g. "daemon_offline"). */
export class IpcError extends Error {
  code: string | null;
  constructor(message: string, code: string | null) {
    super(message);
    this.name = "IpcError";
    this.code = code;
  }
}

/**
 * Call a daemon method over the Unix-socket bridge. Resolves to the daemon's
 * `data` payload on success; throws `IpcError` on a daemon error and on a
 * transport failure (e.g. the daemon being offline -> code "daemon_offline").
 */
export async function ipcCall<T = unknown>(
  method: string,
  params?: Record<string, unknown>
): Promise<T> {
  let reply: IpcReply;
  try {
    reply = await invoke<IpcReply>("ipc_call", { method, params: params ?? null });
  } catch (e) {
    // Transport-level failures come back as a string like "daemon_offline:/path".
    const raw = String(e);
    const code = raw.split(":", 1)[0] || null;
    throw new IpcError(raw, code);
  }
  if (!reply.ok) {
    throw new IpcError(reply.error ?? "unknown daemon error", reply.error_code);
  }
  return reply.data as T;
}

// ---------------------------------------------------------------------------
// Shared daemon types
// ---------------------------------------------------------------------------

export interface HistoryEntry {
  id: string;
  content_type: string;
  preview: string;
  is_sensitive: boolean;
  /**
   * Unicode scalar (code-point / character) offset ranges within `preview` that
   * are sensitive, e.g. [[0,4],[10,16]]. These are character offsets, NOT
   * UTF-16 units or bytes — the daemon counts characters. See `lib/masking.ts`.
   */
  sensitive_spans?: Array<[number, number]>;
  /** Unix epoch milliseconds. */
  wall_time: number;
  pinned: boolean;
  /**
   * macOS bundle id of the app that copied this item, e.g. "com.google.Chrome".
   * Present when the daemon captured the source app at copy time; null when
   * unknown (synced items from another device, older daemon builds, etc.).
   * The UI derives a short readable label via `sourceAppLabel()`.
   */
  app_bundle_id?: string | null;
}

/**
 * Derive a short readable label from a macOS bundle id.
 * "com.google.Chrome" → "Chrome", "com.apple.Safari" → "Safari".
 * Falls back to the raw bundle id when it doesn't contain a dot, or to
 * an empty string when the id is absent.
 */
export function sourceAppLabel(bundleId: string | null | undefined): string {
  if (!bundleId) return "";
  const parts = bundleId.split(".");
  const last = parts[parts.length - 1];
  // Title-case the last segment (handles e.g. "terminal" → "Terminal").
  return last.charAt(0).toUpperCase() + last.slice(1);
}

export interface HistoryPage {
  items: HistoryEntry[];
  total: number;
}

export interface AppSettings {
  p2p_enabled: boolean;
  supabase_url: string | null;
  supabase_anon_key: string | null;
  // Storage / Limits — all map 1-to-1 to AppConfig fields in copypaste-core.
  // Byte fields are stored as raw bytes (u64) and converted to MB in the UI.
  max_text_size_bytes?: number | null;
  max_image_size_bytes?: number | null;
  max_file_size_bytes?: number | null;
  storage_quota_bytes?: number | null;
  sensitive_ttl_secs?: number | null;
  image_quality?: number | null;
  // Sync parity
  sync_on_wifi_only?: boolean | null;
  // Sound / notification on copy — wired to daemon config.toml.
  sound_on_copy?: boolean | null;
  notify_on_copy?: boolean | null;
}

/**
 * Reply from the daemon's `status` method. This is the ONLY IPC method that
 * reports degraded state: when the daemon is up but its backing database is
 * unavailable (e.g. the SQLCipher key no longer matches after a reinstall) it
 * returns `ok:true` with `degraded:true` / `ready:false` plus a machine-readable
 * `degraded_reason`. A healthy daemon returns `degraded:false` / `ready:true`.
 * A reachable socket alone does NOT mean the daemon is fully functional — views
 * must inspect these fields.
 */
export interface DaemonStatus {
  /** "running" when healthy, "degraded" when the DB is unavailable. */
  status: string;
  private_mode: boolean;
  /** True only when the backing DB is open and usable. */
  ready: boolean;
  /** True when the daemon is up but its database cannot be opened/decrypted. */
  degraded: boolean;
  /** Human-readable reason for the degraded state, when present. */
  degraded_reason?: string | null;
  /** `<crate-version>+<git-sha>` (or just `<crate-version>`). Added for
   *  stale-daemon detection after an upgrade; absent on a daemon predating this
   *  field — itself a strong signal it is stale. */
  build_version?: string | null;
  /** Daemon OS process id, if reported. */
  pid?: number | null;
  // TODO(task-7): expose supabase_account_id from daemon status so the UI can
  // surface a cross-device account mismatch in SettingsView's cloud section.
  // Add `supabase_account_id?: string | null` here once the daemon emits it.
}

/**
 * Normalized result of probing {@link api.status}. `kind` collapses the three
 * meaningful daemon conditions into a single discriminant so views can branch
 * without each re-implementing degraded/offline detection:
 *  - "ok": daemon up and its DB is usable.
 *  - "degraded": daemon up but DB unavailable (carries `reason`).
 *  - "offline": daemon unreachable (transport failure / `daemon_offline`).
 */
export type StatusProbe =
  | { kind: "ok" }
  | { kind: "degraded"; reason: string | null }
  | { kind: "offline" };

/**
 * Probe the daemon's status and collapse it to a {@link StatusProbe}. Never
 * throws — a transport failure resolves to `{ kind: "offline" }` so every caller
 * has a defined, non-blank failure path.
 */
export async function probeStatus(): Promise<StatusProbe> {
  try {
    const s = (await api.status()) as Partial<DaemonStatus>;
    if (s && (s.degraded === true || s.ready === false)) {
      return { kind: "degraded", reason: s.degraded_reason ?? null };
    }
    return { kind: "ok" };
  } catch {
    // Transport-level failure (daemon offline) — IpcError or otherwise.
    return { kind: "offline" };
  }
}

export interface SyncStatus {
  passphrase_set: boolean;
  supabase_configured: boolean;
  signed_in: boolean;
  /** Unix epoch milliseconds of last sync, or null if never synced. */
  last_sync_ms: number | null;
  /** Supabase project URL, if configured via env or settings. */
  supabase_url?: string | null;
  /** Signed-in account email, if available. */
  email?: string | null;
  // NOTE: degraded state is intentionally NOT modeled here. get_sync_status does
  // not report it — the daemon exposes degraded/ready ONLY via `status` (see
  // DaemonStatus / probeStatus). The fields below are kept for SettingsView
  // compatibility; the daemon no longer emits them (always undefined at runtime).
  /** @deprecated Never emitted by daemon; kept for SettingsView compat. */
  keychain_locked?: boolean;
  /** @deprecated Never emitted by daemon; kept for SettingsView compat. */
  db_unavailable?: boolean;
  /** @deprecated Never emitted by daemon; kept for SettingsView compat. */
  degraded_reason?: string | null;
}


export interface PairedDevice {
  fingerprint: string;
  name: string;
  /** Unix epoch seconds when this device was paired (0 if unknown). */
  added_at: number;
  /** Peer's P2P sync-listener address as "host:port", or null if not yet learned. */
  address: string | null;
  /** Base64 shared content-sync key (not displayed; serde default = null). */
  sync_key_b64: string | null;
  /** Peer's friendly hardware model (e.g. "MacBook Air"), learned in-band during pairing. */
  model: string | null;
  /** Peer's OS name + version (e.g. "macOS 15.5"), learned in-band. */
  os_version: string | null;
  /** Peer's app/daemon version, learned in-band. */
  app_version: string | null;
  /** Peer's best LAN-routable display IP, learned in-band. */
  local_ip: string | null;
  /** Unix epoch seconds of the first successful sync, or null until the first sync. */
  first_sync_at: number | null;
  /** Unix epoch seconds of the most recent successful sync, or null. */
  last_sync_at: number | null;
}

/**
 * Rich identity for THIS device, returned by `get_own_device_info`.
 * All fields except `app_version` are optional — gracefully handle absent ones.
 */
export interface OwnDeviceInfo {
  /** mTLS certificate fingerprint (same as `get_own_fingerprint`). Null when P2P is disabled. */
  fingerprint: string | null;
  /** User-visible device name (e.g. "Dmytro's MacBook Air"). */
  device_name: string | null;
  /** Friendly hardware model (e.g. "MacBook Air", "Mac mini"). */
  device_model: string | null;
  /** OS name + version (e.g. "macOS 15.5"). */
  os_version: string | null;
  /** Daemon / app version (always present). */
  app_version: string;
  /** Best LAN-routable IPv4 address; absent when no real LAN interface. */
  local_ip: string | null;
}

/** Result of `cloud_test_connection` — an end-to-end Supabase probe. */
export interface CloudTestResult {
  /** True when cloud sync is fully reachable and ready. */
  ok: boolean;
  /** Whether Supabase credentials are present at all. */
  configured: boolean;
  /** Which step the probe reached: config/url/auth/network/schema/rls/done. */
  stage: string;
  /** Human-readable, actionable diagnostic. Never contains secrets. */
  message: string;
}

/** Server-enforced page cap; mirrored so the UI can clamp before sending. */
export const MAX_PAGE = 1000;

// ---------------------------------------------------------------------------
// Typed daemon API — one wrapper per IPC method the UI uses.
// ---------------------------------------------------------------------------

export const api = {
  status: () => ipcCall<DaemonStatus>("status"),

  historyPage: (limit: number, offset: number) =>
    ipcCall<HistoryPage>("history_page", { limit: Math.min(limit, MAX_PAGE), offset }),
  copyItem: (id: string) => ipcCall("copy_item", { id }),
  pinItem: (id: string, pinned: boolean) => ipcCall("pin_item", { id, pinned }),
  deleteItem: (id: string) => ipcCall("delete_item", { id }),
  deleteAll: () => ipcCall<{ deleted: number }>("delete_all", {}),

  getConfig: () => ipcCall<AppSettings>("get_config"),
  setConfig: (settings: AppSettings) =>
    ipcCall("set_config", settings as unknown as Record<string, unknown>),
  getPrivateMode: () => ipcCall<{ private_mode: boolean }>("get_private_mode", {}),
  setPrivateMode: (enabled: boolean) =>
    ipcCall<{ private_mode: boolean }>("set_private_mode", { enabled }),

  setSyncPassphrase: (passphrase: string) =>
    ipcCall("set_sync_passphrase", { passphrase }),
  getSyncStatus: () => ipcCall<SyncStatus>("get_sync_status", {}),
  testCloudConnection: () =>
    ipcCall<CloudTestResult>("cloud_test_connection", {}),

  getItemImage: (id: string) => ipcCall<{ data_uri: string }>("get_item_image", { id }),

  /**
   * Fetch the pre-computed thumbnail for a clipboard image item. Returns
   * `{ thumbnail: "data:image/webp;base64,…" }` when the daemon has a thumb,
   * or `{ thumbnail: null }` when thumbnails are not available for this item
   * (older daemon, non-image item, or thumbnail generation failed at capture
   * time). Callers should fall back to `getItemImage` when `thumbnail` is null.
   */
  getItemThumbnail: (id: string) =>
    ipcCall<{ thumbnail: string | null }>("get_item_thumbnail", { id }),

  /**
   * Ask the daemon to resolve a source-app bundle identifier to a 32×32 PNG
   * icon, base64-encoded.  Returns `null` when the app is not installed or
   * the daemon cannot extract the icon.  Results are cached in the daemon so
   * repeated calls for the same bundle ID are fast.
   */
  getAppIcon: (bundleId: string) =>
    ipcCall<{ png_b64: string | null }>("get_app_icon", { bundle_id: bundleId }),

  getOwnFingerprint: () => ipcCall<{ fingerprint: string }>("get_own_fingerprint"),
  /** Rich identity for this device: name, model, OS, version, LAN IP, fingerprint. */
  getOwnDeviceInfo: () => ipcCall<OwnDeviceInfo>("get_own_device_info"),
  /**
   * Ask the daemon for a fresh QR pairing payload. The returned `qr` string is
   * the `copypaste-core` pairing payload (`CPPAIR1.…`) another device scans to
   * pair automatically; `expires_in_secs` is how long the embedded token stays
   * valid. The QR is a transport for the existing PAKE pairing material — no
   * new crypto.
   */
  generatePairingQr: () =>
    ipcCall<{ qr: string; expires_in_secs: number }>("pair_generate_qr", {}),
  listPeers: () => ipcCall<{ peers: PairedDevice[] }>("list_peers"),
  pairWithPassword: (peer_fingerprint: string, password: string) =>
    ipcCall("pair_peer_with_password", { peer_fingerprint, password }),
  unpairPeer: (fingerprint: string) => ipcCall("unpair_peer", { fingerprint }),
  revokePeer: (fingerprint: string) =>
    ipcCall<{ revoked_at: number }>("revoke_peer", { fingerprint }),
  revokeAllPeers: () => ipcCall<{ revoked: number }>("revoke_all_peers", {}),

  /**
   * Persist a new pin order on the daemon. \ must be the complete ordered
   * list of pinned-item IDs (pinned items only, in the desired display order).
   * The daemon stores the order and returns it sorted that way in subsequent
   * \ responses.
   */
  reorderPinned: (ids: string[]) => ipcCall("reorder_pinned", { ids }),
};

/** Format Unix epoch milliseconds for display. */
export function formatWallTime(ms: number): string {
  if (ms <= 0) return "—";
  return new Date(ms).toLocaleString();
}

// ---------------------------------------------------------------------------
// Tauri-direct commands (bypass daemon IPC — talk to the Tauri backend only)
// ---------------------------------------------------------------------------

/**
 * Play a soft system sound on copy — Maccy-style feedback.
 * Calls the `play_copy_sound` Tauri command which plays NSSound "Tink" on
 * macOS. Non-blocking and failure-safe: any error is swallowed by the Rust
 * side; this wrapper also ignores errors so a missing sound never disrupts the
 * copy flow.
 */
export async function playCopySound(): Promise<void> {
  try {
    await invoke<void>("play_copy_sound");
  } catch {
    // Sound is best-effort; never block the copy flow on a sound failure.
  }
}

/**
 * Show a macOS notification banner on copy — Maccy-style feedback.
 * Calls the `show_copy_notification` Tauri command which posts a
 * user-notification via osascript. Non-blocking and failure-safe: any error
 * (missing entitlement, user denied notifications, etc.) is swallowed.
 * @param preview A short one-line preview of the copied item (may be empty).
 */
export async function showCopyNotification(preview: string): Promise<void> {
  try {
    await invoke<void>("show_copy_notification", { preview });
  } catch {
    // Notification is best-effort; never block the copy flow on a notify failure.
  }
}

/**
 * Get the currently configured popup shortcut accelerator string
 * (e.g. "CmdOrCtrl+Shift+V").
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function getPopupShortcut(): Promise<string> {
  return invoke<string>("get_popup_shortcut");
}

/**
 * Set a new popup shortcut accelerator string at runtime and persist it.
 * Throws a plain `Error` with the error message if the accelerator is
 * invalid or already taken by another application.
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function setPopupShortcut(accelerator: string): Promise<void> {
  try {
    await invoke<void>("set_popup_shortcut", { accelerator });
  } catch (e) {
    throw new Error(String(e));
  }
}

/** Result of {@link pairingQrSvg}. */
export interface PairingQr {
  /** Inline SVG markup of the pairing QR code. */
  svg: string;
  /** Raw `CPPAIR1.…` payload string (copy/fallback target). */
  payload: string;
  /** Seconds until the embedded pairing token expires. */
  expires_in_secs: number;
}

/**
 * Generate a scannable pairing QR for this device. The Tauri backend asks the
 * daemon for a fresh pairing token and renders it as an inline SVG. Scanning it
 * from another device pairs automatically. Throws a plain `Error` on failure
 * (e.g. the daemon being offline). This calls the Tauri command directly.
 */
export async function pairingQrSvg(): Promise<PairingQr> {
  try {
    return await invoke<PairingQr>("pairing_qr_svg");
  } catch (e) {
    throw new Error(String(e));
  }
}

/** Reply from the daemon's `reset_database` recovery method. */
export interface ResetDatabaseResult {
  /** Always true on success. */
  reset: boolean;
  /** True when the daemon recovered in-place (no restart needed). */
  ready: boolean;
}

/**
 * Wipe and recreate the daemon's clipboard database (DESTRUCTIVE recovery).
 *
 * This is the escape hatch for a daemon stuck in degraded mode because its
 * database cannot be decrypted. It erases all local clipboard history and
 * creates a fresh empty database; the daemon recovers in-place. The Tauri
 * backend always sends `confirm = true`. Throws a plain `Error` on failure
 * (daemon offline, reset failed) so the caller can surface the real error.
 */
export async function resetDatabase(): Promise<ResetDatabaseResult> {
  let reply: IpcReply;
  try {
    reply = await invoke<IpcReply>("reset_database");
  } catch (e) {
    throw new Error(String(e));
  }
  if (!reply.ok) {
    throw new IpcError(reply.error ?? "reset_database failed", reply.error_code);
  }
  const data = (reply.data ?? {}) as Partial<ResetDatabaseResult>;
  return { reset: data.reset ?? true, ready: data.ready ?? true };
}

// ---------------------------------------------------------------------------
// Daemon UPGRADE/RESTART lifecycle (Tauri-direct — bypass daemon IPC so these
// work even when the daemon is wedged/unresponsive).
// ---------------------------------------------------------------------------

/** The app's own build version (crate version, e.g. "0.5.2"). */
export async function appVersion(): Promise<string> {
  return invoke<string>("app_version");
}

/**
 * Return the last daemon spawn error from the app-owned lifecycle, if any.
 *
 * Returns `null` when the daemon started successfully (or hasn't been
 * attempted yet). Listen for the `"daemon-spawn-result"` Tauri event for
 * real-time feedback; this command is the fallback for views that load after
 * the event fires.
 */
export async function getDaemonError(): Promise<string | null> {
  return invoke<string | null>("get_daemon_error");
}

/**
 * Restart the daemon so the freshly-installed binary takes over.
 *
 * In app-owned mode this stops the tracked child process (SIGTERM + reap) and
 * respawns the bundled binary — no launchctl involved. Throws a plain `Error`
 * with the failure message on error.
 */
export async function restartDaemon(): Promise<void> {
  try {
    await invoke<void>("restart_daemon");
  } catch (e) {
    throw new Error(String(e));
  }
}

/**
 * Parse a semver string into [major, minor, patch] numbers.
 * Returns null if the string cannot be parsed as semver.
 */
function parseSemver(ver: string): [number, number, number] | null {
  const parts = ver.split(".");
  if (parts.length < 3) return null;
  const nums = parts.slice(0, 3).map(Number);
  if (nums.some(isNaN)) return null;
  return nums as [number, number, number];
}

/**
 * Return -1 if a < b, 0 if equal, 1 if a > b (semver comparison).
 */
function compareSemver(
  a: [number, number, number],
  b: [number, number, number]
): -1 | 0 | 1 {
  for (let i = 0; i < 3; i++) {
    if (a[i] < b[i]) return -1;
    if (a[i] > b[i]) return 1;
  }
  return 0;
}

/**
 * Inspect a pre-fetched {@link DaemonStatus} and app version string to decide
 * if the daemon is stale (running an OLDER build than the app). Returns the
 * daemon's reported version string when stale, `"unknown"` when it predates
 * the `build_version` field, or `null` when not stale (same version, daemon
 * is NEWER, or comparison isn't possible).
 *
 * Only flags as stale when the daemon is strictly OLDER — a daemon that is
 * NEWER than the app (e.g. the user rolled back) is not flagged so the banner
 * doesn't appear in that direction.
 */
export function detectStaleDaemonFromStatus(
  status: Partial<DaemonStatus> | null,
  appVer: string
): string | null {
  if (!status) return null;
  const reported = status.build_version ?? null;
  // No version field => daemon predates this build => stale by definition.
  if (reported === null || reported === "") return "unknown";
  const reportedPrefix = reported.split("+")[0];
  if (reportedPrefix === appVer) return null;
  // Parse both as semver to determine direction.
  const daemonParsed = parseSemver(reportedPrefix);
  const appParsed = parseSemver(appVer);
  if (!daemonParsed || !appParsed) {
    // Cannot parse — fall back to string inequality: flag as stale when different.
    return reportedPrefix !== appVer ? reported : null;
  }
  const cmp = compareSemver(daemonParsed, appParsed);
  // Only stale when daemon is strictly OLDER (cmp === -1).
  return cmp === -1 ? reported : null;
}

/**
 * Compare the running daemon's build to the app's own. Returns the daemon's
 * version when it is STALE (survived an upgrade — strictly OLDER semver),
 * else `null`.
 *
 * Only flags as stale when the daemon is strictly OLDER than the app. A daemon
 * that is NEWER (e.g. after a rollback) is NOT flagged. Best-effort: any error
 * (e.g. daemon offline) yields `null` so callers never block startup on this check.
 */
export async function detectStaleDaemon(): Promise<string | null> {
  let appVer: string;
  let status: DaemonStatus;
  try {
    [appVer, status] = await Promise.all([appVersion(), api.status()]);
  } catch {
    return null;
  }
  return detectStaleDaemonFromStatus(status, appVer);
}

// ---------------------------------------------------------------------------
// Accessibility permission (macOS only — always true on other platforms)
// ---------------------------------------------------------------------------

/**
 * Check whether the macOS Accessibility permission is granted for this app.
 * Returns `true` on non-macOS platforms (no permission needed there).
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function checkAccessibilityPermission(): Promise<boolean> {
  return invoke<boolean>("check_accessibility_permission");
}

/**
 * Open System Settings → Privacy & Security → Accessibility and attempt to
 * (re-)install the CGEventTap if permission was just granted.
 * No-op on non-macOS platforms.
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function requestAccessibilityPermission(): Promise<void> {
  await invoke<void>("request_accessibility_permission");
}

/**
 * Format a Unix timestamp in seconds (as stored in `PairedDevice.added_at`)
 * for human-readable display. Returns "—" for falsy/zero values.
 */
export function formatEpochSecs(secs: number | null | undefined): string {
  if (!secs) return "—";
  return new Date(secs * 1000).toLocaleString();
}

// ---------------------------------------------------------------------------
// Log viewer commands (Tauri-direct — bypass daemon IPC)
// ---------------------------------------------------------------------------

/**
 * Read the last `maxLines` lines from the daemon log files in
 * ~/Library/Logs/CopyPaste/. Returns the log content as a single string.
 */
export async function readLogs(maxLines: number): Promise<string> {
  return invoke<string>("read_logs", { maxLines });
}

/**
 * Return the log directory path (~/Library/Logs/CopyPaste on macOS).
 */
export async function logDirPath(): Promise<string> {
  return invoke<string>("log_dir_path");
}
