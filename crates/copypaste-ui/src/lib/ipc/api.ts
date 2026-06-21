// ---------------------------------------------------------------------------
// lib/ipc/api.ts — typed daemon API (socket-bridge wrappers).
// All calls go through ipcCall() over the Unix socket owned by copypaste-daemon.
// ---------------------------------------------------------------------------

import { ipcCall } from "./transport";
import { invoke } from "./transport";
import type {
  DaemonStatus,
  HistoryPage,
  AppSettings,
  SyncStatus,
  CloudTestResult,
  DiscoveredDevice,
  PairSasStatus,
  PairedDevice,
  OwnDeviceInfo,
} from "./types";

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
  /**
   * Rotate the shared cloud/relay sync key to a NEW passphrase. This is the
   * only honest cloud/relay device revocation: the old key (still held by a
   * revoked device) can no longer decrypt items produced after the rotation,
   * and the relay inbox id (HKDF of the key) diverges so the revoked device's
   * saved token addresses a dead inbox. Remaining devices must re-provision
   * (re-scan the pairing QR or re-enter the new passphrase) to keep syncing.
   */
  rotateSyncKey: (passphrase: string) =>
    ipcCall<{ ok: boolean; rotated: boolean }>("rotate_sync_key", { passphrase }),
  /**
   * Revoke a peer from P2P AND rotate the sync key in one call, cutting the
   * revoked device off from cloud/relay sync too. Requires the new passphrase;
   * the daemon derives the key first, so a bad passphrase fails before any
   * revocation state is mutated.
   */
  revokeAndRotate: (fingerprint: string, passphrase: string) =>
    ipcCall<{ revoked_at: number; rotated: boolean }>("revoke_and_rotate", {
      fingerprint,
      passphrase,
    }),
  getSyncStatus: () => ipcCall<SyncStatus>("get_sync_status", {}),
  testCloudConnection: () =>
    ipcCall<CloudTestResult>("cloud_test_connection", {}),

  getItemImage: (id: string) => ipcCall<{ data_uri: string }>("get_item_image", { id }),

  /**
   * Fetch the full binary payload for a `content_type === "file"` clipboard item.
   * Returns `{ filename, mime, data_b64 }` where `data_b64` is standard base64.
   * Throws `IpcError` when the item is not found or is not a file item.
   */
  getItemFile: (id: string) =>
    ipcCall<{ filename: string; mime: string; data_b64: string }>("get_item_file", { id }),

  /**
   * Open a file-type clipboard item with the OS default application.
   *
   * The Tauri backend fetches the file bytes from the daemon, writes them to a
   * temp file under `$TMPDIR/copypaste_open/`, and calls the OS open command
   * (`/usr/bin/open` on macOS, `xdg-open` on Linux).  The temp file is not
   * cleaned up automatically — it lives until the next system temp-dir purge.
   *
   * Use this for "Open" (open-in-place, no save dialog) as distinct from
   * `getItemFile` which triggers a browser download ("Save As…").
   *
   * Throws `IpcError` when the item is not found, not a file, or the OS open
   * command fails.
   */
  openItemFile: (id: string) => invoke<void>("open_item_file", { id }),

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

  /**
   * Drain all pending peer connect/disconnect events since the last call.
   * Returns an empty array when nothing changed.  Called by the Tauri event
   * bridge roughly every second; individual UI components subscribe to the
   * `usePeerPresence` store instead of calling this directly.
   */
  pollPeerEvents: () =>
    ipcCall<{ events: Array<{ kind: "connected" | "disconnected"; fingerprint: string }> }>(
      "poll_peer_events",
      {},
    ),

  /** List peers currently visible on the LAN via mDNS-SD. */
  listDiscovered: () =>
    ipcCall<{ devices: DiscoveredDevice[] }>("list_discovered", {}),
  /**
   * HB-9: force an mDNS-SD rescan (restart-in-place re-browse) and return the
   * fresh discovered list. Same response shape as {@link listDiscovered}.
   */
  rescanDiscovered: () =>
    ipcCall<{ devices: DiscoveredDevice[] }>("rescan_discovered", {}),
  /**
   * Begin a discovery-initiated SAS pairing with `deviceId` (the discovered
   * peer's `device_id`). Returns immediately; poll {@link pairGetSas} for the
   * SAS. Throws `IpcError` with code `rate_limited` when another pairing is
   * already in progress.
   */
  pairWithDiscovered: (deviceId: string) =>
    ipcCall("pair_with_discovered", { device_id: deviceId }),
  /** Poll the discovery-pairing state machine. */
  pairGetSas: () => ipcCall<PairSasStatus>("pair_get_sas", {}),
  /** Deliver the local user's SAS accept (true) / reject (false) decision. */
  pairConfirmSas: (accept: boolean) =>
    ipcCall<{ ok: boolean; accepted: boolean }>("pair_confirm_sas", { accept }),
  /** Abort an in-flight discovery pairing and reset the machine to idle. */
  pairAbort: () => ipcCall<{ ok: boolean }>("pair_abort", {}),
  /**
   * Reset the pairing state machine to idle without cancelling an in-flight
   * handshake. Safe to call from any state — in particular AFTER a terminal
   * outcome (confirmed / aborted / timed_out / rejected) so the machine is
   * ready for the next LAN pairing attempt. (bd CopyPaste-1jms.3 / 1jms.12)
   *
   * Difference from pairAbort: `pair_abort` also signals the remote peer;
   * `pair_reset` is a local-only reset used once the handshake has already
   * ended and we just want the SM back at idle.
   */
  pairReset: () => ipcCall<{ ok: boolean }>("pair_reset", {}),
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

  /**
   * Ingest a file into the clipboard history directly from the UI.
   *
   * The caller provides the raw file bytes already read (e.g. via the browser
   * File API from an `<input type="file">` picker or the drag-drop handler),
   * the filename, and an optional MIME type.  The bytes are base64-encoded and
   * forwarded to the daemon's `add_file_item` method which encrypts and stores
   * them exactly like a pasteboard-captured file.
   *
   * Returns `{ id: string }` on success. Throws `IpcError` on failure.
   */
  addFileItem: (
    bytes: Uint8Array,
    filename: string,
    mime = "application/octet-stream"
  ): Promise<{ id: string }> => {
    // Chunk-based btoa avoids RangeError ("Maximum call stack size exceeded")
    // when spreading a large Uint8Array into String.fromCharCode (C3).
    const data_b64 = (() => {
      let bin = "";
      const CHUNK = 8192;
      for (let i = 0; i < bytes.length; i += CHUNK) {
        bin += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
      }
      return btoa(bin);
    })();
    return ipcCall<{ id: string }>("add_file_item", { filename, mime, data_b64 });
  },

  /**
   * Full-text search over the daemon's FTS5 index (all stored items, not just
   * the loaded page). Returns up to `limit` matching item IDs. Use the returned
   * IDs as a set to augment the client-side substring filter in HistoryView so
   * items beyond the loaded page are discoverable.
   */
  searchItems: (query: string, limit = 500): Promise<{ id: string }[]> =>
    ipcCall<{ items: { id: string }[] }>("search", { query, limit }).then(
      (r) => r.items ?? []
    ),

  // ---------------------------------------------------------------------------
  // 85n9: Backup / Restore — export and import clipboard history as JSON
  // ---------------------------------------------------------------------------

  /**
   * Export clipboard history from the daemon.
   *
   * The daemon verb is "export". Params:
   *   limit            — max items to export (0 = no cap; daemon default is 0).
   *   include_sensitive — when true, sensitive items are included in plaintext;
   *                       when false (default) sensitive items are omitted.
   *
   * The reply `data` is the bulk JSON object `{ items: [...] }` where each item
   * carries `content_type`, `content_bytes_b64`, `created_at_ms`, and `metadata`.
   * This is the exact shape the daemon's `import` verb expects on round-trip.
   *
   * The UI triggers a browser download of the JSON via Blob + anchor — no fs
   * capability is needed (same pattern as FileChip's triggerDownload).
   *
   * Throws IpcError when the daemon does not support the export verb (older
   * daemon), or on any other daemon error.
   */
  exportItems: (
    includeSensitive = false,
    limit = 0,
  ): Promise<{ items: unknown[] }> =>
    ipcCall<{ items: unknown[] }>("export", {
      limit,
      include_sensitive: includeSensitive,
    }),

  /**
   * Import clipboard items into the daemon.
   *
   * The daemon verb is "import". Params:
   *   items — the array from a previous exportItems() call (or a compatible
   *            JSON export file). The daemon deduplicates and recomputes
   *            sensitivity on each item (CopyPaste-vuxs).
   *
   * Returns `{ inserted: number; skipped: number }` on success.
   * Throws IpcError on failure.
   */
  importItems: (
    items: unknown[],
  ): Promise<{ inserted: number; skipped: number }> =>
    ipcCall<{ inserted: number; skipped: number }>("import", { items }),

  // ---------------------------------------------------------------------------
  // gq51: Database maintenance — vacuum and stats
  // ---------------------------------------------------------------------------

  /**
   * Run `VACUUM` on the SQLite/SQLCipher database to compact free pages and
   * reclaim disk space. Long-running (can take several seconds on large DBs);
   * the UI should show a busy indicator.
   *
   * Returns `{ ok: true }` on success. Throws IpcError on failure.
   */
  vacuum: (): Promise<{ ok: boolean }> =>
    ipcCall<{ ok: boolean }>("vacuum", {}),

  /**
   * Fetch storage statistics for the local clipboard database.
   *
   * Returns:
   *  - item_count  — total number of items stored (all pages).
   *  - size_bytes  — approximate on-disk size of the database file in bytes.
   *
   * Throws IpcError when the daemon does not support the verb (older daemon).
   */
  getDbStats: (): Promise<{ item_count: number; size_bytes: number }> =>
    ipcCall<{ item_count: number; size_bytes: number }>("db_stats", {}),
};
