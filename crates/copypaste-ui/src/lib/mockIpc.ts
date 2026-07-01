/**
 * Browser mock-IPC harness (CopyPaste-v57).
 *
 * Activated when VITE_MOCK=1 (env) or ?mock=1 (URL query param).
 * Replaces all `invoke()` calls so the full UI renders in a plain browser
 * with no daemon, no Tauri runtime, and no native bridge.
 *
 * SHAPE CONTRACT:
 *  - `cmd === "ipc_call"` → `args.method` switch → returns IpcReply so that
 *    `ipcCall()` can unwrap it exactly as with a real daemon response.
 *  - All other commands (Tauri-direct) → sensible fixtures returned directly.
 */

import type {
  DaemonStatus,
  HistoryPage,
  HistoryEntry,
  AppSettings,
  SyncStatus,
  PairSasStatus,
  CloudTestResult,
  IpcReply,
} from "./ipc";
// Shared fixture factories (design.md Decision 7/G3, task 6.5) — the single
// source of truth for entity-shaped sample data, also consumed by
// GalleryView/**. See fixtures/index.ts for the import-boundary rule this
// module is one of the two allowed consumers of.
import {
  makeHistoryEntry,
  makeDevice,
  makeDiscoveredDevice,
  makeOwnDeviceInfo,
  makePairStatus,
  FIXTURE_OWN_DEVICE_ID as OWN_DEVICE_ID,
  FIXTURE_OWN_FINGERPRINT as OWN_FINGERPRINT,
  mins,
  hours,
  days,
} from "./fixtures";

// ---------------------------------------------------------------------------
// Fixture: clipboard history — 14 items covering every kind
// ---------------------------------------------------------------------------

