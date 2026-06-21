// ---------------------------------------------------------------------------
// Mock-IPC gate — activated by VITE_MOCK=1 (env) or ?mock=1 (URL query param).
// When active, all invoke() calls are handled by the in-process mockInvoke()
// fixture so the full UI renders in a plain browser with no Tauri runtime.
// The real (non-mock) path is COMPLETELY unchanged — this is a dead-code branch
// in production builds where VITE_MOCK is not "1".
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Mock-IPC gate (DEV-only): the ?mock=1 escape hatch is only honoured in
// development / test builds. In production (import.meta.env.DEV === false)
// the entire branch is dead code — Rollup/Vite tree-shakes mockIpc.ts out of
// the bundle entirely, so fixture data (developer email, fixture secrets) never
// ships to end-users. The runtime ?mock=1 URL gate is also only active in DEV,
// preventing production webview users from swapping in the fixture harness.
// ---------------------------------------------------------------------------

import { invoke as tauriInvoke } from "@tauri-apps/api/core";

type InvokeFn = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

// `invoke` and `MOCK` are resolved below via a DEV-gated block. Both branches
// always assign them, so TypeScript can prove they are initialised before use.
let invoke: InvokeFn;
let MOCK: boolean;

if (import.meta.env.DEV) {
  // DEV / test: honour VITE_MOCK=1 (build-time) or ?mock=1 (runtime URL).
  const mockRequested =
    (import.meta.env?.VITE_MOCK === "1") ||
    (typeof window !== "undefined" &&
      new URLSearchParams(window.location.search).has("mock"));

  if (mockRequested) {
    // Dynamic import keeps mockIpc.ts out of the production module graph.
    // Top-level await is valid here: "module": "ESNext" + "moduleResolution":
    // "bundler" enable top-level await in TypeScript ESM modules.
    // Vite statically replaces import.meta.env.DEV with `false` in prod, so
    // this entire `if` block — including the dynamic import string — is dead
    // code that Rollup eliminates before bundling.
    const { mockInvoke } = await import("./mockIpc");
    MOCK = true;
    invoke = (cmd, args) => mockInvoke(cmd, args) as Promise<never>;
  } else {
    MOCK = false;
    invoke = tauriInvoke;
  }
} else {
  // Production: always the real Tauri bridge. mockIpc.ts is never referenced.
  MOCK = false;
  invoke = tauriInvoke;
}

export { MOCK };

/**
 * IPC wire protocol version this UI build was compiled against (ADR-007).
 * Bump this when a breaking wire change is shipped alongside a UI update.
 * The daemon emits `protocol_version` on every response; when the daemon's
 * version exceeds this value the client should surface an upgrade prompt.
 */
export const CURRENT_PROTOCOL_VERSION = 1;

/**
 * Optional callback invoked when a daemon response carries a `protocol_version`
 * that differs from {@link CURRENT_PROTOCOL_VERSION}. The default handler
 * emits a `console.warn`. Replace this at startup (e.g. in App.tsx) to surface
 * a richer banner instead.
 *
 * Signature: `(daemonVersion: number) => void`
 */
export let protocolMismatchHandler: ((daemonVersion: number) => void) | null = null;

/**
 * Replace the module-level {@link protocolMismatchHandler}. Call once at
 * app startup (App.tsx) to wire in a UI banner instead of the default
 * `console.warn`. Pass `null` to restore the default warn-only behaviour.
 */
export function setProtocolMismatchHandler(
  handler: ((daemonVersion: number) => void) | null
): void {
  protocolMismatchHandler = handler;
}

