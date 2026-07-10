// ---------------------------------------------------------------------------
// lib/ipc/types.ts — all shared daemon wire types (zero runtime code).
// Consumed via the barrel at lib/ipc.ts; import consumers are unchanged.
// ---------------------------------------------------------------------------

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
}

/**
 * Normalized result of probing the daemon status. `kind` collapses the three
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
  /**
   * CopyPaste-1jms.34: canonical Supabase account identity for this device.
   *
   * Derived from the Supabase project URL + GoTrue user UUID by the daemon via
   * `copypaste_supabase::supabase_account_id`. Two paired devices MUST share the
   * same value for Supabase RLS to let them see each other's rows. A mismatch
   * means they are on different Supabase projects or different GoTrue accounts —
   * their clipboard rows are silently invisible to each other.
   *
   * This is a non-secret stable identifier (not a token/key). Absent/null when
   * cloud-sync is off, not configured, or anon-key-only (no GoTrue session).
   * Absent on daemons predating this field — treat absence as null.
   *
   * Peer `supabase_account_id` is now plumbed through the `list_peers` response
   * (CopyPaste-yw2k). The UI compares this local value against each peer's
   * `supabase_account_id` in `useSettingsState` to set `cloudAccountMismatch`.
   */
  supabase_account_id?: string | null;
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
  /**
   * CopyPaste-ptgcc: current rekey-failure count for this peer's pairwise
   * sync key, as tracked by the daemon's outbound fanout loop. Present only
   * when P2P is running AND at least one rekey failure has been recorded
   * since daemon start; absent means either no failures or an older daemon
   * that predates this field. A non-zero value means this device cannot
   * currently encrypt outbound items for this peer — a stronger signal than
   * `last_sync_at` staleness alone.
   */
  rekey_failures?: number;
  /**
   * CopyPaste-1jms.30: trust level as reported by the daemon's list_peers response.
   * "verified" = peer completed SAS confirmation; any other value (or absent) = not
   * SAS-verified. Optional for back-compat with daemon builds predating this field.
   * Mirrors Android's trustLabel(peer): Verified iff sasVerified, else Unverified.
   */
  trust?: string;
  /**
   * CopyPaste-1jms.32: transport kind used for the most recent sync with this peer.
   * "p2p"      = direct mTLS P2P connection (currently live).
   * "relay"    = HTTP relay store-and-forward inbox.
   * "supabase" = Supabase cloud backend.
   * null / absent = unknown (no transport active, or daemon predates this field).
   * When absent, the UI falls back to the local_ip/address heuristic.
   */
  transport?: "p2p" | "relay" | "supabase" | null;
  /**
   * CopyPaste-yw2k: peer's stable, non-secret Supabase account identity.
   *
   * Derived by the peer from `copypaste_supabase::supabase_account_id(url, user_id)`
   * and exchanged in-band over the bootstrap channel at pairing time. Two paired
   * devices MUST share the same value for Supabase RLS to let them see each other's
   * rows. A mismatch means they are on different Supabase projects or different
   * GoTrue accounts.
   *
   * Absent/null when the peer is a legacy build that doesn't carry this field,
   * or when cloud-sync is not configured on the peer side. Use this to set
   * `cloudAccountMismatch` in `useSettingsState`.
   */
  supabase_account_id?: string | null;
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

/** Reply from the daemon's `reset_database` recovery method. */
export interface ResetDatabaseResult {
  /** Always true on success. */
  reset: boolean;
  /** True when the daemon recovered in-place (no restart needed). */
  ready: boolean;
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