const HISTORY_ITEMS: HistoryEntry[] = [
  // 1. Pinned — plain text, just now, local
  makeHistoryEntry({
    id: "item-001",
    content_type: "text",
    preview: "Meeting at 3 PM — don't forget to bring the Q3 report.",
    is_sensitive: false,
    wall_time: mins(1),
    pinned: true,
    kind: "TEXT",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.apple.Notes",
  }),

  // 2. URL, 5 minutes ago, from Chrome
  makeHistoryEntry({
    id: "item-002",
    content_type: "text",
    preview: "https://github.com/gastownhall/copypaste/pull/142",
    is_sensitive: false,
    wall_time: mins(5),
    pinned: false,
    kind: "URL",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.google.Chrome",
  }),

  // 3. Email address, 12 minutes ago
  makeHistoryEntry({
    id: "item-003",
    content_type: "text",
    preview: "alice.wonderland@example.com",
    is_sensitive: false,
    wall_time: mins(12),
    pinned: false,
    kind: "EMAIL",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.apple.mail",
  }),

  // 4. Phone number, sensitive, 20 minutes ago
  makeHistoryEntry({
    id: "item-004",
    content_type: "text",
    preview: "+1 (555) 867-5309",
    is_sensitive: true,
    sensitive_spans: [[0, 18]],
    wall_time: mins(20),
    pinned: false,
    kind: "PHONE",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.apple.MobileSMS",
  }),

  // 5. Hex colour, 35 minutes ago
  makeHistoryEntry({
    id: "item-005",
    content_type: "text",
    preview: "#6C47FF",
    is_sensitive: false,
    wall_time: mins(35),
    pinned: false,
    kind: "COLOR",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.figma.Desktop",
  }),

  // 6. Number / amount, 1 hour ago
  makeHistoryEntry({
    id: "item-006",
    content_type: "text",
    preview: "42_000_000",
    is_sensitive: false,
    wall_time: hours(1),
    pinned: false,
    kind: "NUMBER",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: null,
  }),

  // 7. File path, 2 hours ago
  makeHistoryEntry({
    id: "item-007",
    content_type: "text",
    preview:
      "/Users/dmytro/Documents/CopyPaste/crates/copypaste-ui/src/lib/ipc.ts",
    is_sensitive: false,
    wall_time: hours(2),
    pinned: false,
    kind: "PATH",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.microsoft.VSCode",
  }),

  // 8. JSON blob, 3 hours ago
  makeHistoryEntry({
    id: "item-008",
    content_type: "text",
    preview:
      '{"user":{"id":7,"name":"Alice","roles":["admin","editor"]},"ts":1718000000}',
    is_sensitive: false,
    wall_time: hours(3),
    pinned: false,
    kind: "JSON",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.microsoft.VSCode",
  }),

  // 9. Code snippet, 5 hours ago
  makeHistoryEntry({
    id: "item-009",
    content_type: "text",
    preview:
      "export const sum = (a: number, b: number): number => a + b;\n// Unit-tested",
    is_sensitive: false,
    wall_time: hours(5),
    pinned: false,
    kind: "CODE",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.microsoft.VSCode",
  }),

  // 10. Long multiline text, 1 day ago
  makeHistoryEntry({
    id: "item-010",
    content_type: "text",
    preview:
      "Lorem ipsum dolor sit amet, consectetur adipiscing elit.\n" +
      "Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\n" +
      "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.",
    is_sensitive: false,
    wall_time: days(1),
    pinned: false,
    kind: "TEXT",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.apple.Safari",
  }),

  // 11. Image — from another device (MacBook Pro)
  makeHistoryEntry({
    id: "item-011",
    content_type: "image/png",
    preview: "[image]",
    is_sensitive: false,
    wall_time: hours(4),
    pinned: false,
    kind: "IMAGE",
    origin_device_id: "bbccddee-aaaa-bbbb-cccc-ddeeff001122",
    origin_device_name: "MacBook Pro",
    app_bundle_id: null,
  }),

  // 12. File attachment — from iPhone
  makeHistoryEntry({
    id: "item-012",
    content_type: "file",
    preview: "[file: Q3_Report_2026.pdf]",
    is_sensitive: false,
    wall_time: days(2),
    pinned: false,
    kind: "FILE",
    origin_device_id: "ccddeeff-bbbb-cccc-dddd-eeff00112233",
    origin_device_name: "iPhone 16 Pro",
    app_bundle_id: null,
    too_large_to_sync: false,
  }),

  // 13. Sensitive/private item with sensitive spans, 3 days ago
  makeHistoryEntry({
    id: "item-013",
    content_type: "text",
    preview: "Password: Hunter2!@#  (internal wiki)",
    is_sensitive: true,
    sensitive_spans: [[10, 20]],
    wall_time: days(3),
    pinned: false,
    kind: "TEXT",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.1password.1password-7",
  }),

  // 14. API key — sensitive, 4 days ago
  makeHistoryEntry({
    id: "item-014",
    content_type: "text",
    preview: "sk-proj-AAABBBCCCDDDEEEFFFGGGHHHIIIJJJKKKLLL",
    is_sensitive: true,
    sensitive_spans: [[0, 43]],
    wall_time: days(4),
    pinned: false,
    kind: "TEXT",
    origin_device_id: OWN_DEVICE_ID,
    origin_device_name: null,
    app_bundle_id: "com.openai.chat",
    too_large_to_sync: false,
  }),
];

// ---------------------------------------------------------------------------
// Fixture: paired devices
// ---------------------------------------------------------------------------