/** Raw daemon reply, mirrored from the Rust `ipc_call` bridge. */
export interface IpcReply {
  ok: boolean;
  data: unknown | null;
  error: string | null;
  error_code: string | null;
  /**
   * Wire protocol version the daemon speaks (ADR-007). Optional for back-compat
   * with daemon builds predating the field. The Tauri bridge (`src-tauri/src/ipc.rs`)
   * forwards this field; absent only on pre-ABI-17 daemon builds where the bridge
   * did not yet forward it — those arrive as `undefined`.
   */
  protocol_version?: number;
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
 * ro0r: Exponential-backoff retry parameters for `migration_in_progress`.
 *
 * When the daemon replies with error_code "migration_in_progress" the DB
 * migration is briefly in flight and the request should be retried shortly.
 * We retry up to MAX_MIGRATION_RETRIES times with the backoff schedule below
 * (250 ms → 500 ms → 1000 ms → 2000 ms → 2000 ms …) before giving up and
 * re-throwing the original IpcError so the caller sees it.
 *
 * Only "migration_in_progress" is retried — all other error codes propagate
 * immediately. This is intentional: retrying arbitrary errors would mask bugs
 * and create unpredictable behaviour.
 */
const MAX_MIGRATION_RETRIES = 5;
const MIGRATION_BASE_DELAY_MS = 250;
const MIGRATION_MAX_DELAY_MS = 2000;

function migrationDelay(attempt: number): Promise<void> {
  // Exponential backoff: 250, 500, 1000, 2000, 2000, …
  const ms = Math.min(
    MIGRATION_BASE_DELAY_MS * Math.pow(2, attempt),
    MIGRATION_MAX_DELAY_MS
  );
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Call a daemon method over the Unix-socket bridge. Resolves to the daemon's
 * `data` payload on success; throws `IpcError` on a daemon error and on a
 * transport failure (e.g. the daemon being offline -> code "daemon_offline").
 *
 * Per ADR-007, checks the daemon's `protocol_version` on every reply. When the
 * daemon speaks a version higher than {@link CURRENT_PROTOCOL_VERSION} the
 * client may be unable to handle future field changes — a warning is surfaced
 * via {@link protocolMismatchHandler} (defaults to `console.warn`).
 *
 * ro0r: When the daemon replies with error_code "migration_in_progress", the
 * call is automatically retried with exponential backoff (up to 5 attempts,
 * 250 ms → 2 s cap) before propagating the error. No other error codes are
 * retried — only "migration_in_progress".
 */
export async function ipcCall<T = unknown>(
  method: string,
  params?: Record<string, unknown>
): Promise<T> {
  // ro0r: retry loop for migration_in_progress (only). All other errors fall
  // through immediately. `attempt` starts at 0; the loop runs until we either
  // succeed, hit an unretriable error, or exhaust MAX_MIGRATION_RETRIES.
  for (let attempt = 0; ; attempt++) {
    let reply: IpcReply;
    try {
      reply = await invoke<IpcReply>("ipc_call", { method, params: params ?? null });
    } catch (e) {
      // Transport-level failures come back as a string like "daemon_offline:/path".
      const raw = String(e);
      const code = raw.split(":", 1)[0] || null;
      throw new IpcError(raw, code);
    }

    // ADR-007: check protocol version on every reply. The field is optional
    // because (a) the Tauri bridge did not forward it before this fix and (b)
    // old daemon builds predate the field — both cases arrive as `undefined`,
    // which we treat as "no mismatch detected" rather than a false alarm.
    const daemonVersion = reply.protocol_version;
    if (daemonVersion !== undefined && daemonVersion !== CURRENT_PROTOCOL_VERSION) {
      const handler = protocolMismatchHandler;
      if (handler !== null) {
        handler(daemonVersion);
      } else {
        console.warn(
          `[copypaste] IPC protocol version mismatch: daemon speaks v${daemonVersion}, ` +
          `client expects v${CURRENT_PROTOCOL_VERSION}. ` +
          "Please upgrade the CopyPaste app or restart the daemon."
        );
      }
    }

    if (!reply.ok) {
      // Also fire protocolMismatchHandler on the daemon's explicit version-mismatch
      // error code ("n" / "version_mismatch"). This covers older daemons that reject
      // the request before emitting the protocol_version field in the reply.
      const code = reply.error_code ?? null;
      // ERR_CODE_VERSION_MISMATCH is the string "version_mismatch" (verified in
      // protocol.rs / copypaste-ipc error.rs). The earlier "n" alias was dead
      // code — "n" is the redacted-secret field marker, not an error code.
      if (code === "version_mismatch") {
        const handler = protocolMismatchHandler;
        if (handler !== null) {
          // Pass CURRENT_PROTOCOL_VERSION + 1 as a sentinel so the handler knows
          // the daemon is ahead (exact daemon version not available at error time).
          handler(CURRENT_PROTOCOL_VERSION + 1);
        } else {
          console.warn(
            "[copypaste] Daemon rejected request due to protocol version mismatch. " +
            "Please upgrade the CopyPaste app or restart the daemon."
          );
        }
      }

      // ro0r: retry only on migration_in_progress — a transient state where the
      // daemon's SQLite migration is briefly in flight. All other error codes
      // are not retried and propagate to the caller immediately.
      if (code === "migration_in_progress" && attempt < MAX_MIGRATION_RETRIES) {
        await migrationDelay(attempt);
        continue; // retry the request
      }

      throw new IpcError(reply.error ?? "unknown daemon error", code);
    }
    return reply.data as T;
  }
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
  /**
   * True when the daemon flagged this item as exceeding the configured sync
   * size cap, so it is kept locally but will not be synced to other devices.
   * Optional for back-compat with older daemon builds that don't emit it.
   * Arrives as snake_case `too_large_to_sync` in the daemon JSON.
   */
  too_large_to_sync?: boolean;
  /**
   * Stable device UUID of the machine that originally captured this item.
   * Empty string for pre-v3 rows that were never backfilled. Compare against
   * `HistoryPage.own_device_id` to determine whether an item is local.
   * Optional for back-compat with daemon builds predating this field.
   */
  origin_device_id?: string;
  /**
   * Human-readable name of the device that originally captured this item,
   * as stored in the local `devices` table.  `null` when the device was
   * never paired on this machine (e.g. an item received from a third device
   * that was not directly paired here) or for pre-v3 rows with an empty
   * `origin_device_id`.  Optional for back-compat with older daemon builds
   * that do not emit this field.
   */
  origin_device_name?: string | null;
  /**
   * Refined content-kind label computed by the daemon's core text-kind
   * classifier. Values for text items: "TEXT" | "URL" | "EMAIL" | "PHONE" |
   * "COLOR" | "JSON" | "CODE" | "NUMBER" | "PATH". Non-text: "IMAGE" | "FILE".
   * Optional for back-compat — older daemon builds do not emit this field.
   * The UI falls back to a content_type-derived label when absent.
   */
  kind?: string;
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
  /**
   * This daemon's own stable device UUID. Compare each item's `origin_device_id`
   * against this to label locally-captured items as "This device".
   * Empty string on daemon builds that predate this field.
   */
  own_device_id: string;
}

export interface AppSettings {
  /**
   * j9xj (PG-30): Master sync kill-switch — Android parity.
   * When false, ALL sync transports (P2P, Supabase cloud, relay) are disabled
   * regardless of their individual settings. When true (default), individual
   * transport switches apply as normal.
   *
   * Implemented end-to-end: `AppConfig::sync_enabled` exists in
   * `crates/copypaste-core/src/config/mod.rs` (default `true`); the daemon
   * gates P2P, relay, and Supabase transports on this flag at startup and on
   * hot-reload. Contract tests live in `crates/copypaste-daemon/src/daemon.rs`.
   *
   * `null` / absent = preserve stored value. Maps to `AppConfig::sync_enabled`.
   */
  sync_enabled?: boolean | null;
  p2p_enabled: boolean;
  supabase_url: string | null;
  supabase_anon_key: string | null;
  /** HTTP relay base URL for store-and-forward sync. Non-secret; surfaced
   *  verbatim by get_config. null / absent on set_config preserves the stored
   *  value. */
  relay_url?: string | null;
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
  /**
   * Whether the daemon may make a one-off STUN request to discover the device's
   * public IP. `null` / absent = preserve stored value. Maps to
   * `AppConfig::collect_public_ip` in the daemon.
   */
  collect_public_ip?: boolean | null;
  /**
   * When true, paste-back strips all rich clipboard types and writes plain
   * text only. `null` / absent = preserve stored value. Maps to
   * `AppConfig::paste_as_plain_text`.
   */
  paste_as_plain_text?: boolean | null;
  /**
   * Bundle IDs of apps whose clipboard copies are silently skipped (macOS).
   * `null` / absent = preserve stored value; an explicit `[]` clears the list.
   * Maps to `AppConfig::excluded_app_bundle_ids`.
   */
  excluded_app_bundle_ids?: string[] | null;
  /**
   * Whether this device advertises itself via mDNS-SD and browses for peers on
   * the local network. `false` = LAN-invisible (stealth mode). `null` / absent
   * = preserve stored value. Default: `true`. Maps to
   * `AppConfig::lan_visibility` in core config.toml.
   */
  lan_visibility?: boolean | null;
  /**
   * When true (daemon default), incoming synced clipboard items are automatically
   * written to the local system clipboard so the device is always up-to-date.
   * When false, synced items are stored in history but never applied to the
   * active clipboard without an explicit paste action. `null` / absent = preserve
   * stored value. Maps to `AppConfig::auto_apply_synced_clip` in copypaste-core.
   */
  auto_apply_synced_clip?: boolean | null;
  /**
   * True when a Supabase GoTrue email is stored in the daemon's config.
   * The daemon never returns the email itself — only this presence flag.
   * Read-only from `get_config`; ignored when `set_config` omits it.
   * Maps to `AppConfig::supabase_email` after redaction in `redact_config_secrets`.
   */
  supabase_email_set?: boolean;
  /**
   * True when a Supabase GoTrue password is stored in the daemon's config.
   * The daemon never returns the password itself — only this presence flag.
   * Read-only from `get_config`; ignored when `set_config` omits it.
   * Maps to `AppConfig::supabase_password` after redaction in `redact_config_secrets`.
   */
  supabase_password_set?: boolean;
  /**
   * Supabase GoTrue email for email+password sign-in. Write-only: sent via
   * `set_config` to the daemon; never returned by `get_config` (only the
   * `supabase_email_set` presence flag is returned). The daemon persists it
   * in config.toml. `null` / absent = preserve stored value.
   */
  supabase_email?: string | null;
  /**
   * Supabase GoTrue password for email+password sign-in. Write-only: sent via
   * `set_config` to the daemon; never returned by `get_config` (only the
   * `supabase_password_set` presence flag is returned). Never logged or echoed.
   * `null` / absent = preserve stored value.
   */
  supabase_password?: string | null;
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

/**
 * CopyPaste-merc: canonical sync-badge state computed once by the daemon.
 *
 * Mirrors `copypaste_ipc::SyncBadgeState` (snake_case wire names). When this
 * field is present in a `get_sync_status` response, consumers MUST use it
 * directly and MUST NOT re-derive the badge colour from raw fields (`last_sync_ms`,
 * `supabase_configured`, etc.). The daemon is the single source of truth.
 *
 * A thin fallback to the local `deriveSyncState` is permitted ONLY when the
 * field is absent (daemons predating this field will not include it).
 */
export type SyncBadgeState =
  | "synced"
  | "syncing"
  | "idle"
  | "offline"
  | "error"
  | "misconfigured";

export interface SyncStatus {
  passphrase_set: boolean;
  supabase_configured: boolean;
  /**
   * Presence flags for the GoTrue account credentials (PG-13 / jhvl). The daemon
   * redacts `supabase_email`/`supabase_password` to `n: bool` on read; these expose
   * that "is set" state so SettingsView can show a "set ✓" hint without ever
   * receiving the secret. Optional: undefined when the daemon does not surface them
   * (the hint is simply hidden). For the hint to display, the daemon's
   * get_sync_status (or the redacted config) must carry these flags.
   */
  supabase_email_set?: boolean;
  supabase_password_set?: boolean;
  signed_in: boolean;
  /** Unix epoch milliseconds of last sync, or null if never synced. */
  last_sync_ms: number | null;
  /** Supabase project URL, if configured via env or settings. */
  supabase_url?: string | null;
  /** Signed-in account email, if available. */
  email?: string | null;
  /**
   * CopyPaste-merc: canonical badge state, daemon-computed.
   *
   * When present, render this directly — do NOT re-derive from raw fields.
   * Absent on daemons predating this field; fall back to local derivation then.
   */
  badge_state?: SyncBadgeState | null;
}


/**
 * One peer seen on the LAN via mDNS-SD, as returned by `list_discovered`.
 * Field names mirror the daemon's `list_discovered` response exactly.
 */
export interface DiscoveredDevice {
  /** The peer's mDNS `did` — its canonical certificate fingerprint. */
  device_id: string;
  /** User-visible device name advertised over mDNS. */
  device_name: string;
  /** Resolved IP addresses (IPv4-first), as strings. */
  ip_addrs: string[];
  /** P2P sync-listener port. */
  port: number;
  /**
   * Bootstrap port for SAS pairing. `null` on v1 peers that don't advertise
   * one — the UI disables "Pair" in that case.
   */
  bport: number | null;
  /** True when this device's fingerprint already matches a paired entry. */
  paired: boolean;
}

/**
 * State of the discovery-initiated SAS pairing state machine, returned by
 * `pair_get_sas`. `state` values mirror the daemon's wire strings exactly.
 */
export type PairingSasState =
  | "idle"
  | "initiating"
  | "awaiting_sas"
  | "confirmed"
  | "rejected"
  | "aborted"
  | "timed_out";

/** Reply from the daemon's `pair_get_sas` poll. */
export interface PairSasStatus {
  state: PairingSasState;
  /** 6 decimal digits — present only when `state === "awaiting_sas"`. */
  sas?: string;
  /** "initiator" | "responder" — present only mid-pairing. */
  role?: string;
  // Peer identity fields — included when the daemon has learned them during
  // the pairing handshake. All optional for back-compat: older daemon builds
  // omit them; the UI shows what is available and falls back gracefully.
  // model/OS/app_version are surfaced post-SAS in the pair_with_discovered
  // response, not here.
  /** Peer's user-visible device name (legacy field name; see peer_device_name). */
  peer_name?: string | null;
  /** Peer's hardware model (e.g. "Pixel 8 Pro") — post-SAS only. */
  peer_model?: string | null;
  /** Peer's OS name + version (e.g. "Android 15") — post-SAS only. */
  peer_os?: string | null;
  /** Peer's app/daemon version — post-SAS only. */
  peer_app_version?: string | null;
  /** Peer's best LAN-routable IP address (legacy single-IP field). */
  peer_ip?: string | null;
  /**
   * Peer's human-readable device name, advertised over mDNS before dialling.
   * Present on the initiator path; absent on the responder path (inbound
   * connection — no prior mDNS resolution).
   */
  peer_device_name?: string;
  /** Peer's resolved IP addresses (IPv4-first), from mDNS. Initiator path only. */
  peer_ip_addrs?: string[];
  /**
   * Peer's certificate fingerprint (the mDNS `device_id` = hex SHA-256).
   * Present on the initiator path; absent on the responder path.
   */
  peer_fingerprint?: string;
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
  /**
   * Peer's public (WAN) IPv4 address, persisted + returned by the daemon
   * (peers.rs + list_peers passthrough). Null/absent until learned.
   */
  public_ip?: string | null;
  /** Unix epoch seconds of the first successful sync, or null until the first sync. */
  first_sync_at: number | null;
  /** Unix epoch seconds of the most recent successful sync, or null. */
  last_sync_at: number | null;
  /**
   * Whether the peer is currently reachable/online as reported by the daemon.
   * Optional for back-compat with older daemons that don't emit this field.
   */
  online?: boolean;
  /**
   * Seconds since this peer was last seen (large / absent = never seen).
   * Optional for back-compat with older daemons that don't emit this field.
   */
  last_seen_secs?: number;
  /**
   * Round-trip time in milliseconds to this peer over the mTLS P2P connection.
   * Absent when no live P2P connection exists or the daemon has not yet measured it.
   */
  latency_ms?: number;
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
  /**
   * Public (WAN) IPv4 address as seen by the relay, e.g. "203.0.113.42".
   * Null when the daemon hasn't fetched it yet or the lookup failed.
   * NOTE: peer cards do NOT yet show public IP — needs a PeerMeta proto bump
   * (daemon follow-up); only the "This device" card shows it for now.
   */
  public_ip?: string | null;
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

/** Format Unix epoch milliseconds for display. */
export function formatWallTime(ms: number): string {
  if (ms <= 0) return "—";
  return new Date(ms).toLocaleString();
}

/**
 * Returns true when a daemon content_type represents an image — either the
 * bare legacy token "image" or any MIME-typed "image/*" variant.
 * Single source of truth shared by HistoryView, Popup, and notification logic.
 */
export function isImageType(ct: string): boolean {
  return ct === "image" || ct.startsWith("image/");
}

/**
 * Extract a human-readable message from a caught error, falling back to
 * `fallback` when the error is not an IpcError (e.g. a transport TypeError).
 * Use at every catch site instead of the repeated ternary.
 *
 * @param err      The value caught in a catch clause (type `unknown`).
 * @param fallback Fallback string when `err` is not an IpcError.
 */
export function ipcErrorMessage(err: unknown, fallback: string): string {
  return err instanceof IpcError ? err.message : fallback;
}

/**
 * Map a caught error to a safe, user-facing string that NEVER leaks socket
 * paths, usernames, or internal Rust error strings into the DOM.
 *
 * Rules:
 *  - Known IPC error codes → canonical friendly copy.
 *  - Unknown IpcError      → "Something went wrong." (code is logged, not rendered).
 *  - Non-IpcError          → "Something went wrong." (never serialises raw transport
 *                            strings that may contain socket paths or usernames).
 *
 * Use this wherever `err.message` or `String(err)` would otherwise be rendered
 * as visible text. Console-logging the raw error for diagnostics is fine —
 * just never put it in the DOM.
 */
export function friendlyIpcError(err: unknown): string {
  if (!(err instanceof IpcError)) {
    // Non-IpcError (e.g. TypeError, transport string) — never leak internals.
    return "Something went wrong.";
  }
  switch (err.code) {
    case "daemon_offline":
      return "The background service is not running.";
    case "ipc_not_ready":
    case "IPC_NOT_READY":
      return "The background service is starting up. Please wait a moment.";
    case "not_found":
    case "NotFound":
      return "The requested item was not found.";
    case "permission_denied":
    case "PermissionDenied":
      return "Permission denied.";
    case "migration_in_progress":
      return "A database migration is in progress. Please wait.";
    case "version_mismatch":
      return "The app and background service are on incompatible versions. Please restart.";
    case "rate_limited":
      return "Too many requests. Please wait and try again.";
    default:
      // Unknown code — return generic copy. Do NOT include err.message: it may
      // contain socket paths or other internal strings.
      return "Something went wrong.";
  }
}

/**
 * Returns true when the error represents the daemon being alive but not yet
 * ready to serve requests (e.g. still initialising its database). Daemon
 * error code: `"ipc_not_ready"` or the legacy uppercase variant
 * `"IPC_NOT_READY"`. Views should show a friendly "starting up" state rather
 * than a hard error when this returns true.
 */
export function isIpcNotReady(err: unknown): boolean {
  if (!(err instanceof IpcError)) return false;
  return err.code === "ipc_not_ready" || err.code === "IPC_NOT_READY";
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
 * Show a rich macOS notification banner on copy via UNUserNotificationCenter.
 * Derives a human-readable title ("Text Copied" / "Image Copied" / "File
 * Copied") and a preview body (first ~160 chars of text, filename for files,
 * "Image" for images) from the item's content type and preview string, then
 * calls the `show_copy_notification` Tauri command which posts it from inside
 * the CopyPaste.app bundle so the banner shows the app icon.
 *
 * Non-blocking and failure-safe: any error is swallowed.
 *
 * @param contentType The daemon content type: "text" | "image" | "file" | "".
 * @param preview     The raw preview string from the daemon (may be empty).
 */
export async function showCopyNotification(
  contentType: string,
  preview: string
): Promise<void> {
  const { title, body } = buildNotificationContent(contentType, preview);
  try {
    await invoke<void>("show_copy_notification", { title, body });
  } catch {
    // Notification is best-effort; never block the copy flow on a notify failure.
  }
}

/** Build notification title + body from daemon content_type + preview. */
function buildNotificationContent(
  contentType: string,
  preview: string
): { title: string; body: string } {
  if (contentType === "text") {
    return { title: "Text Copied", body: truncatePreviewBody(preview) || "Copied" };
  }
  if (contentType === "image" || contentType.startsWith("image/")) {
    return { title: "Image Copied", body: "Image" };
  }
  if (contentType === "file") {
    // Daemon preview is "[file: <filename>]" — strip the wrapper.
    const inner = preview.replace(/^\[file:\s*/, "").replace(/\]$/, "").trim();
    return { title: "File Copied", body: inner || "File" };
  }
  // Fallback / unknown content type.
  return { title: "Copied", body: truncatePreviewBody(preview) || "Copied" };
}

/**
 * Truncate a raw text preview to ~160 chars at a word boundary with a
 * trailing `…`.  Preserves newlines so multi-line text reads naturally.
 */
function truncatePreviewBody(preview: string): string {
  const MAX = 160;
  const s = preview.trim();
  if (s.length <= MAX) return s;
  const cut = s.slice(0, MAX);
  const wordBoundary = Math.max(cut.lastIndexOf(" "), cut.lastIndexOf("\n"));
  const chopped = wordBoundary > 0 ? cut.slice(0, wordBoundary).trimEnd() : cut.trimEnd();
  return chopped + "…";
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
 * Get the built-in default popup shortcut accelerator string from the Rust
 * layer (currently "CmdOrCtrl+Shift+V").
 *
 * CopyPaste-sqw0: this is the authoritative source of the default.  Rust's
 * `DEFAULT_POPUP_SHORTCUT` constant in `src-tauri/src/lib.rs` is the single
 * source of truth.  `SettingsView.tsx` fetches this at load time via
 * `getDefaultPopupShortcut()` and uses it for the "reset to default" button,
 * so the two sides can never drift silently.
 *
 * This calls the Tauri command directly, NOT the daemon IPC socket.
 */
export async function getDefaultPopupShortcut(): Promise<string> {
  return invoke<string>("get_default_popup_shortcut");
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

/**
 * Bring the main CopyPaste window to the foreground.
 *
 * Used when an incoming pairing request arrives on the responder side so the
 * user sees the SAS confirmation modal without having to open the app manually.
 * Non-blocking and failure-safe: any error is swallowed.
 */
export async function focusMainWindow(): Promise<void> {
  try {
    await invoke<void>("focus_main_window");
  } catch {
    // Best-effort — never block the pairing flow on a window-focus failure.
  }
}

/**
 * Fire a macOS notification informing the user that a remote device is
 * requesting to pair. Reuses the existing UNUserNotificationCenter path via
 * `show_copy_notification` so no new Tauri command is needed.
 *
 * Non-blocking and failure-safe.
 *
 * @param peerName  User-visible name of the peer requesting to pair.
 */
export async function showPairingRequestNotification(peerName: string): Promise<void> {
  const title = "CopyPaste: Pairing Request";
  const body = `"${peerName}" wants to pair — open CopyPaste to confirm.`;
  try {
    await invoke<void>("show_copy_notification", { title, body });
  } catch {
    // Best-effort — notification failure must never break the pairing flow.
  }
}

/**
 * Write `text` as plain UTF-8 to the system clipboard (no rich formatting),
 * then activate the prior app and synthesise Cmd+V.
 *
 * This is the backend for the Option+Enter "paste as plain text" shortcut (F1).
 * The caller must hide the popup BEFORE calling this so the prior app receives
 * focus before the synthetic Cmd+V fires.
 *
 * On non-macOS this is a no-op.
 */
export async function pasteAsPlainText(text: string): Promise<void> {
  await invoke<void>("paste_plain_text", { text });
}

// ---------------------------------------------------------------------------
// CopyPaste-6uy9: allow-screenshots / content-protection toggle
// ---------------------------------------------------------------------------

/**
 * Return the current allow-screenshots preference.
 * `true` = screenshots allowed (content protection disabled).
 * `false` = content protection ON (default — PG-25 behaviour).
 */
export async function getAllowScreenshots(): Promise<boolean> {
  return invoke<boolean>("get_allow_screenshots");
}

/**
 * Enable or disable screenshot / screen-recording protection for all windows.
 *
 * `allow = true`  — disables NSWindowSharingNone so screen-capture tools
 *                   can capture CopyPaste windows.
 * `allow = false` — re-enables protection (PG-25 default).
 *
 * The preference is persisted to `ui-config.json` and applied immediately
 * to all open windows without a restart.
 */
export async function setAllowScreenshots(allow: boolean): Promise<void> {
  await invoke<void>("set_allow_screenshots", { allow });
}
