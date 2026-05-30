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
}

export interface HistoryPage {
  items: HistoryEntry[];
  total: number;
}

export interface AppSettings {
  p2p_enabled: boolean;
  supabase_url: string | null;
  supabase_anon_key: string | null;
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
  /**
   * Optional, best-effort degraded-state flags the daemon MAY report when it is
   * up but cannot do crypto/storage (e.g. the macOS Keychain is locked or the
   * SQLCipher DB could not be opened). Treated as optional: the UI surfaces a
   * banner when present but never depends on them.
   */
  keychain_locked?: boolean;
  db_unavailable?: boolean;
  /** Optional human-readable degraded-state reason, if the daemon supplies one. */
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
  status: () => ipcCall("status"),

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
  setPrivateMode: (enabled: boolean) => ipcCall("set_private_mode", { enabled }),

  setSyncPassphrase: (passphrase: string) =>
    ipcCall("set_sync_passphrase", { passphrase }),
  getSyncStatus: () => ipcCall<SyncStatus>("get_sync_status", {}),
  testCloudConnection: () =>
    ipcCall<CloudTestResult>("cloud_test_connection", {}),

  getItemImage: (id: string) => ipcCall<{ data_uri: string }>("get_item_image", { id }),

  getOwnFingerprint: () => ipcCall<{ fingerprint: string }>("get_own_fingerprint"),
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
  revokeAllPeers: () => ipcCall<{ revoked: number }>("revoke_all_peers", {})
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