const PAIRED_DEVICES = [
  makeDevice({
    fingerprint:
      "bbccddee112233445566778899aabbccddeeff0011223344556677889900aabb",
    name: "MacBook Pro",
    added_at: Math.floor(days(90) / 1000),
    address: "192.168.1.42:7878",
    sync_key_b64: null,
    model: "MacBook Pro 16-inch (2023)",
    os_version: "macOS 15.5",
    app_version: "0.7.1",
    local_ip: "192.168.1.42",
    public_ip: "203.0.113.10",
    first_sync_at: Math.floor(days(89) / 1000),
    last_sync_at: Math.floor(hours(2) / 1000),
    online: true,
    last_seen_secs: 4,
    latency_ms: 12,
  }),
  makeDevice({
    fingerprint:
      "ccddeeff223344556677889900aabbccddeeff001122334455667788990011cc",
    name: "iPhone 16 Pro",
    added_at: Math.floor(days(45) / 1000),
    address: null,
    sync_key_b64: null,
    model: "iPhone 16 Pro",
    os_version: "iOS 18.5",
    app_version: "0.7.0",
    local_ip: "192.168.1.55",
    public_ip: null,
    first_sync_at: Math.floor(days(44) / 1000),
    last_sync_at: Math.floor(hours(8) / 1000),
    online: false,
    last_seen_secs: 29_200,
    latency_ms: undefined,
  }),
  makeDevice({
    fingerprint:
      "ddeeff334455667788990011aabbccddeeff0011223344556677889900ddeeff",
    name: "iPad Pro",
    added_at: Math.floor(days(7) / 1000),
    address: "192.168.1.77:7878",
    sync_key_b64: null,
    model: "iPad Pro 13-inch (M4)",
    os_version: "iPadOS 18.5",
    app_version: "0.7.1",
    local_ip: "192.168.1.77",
    public_ip: null,
    first_sync_at: Math.floor(days(6) / 1000),
    last_sync_at: Math.floor(mins(45) / 1000),
    online: true,
    last_seen_secs: 18,
    latency_ms: 28,
  }),
];

// ---------------------------------------------------------------------------
// Fixture: discovered LAN devices
// ---------------------------------------------------------------------------

const DISCOVERED_DEVICES = [
  makeDiscoveredDevice({
    device_id:
      "eeff005566778899aabbccddeeff001122334455667788990011aabb00eeff11",
    device_name: "Dmytro's Mac mini",
    ip_addrs: ["192.168.1.100"],
    port: 7878,
    bport: 7879,
    paired: false,
  }),
  makeDiscoveredDevice({
    device_id:
      "ff00116677889900aabbccddeeff001122334455667788990011aabbccff0022",
    device_name: "Work MacBook Air",
    ip_addrs: ["192.168.1.120"],
    port: 7878,
    bport: null, // v1 peer — Pair button should be disabled
    paired: false,
  }),
];

// ---------------------------------------------------------------------------
// Fixture: daemon status — ONLINE / HEALTHY (no banners)
// ---------------------------------------------------------------------------

const DAEMON_STATUS: DaemonStatus = {
  status: "running",
  private_mode: false,
  ready: true,
  degraded: false,
  degraded_reason: null,
  build_version: "0.7.1",
  pid: 12345,
};

// ---------------------------------------------------------------------------
// Fixture: app settings
// ---------------------------------------------------------------------------

const APP_SETTINGS: AppSettings = {
  p2p_enabled: true,
  supabase_url: "https://xyzxyzxyz.supabase.co",
  supabase_anon_key: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.mock",
  relay_url: "https://relay.copypaste.app",
  max_text_size_bytes: 5_242_880,
  max_image_size_bytes: 20_971_520,
  max_file_size_bytes: 52_428_800,
  storage_quota_bytes: 1_073_741_824,
  sensitive_ttl_secs: 86_400,
  sync_on_wifi_only: false,
  sound_on_copy: true,
  notify_on_copy: false,
  collect_public_ip: false, // am9w: opt-out default, matches daemon #[serde(default)]
  paste_as_plain_text: false,
  excluded_app_bundle_ids: ["com.apple.Passwords"],
  lan_visibility: true,
  supabase_email_set: true,
  supabase_password_set: true,
};

// ---------------------------------------------------------------------------
// Fixture: sync status
// ---------------------------------------------------------------------------

const SYNC_STATUS: SyncStatus = {
  passphrase_set: true,
  supabase_configured: true,
  signed_in: true,
  last_sync_ms: mins(45),
  supabase_url: "https://xyzxyzxyz.supabase.co",
  email: "dmitriy.evseev.99@gmail.com",
};

// ---------------------------------------------------------------------------
// Fixture: own device info
// ---------------------------------------------------------------------------

const OWN_DEVICE_INFO = makeOwnDeviceInfo({
  fingerprint: OWN_FINGERPRINT,
  device_name: "Dmytro's MacBook Air",
  device_model: "MacBook Air 15-inch (M3)",
  os_version: "macOS 15.5",
  app_version: "0.7.1",
  local_ip: "192.168.1.50",
  public_ip: "203.0.113.42",
});

// ---------------------------------------------------------------------------
// Fixture: SAS pairing state (idle — no modal on load)
// ---------------------------------------------------------------------------

const SAS_STATUS: PairSasStatus = makePairStatus();

// ---------------------------------------------------------------------------
// Fixture: cloud test
// ---------------------------------------------------------------------------

const CLOUD_TEST: CloudTestResult = {
  ok: true,
  configured: true,
  stage: "done",
  message: "Cloud sync is fully operational. Last sync: < 1 minute ago.",
};

// ---------------------------------------------------------------------------
// Fixture: pairing QR SVG (minimal well-formed inline SVG)
// ---------------------------------------------------------------------------

const PAIRING_QR_SVG =
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 21 21" width="200" height="200" shape-rendering="crispEdges">` +
  `<rect width="21" height="21" fill="white"/>` +
  `<rect x="0" y="0" width="7" height="7" fill="black"/>` +
  `<rect x="1" y="1" width="5" height="5" fill="white"/>` +
  `<rect x="2" y="2" width="3" height="3" fill="black"/>` +
  `<rect x="14" y="0" width="7" height="7" fill="black"/>` +
  `<rect x="15" y="1" width="5" height="5" fill="white"/>` +
  `<rect x="16" y="2" width="3" height="3" fill="black"/>` +
  `<rect x="0" y="14" width="7" height="7" fill="black"/>` +
  `<rect x="1" y="15" width="5" height="5" fill="white"/>` +
  `<rect x="2" y="16" width="3" height="3" fill="black"/>` +
  `<rect x="9" y="0" width="1" height="1" fill="black"/>` +
  `<rect x="9" y="2" width="3" height="1" fill="black"/>` +
  `<rect x="7" y="9" width="7" height="1" fill="black"/>` +
  `<rect x="9" y="11" width="3" height="3" fill="black"/>` +
  `<rect x="14" y="9" width="7" height="3" fill="black"/>` +
  `</svg>`;

// ---------------------------------------------------------------------------
// IpcReply builder — shapes reply so ipcCall() unwraps correctly:
//   if (!reply.ok) throw IpcError else return reply.data as T
// ---------------------------------------------------------------------------

function ok(data: unknown): IpcReply {
  return {
    ok: true,
    data,
    error: null,
    error_code: null,
    protocol_version: 1,
  };
}

// ---------------------------------------------------------------------------
// Delay helper — makes fixtures feel slightly async (like real IPC)
// ---------------------------------------------------------------------------

function delay(ms = 30): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// ---------------------------------------------------------------------------
// Mock invoke — the single replacement for Tauri's invoke()
// ---------------------------------------------------------------------------

export async function mockInvoke(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<unknown> {
  await delay();

  // -------------------------------------------------------------------------
  // ipc_call — all daemon methods (routed through Unix socket in real mode)
  // -------------------------------------------------------------------------
  if (cmd === "ipc_call") {
    const typedArgs = args as { method: string; params?: Record<string, unknown> };
    const method = typedArgs.method;
    const params = typedArgs.params ?? {};

    switch (method) {
      // daemon health
      case "status":
        return ok(DAEMON_STATUS);

      // clipboard history
      case "history_page": {
        const limit = (params.limit as number | undefined) ?? 50;
        const offset = (params.offset as number | undefined) ?? 0;
        const slice = HISTORY_ITEMS.slice(offset, offset + limit);
        const page: HistoryPage = {
          items: slice,
          total: HISTORY_ITEMS.length,
          own_device_id: OWN_DEVICE_ID,
        };
        return ok(page);
      }

      case "copy_item":
        return ok({ ok: true });

      case "pin_item":
        return ok({ ok: true });

      case "delete_item":
        return ok({ ok: true });

      case "delete_all":
        return ok({ deleted: 0 });

      case "reorder_pinned":
        return ok({ ok: true });

      // full-text search
      case "search": {
        const q = ((params.query as string | undefined) ?? "").toLowerCase();
        const matches = HISTORY_ITEMS
          .filter((i) => i.preview.toLowerCase().includes(q))
          .map((i) => ({ id: i.id }));
        return ok({ items: matches });
      }

      // config
      case "get_config":
        return ok(APP_SETTINGS);

      case "set_config":
        return ok({ ok: true });

      case "get_private_mode":
        return ok({ private_mode: false });

      case "set_private_mode":
        return ok({ private_mode: (params.enabled as boolean | undefined) ?? false });

      // sync / passphrase
      case "set_sync_passphrase":
        return ok({ ok: true });

      case "rotate_sync_key":
        return ok({ ok: true, rotated: true });

      case "revoke_and_rotate":
        return ok({
          revoked_at: Math.floor(Date.now() / 1000),
          rotated: true,
        });

      case "get_sync_status":
        return ok(SYNC_STATUS);

      case "cloud_test_connection":
        return ok(CLOUD_TEST);

      // image / file items
      case "get_item_image":
        return ok({
          // 1×1 transparent PNG
          data_uri:
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
        });

      case "get_item_thumbnail":
        return ok({
          thumbnail:
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
        });

      case "get_item_file":
        return ok({
          filename: "Q3_Report_2026.pdf",
          mime: "application/pdf",
          data_b64: "JVBERi0xLjQ=",
        });

      case "add_file_item":
        return ok({ id: `item-mock-file-${Date.now()}` });

      // app icon
      case "get_app_icon":
        return ok({ png_b64: null });

      // devices / pairing
      case "get_own_fingerprint":
        return ok({ fingerprint: OWN_FINGERPRINT });

      case "get_own_device_info":
        return ok(OWN_DEVICE_INFO);

      case "pair_generate_qr":
        return ok({
          qr: "CPPAIR1.mock.payload.aabbccdd",
          expires_in_secs: 300,
        });

      case "list_peers":
        return ok({ peers: PAIRED_DEVICES });

      case "poll_peer_events":
        return ok({ events: [] });

      case "list_discovered":
        return ok({ devices: DISCOVERED_DEVICES });

      case "rescan_discovered":
        return ok({ devices: DISCOVERED_DEVICES });

      case "pair_with_discovered":
        return ok({ ok: true });

      case "pair_get_sas":
        return ok(SAS_STATUS);

      case "pair_confirm_sas":
        return ok({ ok: true, accepted: true });

      case "pair_abort":
        return ok({ ok: true });

      case "pair_peer_with_password":
        return ok({ ok: true });

      case "unpair_peer":
        return ok({ ok: true });

      case "revoke_peer":
        return ok({ revoked_at: Math.floor(Date.now() / 1000) });

      case "revoke_all_peers":
        return ok({ revoked: 0 });

      // ---------------------------------------------------------------------------
      // 85n9: Backup / Restore — export and import
      // ---------------------------------------------------------------------------

      case "export": {
        const includeSensitive = (params.include_sensitive as boolean | undefined) ?? false;
        const exportItems = includeSensitive
          ? HISTORY_ITEMS
          : HISTORY_ITEMS.filter((i) => !i.is_sensitive);
        // Return the minimal export shape the daemon produces (content_bytes_b64 is
        // base64 of the raw plaintext; we use a fixed stub here for the fixture).
        const items = exportItems.map((i) => ({
          content_type: i.content_type,
          content_bytes_b64: btoa(i.preview),
          created_at_ms: i.wall_time,
          metadata: null,
        }));
        return ok({ items });
      }

      case "import":
        // The daemon returns inserted/skipped counts after dedup. Fixture returns
        // the full length as inserted (no dedup in mock).
        return ok({
          inserted: ((params.items as unknown[] | undefined) ?? []).length,
          skipped: 0,
        });

      // ---------------------------------------------------------------------------
      // gq51: Database maintenance
      // ---------------------------------------------------------------------------

      case "vacuum":
        // Simulate a successful vacuum (no-op in the mock).
        return ok({ ok: true });

      case "db_stats":
        return ok({
          item_count: HISTORY_ITEMS.length,
          // Approximate: 4 KB per item as a rough fixture.
          size_bytes: HISTORY_ITEMS.length * 4096,
        });

      default:
        console.warn("[mock-ipc] unhandled ipc_call method:", method);
        return ok(null);
    }
  }

  // -------------------------------------------------------------------------
  // Tauri-direct commands
  // -------------------------------------------------------------------------

  switch (cmd) {
    // App lifecycle
    case "app_version":
      return "0.7.1";

    case "get_daemon_error":
      // null = daemon started fine, no error banner
      return null;

    case "restart_daemon":
      return undefined;

    case "reset_database":
      // resetDatabase() unwraps IpcReply manually — return the right shape
      return {
        ok: true,
        data: { reset: true, ready: true },
        error: null,
        error_code: null,
      } satisfies IpcReply;

    // Accessibility
    case "check_accessibility_permission":
      // true = permission granted, no banner shown
      return true;

    case "request_accessibility_permission":
      return undefined;

    // Popup shortcut
    case "get_popup_shortcut":
      return "CmdOrCtrl+Shift+V";

    // CopyPaste-sqw0: exposes the Rust DEFAULT_POPUP_SHORTCUT constant so TS
    // never hardcodes it independently.  Must match lib.rs:DEFAULT_POPUP_SHORTCUT.
    case "get_default_popup_shortcut":
      return "CmdOrCtrl+Shift+V";

    case "set_popup_shortcut":
      return undefined;

    // Pairing QR SVG (Tauri backend renders the SVG, not the daemon)
    case "pairing_qr_svg":
      return {
        svg: PAIRING_QR_SVG,
        payload: "CPPAIR1.mock.payload.aabbccdd",
        expires_in_secs: 300,
      };

    // Sound / notification — fire-and-forget, best-effort
    case "play_copy_sound":
      return undefined;

    case "show_copy_notification":
      return undefined;

    // File opening
    case "open_item_file":
      return undefined;

    // Paste
    case "paste_plain_text":
      return undefined;

    // Popup-specific commands
    case "hide_popup":
      return undefined;

    case "paste_to_frontmost":
      return undefined;

    // Logs
    case "read_logs":
      return [
        "2026-06-14T10:00:00Z INFO  copypaste_daemon: daemon started, version=0.7.1",
        "2026-06-14T10:00:01Z INFO  copypaste_daemon::ipc: Unix socket listener ready",
        "2026-06-14T10:00:02Z INFO  copypaste_daemon::sync: P2P listener bound on :7878",
        "2026-06-14T10:01:15Z INFO  copypaste_daemon::clipboard: captured text item id=item-001",
        "2026-06-14T10:02:30Z INFO  copypaste_daemon::sync: peer connected fingerprint=bbccddee…",
        "2026-06-14T10:02:31Z DEBUG copypaste_daemon::sync: synced 3 items with peer MacBook Pro",
        "2026-06-14T10:05:00Z INFO  copypaste_daemon::clipboard: captured url item id=item-002",
      ].join("\n");

    case "log_dir_path":
      return "/Users/dmytro/Library/Logs/CopyPaste";

    // Window management
    case "focus_main_window":
      return undefined;

    // CopyPaste-6uy9: allow-screenshots preference (mock: protection ON by default)
    case "get_allow_screenshots":
      return false;

    case "set_allow_screenshots":
      return undefined;

    default:
      console.warn("[mock-ipc] unhandled Tauri command:", cmd);
      return undefined;
  }
}
