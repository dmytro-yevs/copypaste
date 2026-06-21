//! Method-name constants and typed request/response payloads for individual
//! IPC methods.
//!
//! The daemon dispatches on the bare method-name string (see
//! `copypaste-daemon::ipc`), so these constants are the single shared source of
//! truth for the wire-level method name. Clients (UI, CLI) reference the
//! constant instead of re-typing the string literal, so a rename is a
//! compile-time break rather than a silent runtime mismatch.

use serde::{Deserialize, Serialize};

// ── Core clipboard methods ──────────────────────────────────────────────────

/// Fetch a paginated list of clipboard items.
pub const METHOD_LIST: &str = "list";

/// Fetch one page of clipboard history items (with pagination).
///
/// Params: `{ limit: u32, offset: u32 }`.  Response: `{ items: […], total: u32,
/// own_device_id: String }`.  The UI uses this (not the legacy `list`) for the
/// paginated history view so it can load incrementally.
pub const METHOD_HISTORY_PAGE: &str = "history_page";

/// Full-text search over clipboard items.
pub const METHOD_SEARCH: &str = "search";

/// Copy a clipboard item back to the system clipboard by id.
///
/// Params: `{ id: String }`.  Different from the legacy `copy` method in that
/// it uses the item's stable UUID rather than an integer index, and returns a
/// richer response (including the decrypted text for paste-back).
pub const METHOD_COPY_ITEM: &str = "copy_item";

/// Copy a clipboard item back to the system clipboard by id.
pub const METHOD_COPY: &str = "copy";

/// Delete a single clipboard item by id.
///
/// Params: `{ id: String }`.  Deletes the item with the given UUID from the
/// encrypted database and removes it from the FTS index.
pub const METHOD_DELETE_ITEM: &str = "delete_item";

/// Delete a single clipboard item by id.
pub const METHOD_DELETE: &str = "delete";

/// Delete all clipboard items (clear history).
pub const METHOD_DELETE_ALL: &str = "delete_all";

/// Return the total count of stored clipboard items.
pub const METHOD_COUNT: &str = "count";

/// Return aggregate statistics about the clipboard database.
///
/// This is the CLI diagnostic view. The UI uses [`METHOD_DB_STATS`] instead,
/// which returns only `{item_count, size_bytes}` for the settings panel.
/// These two methods are intentionally distinct: `stats` is richer and intended
/// for human-readable terminal output; `db_stats` is minimal and typed for the
/// UI's storage summary widget (c4q2.23).
///
/// Params: none (empty `{}`).
/// Response: [`StatsResponse`].
pub const METHOD_STATS: &str = "stats";

/// Success payload for [`METHOD_STATS`].
///
/// All counts are live values from the encrypted database at the time of the
/// call. The daemon serialises this struct directly, so field names are the
/// stable wire contract between the daemon and the CLI (c4q2.23 — formerly an
/// ad-hoc `serde_json::json!({...})` with no typed schema).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StatsResponse {
    /// Total number of clipboard items (all rows, including sensitive ones).
    pub total_items: i64,
    /// Number of items flagged as sensitive (password / secret patterns).
    pub sensitive_items: i64,
    /// IPC/schema version string (not a semver release; currently `"1"`).
    pub version: String,
    /// Daemon build version string (`env!("CARGO_PKG_VERSION")`).
    pub build_version: String,
}

// ── Daemon health ───────────────────────────────────────────────────────────

/// Query the running daemon's health / readiness state.
pub const METHOD_STATUS: &str = "status";

// ── Import / export ─────────────────────────────────────────────────────────

/// Export clipboard items as a JSON blob.
pub const METHOD_EXPORT: &str = "export";

/// Bulk-import clipboard items from a JSON blob.
pub const METHOD_IMPORT: &str = "import";

// ── Pinning ─────────────────────────────────────────────────────────────────

/// Pin or unpin a clipboard item (takes `{id, pinned: bool}`).
pub const METHOD_PIN_ITEM: &str = "pin_item";

// ── Private / pause mode ────────────────────────────────────────────────────

/// Enable or disable clipboard recording pause mode.
pub const METHOD_SET_PRIVATE_MODE: &str = "set_private_mode";

/// Query the current private-mode state.
pub const METHOD_GET_PRIVATE_MODE: &str = "get_private_mode";

// ── Sync key management ─────────────────────────────────────────────────────

/// Store the shared sync passphrase and derive the content-sync key from it.
///
/// Params: `{ passphrase: String }`.  The daemon stores the key material in the
/// Keychain (macOS) or in-memory; the passphrase itself is never persisted.
pub const METHOD_SET_SYNC_PASSPHRASE: &str = "set_sync_passphrase";

/// Rotate the shared content-sync key to a new passphrase.
///
/// Params: `{ passphrase: String }`.  After rotation the old key is zeroized;
/// previously paired devices that haven't re-provisioned can no longer decrypt
/// new items.  Returns `{ ok: bool, rotated: bool }`.
pub const METHOD_ROTATE_SYNC_KEY: &str = "rotate_sync_key";

/// Revoke a peer from P2P AND rotate the sync key in one atomic call.
///
/// Params: `{ fingerprint: String, passphrase: String }`.  The daemon derives
/// the new key first (bad passphrase → fail before any state is mutated) then
/// removes the peer from `peers.json` and rotates the key.
/// Returns `{ revoked_at: i64, rotated: bool }`.
pub const METHOD_REVOKE_AND_ROTATE: &str = "revoke_and_rotate";

// ── Cloud sync ──────────────────────────────────────────────────────────────

/// Read the current daemon configuration object.
pub const METHOD_GET_CONFIG: &str = "get_config";

/// Write / merge a partial daemon configuration object.
pub const METHOD_SET_CONFIG: &str = "set_config";

/// Store the Supabase GoTrue account password directly in the macOS Keychain
/// (or an in-memory fallback on non-macOS) **without** routing it through
/// `set_config` and **without** persisting it to `config.json`.
///
/// # Why a dedicated verb?
///
/// `set_config` carries the password in the JSON payload which travels over
/// the Unix socket and is briefly held in the daemon's request-buffer memory.
/// Although the socket is `0600` and the memory is ephemeral, the password
/// would also have appeared in `config.json` on any platform where the Keychain
/// write succeeded but the read-back verification failed — e.g. ephemeral-key
/// (CI) or non-macOS builds.  A dedicated verb makes the intent unambiguous and
/// removes the password from the general-purpose config payload.
///
/// # Non-macOS behaviour
///
/// On non-macOS the Keychain is unavailable.  The daemon holds the password
/// in-memory for the lifetime of the current process and logs a warning.  The
/// password is **never** written to `config.json` via this verb — callers that
/// need persistence on non-macOS must use `set_config` explicitly.
pub const METHOD_STORE_CLOUD_PASSWORD: &str = "store_cloud_password";

/// Parameters for [`METHOD_STORE_CLOUD_PASSWORD`].
///
/// Carries exactly one field so the password is never mixed with other
/// `set_config` fields and can be zeroized independently on the daemon side.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoreCloudPasswordRequest {
    /// The Supabase GoTrue account password (plain-text, passed over the local
    /// 0600 Unix socket). The daemon zeroizes this field after writing it to
    /// the Keychain / in-memory store.
    pub password: String,
}

/// Success payload for [`METHOD_STORE_CLOUD_PASSWORD`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StoreCloudPasswordResponse {
    /// `true` when the password was persisted to the macOS Keychain.
    /// `false` on non-macOS platforms where only in-memory storage is used.
    pub persisted: bool,
}

/// Query the current cloud-sync state.
pub const METHOD_GET_SYNC_STATUS: &str = "get_sync_status";

// ── SyncBadgeState — canonical daemon-computed badge state ──────────────────

/// The canonical sync-badge state computed once by the daemon and delivered
/// over IPC so every consumer (macOS UI, Android) renders an identical badge
/// without each re-implementing the derivation logic.
///
/// ## Motivation (CopyPaste-merc)
///
/// Before this type, macOS `SyncStatusChip.tsx` and Android `SyncStatusBadge.kt`
/// each re-derived the badge from raw IPC fields using local constants
/// (`RECENT_SYNC_MS = 300_000` on each platform) that could drift independently.
/// The badge could disagree on a daemon crash (macOS sees IPC-unreachable →
/// `Offline`; Android only sees OS-network → `NetworkOffline`).
///
/// Now the daemon is the **single source of truth**. Consumers that receive
/// `badge_state` in the `get_sync_status` response must render it directly and
/// must NOT re-derive the state from raw fields. A thin backward-compat
/// fallback is permitted only for responses from daemons older than this field.
///
/// ## Variants
///
/// | Variant          | Dot colour       | Meaning                                              |
/// |------------------|------------------|------------------------------------------------------|
/// | `Synced`         | green            | At least one peer exchanged data within 5 minutes.   |
/// | `Syncing`        | green (pulse)    | A sync round-trip is actively in flight.             |
/// | `Idle`           | grey             | Configured but no recent sync (devices may be off).  |
/// | `Offline`        | red              | Daemon detects no usable sync path.                  |
/// | `Error`          | red              | Sync backend returned an explicit error.             |
/// | `Misconfigured`  | amber            | Cloud URL set but credentials incomplete/invalid.    |
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncBadgeState {
    /// At least one peer/backend exchanged data within [`SYNC_BADGE_RECENT_MS`].
    Synced,
    /// A sync round-trip is actively in flight (future: when daemon exposes this).
    Syncing,
    /// Sync is configured but no recent successful exchange. Peers may be off.
    Idle,
    /// Daemon cannot reach any sync backend — IPC unreachable or no network path.
    Offline,
    /// Sync backend returned an explicit error (auth failure, RLS, relay down).
    Error,
    /// Cloud URL is set but credentials are missing or invalid
    /// (`supabase_configured == false` while `supabase_url` is non-empty).
    Misconfigured,
}

/// How recent a last-sync timestamp must be (milliseconds) for the daemon to
/// consider the link "synced". Single source of truth — replaces the per-platform
/// `RECENT_SYNC_MS` constants (macOS 300_000 and Android 5 * 60 * 1_000 L) that
/// were duplicated and could drift independently.
pub const SYNC_BADGE_RECENT_MS: u64 = 5 * 60 * 1_000; // 5 minutes

/// Compute the [`SyncBadgeState`] from raw daemon-side signals.
///
/// This is the single place where the badge derivation lives. The daemon calls
/// this and embeds the result in the `get_sync_status` response so consumers
/// never need to re-derive it.
///
/// # Arguments
///
/// * `passphrase_set` — whether a sync key is loaded (P2P or cloud).
/// * `supabase_url_set` — whether a Supabase project URL is configured.
/// * `supabase_configured` — URL + anon key both present (or `SUPABASE_URL` env).
/// * `signed_in` — whether GoTrue auth succeeded.
/// * `last_sync_ms` — timestamp of the last successful exchange (epoch ms), or
///   `None` when never synced.
/// * `now_ms` — current wall-clock time (epoch ms). Pass `None` to use
///   `std::time::SystemTime::now()` automatically.
///
/// To signal an active in-flight sync round-trip, use
/// [`compute_sync_badge_state_with_inflight`] instead. This function is kept
/// for backward-compatibility with existing callers and delegates with
/// `in_flight = false`.
pub fn compute_sync_badge_state(
    passphrase_set: bool,
    supabase_url_set: bool,
    supabase_configured: bool,
    signed_in: bool,
    last_sync_ms: Option<i64>,
    now_ms: Option<u64>,
) -> SyncBadgeState {
    // Delegate to the extended variant with in_flight=false so the existing
    // daemon caller continues to compile and behave identically (CopyPaste-1jms.22).
    compute_sync_badge_state_with_inflight(
        passphrase_set,
        supabase_url_set,
        supabase_configured,
        signed_in,
        last_sync_ms,
        now_ms,
        false,
    )
}

/// Extended variant of [`compute_sync_badge_state`] that adds an `in_flight`
/// signal for when a sync round-trip is actively in progress.
///
/// When `in_flight` is `true` and no recent sync has already been recorded,
/// this returns [`SyncBadgeState::Syncing`] (green pulse) instead of falling
/// through to the `Error`/`Offline`/`Idle` branches. The `Syncing` state is
/// transient: the caller is responsible for setting `in_flight` back to
/// `false` once the round-trip completes or fails.
///
/// The daemon should adopt this function once it threads an `Arc<AtomicBool>`
/// in-flight flag through the cloud-poll, relay-receive, and P2P loops.
///
/// # Arguments
///
/// Same as [`compute_sync_badge_state`], plus:
///
/// * `in_flight` — `true` while a cloud poll, relay push, or P2P handshake is
///   actively running.
pub fn compute_sync_badge_state_with_inflight(
    passphrase_set: bool,
    supabase_url_set: bool,
    supabase_configured: bool,
    signed_in: bool,
    last_sync_ms: Option<i64>,
    now_ms: Option<u64>,
    in_flight: bool,
) -> SyncBadgeState {
    // Resolve current time — allows tests to inject a deterministic value.
    let now = now_ms.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    });

    // Misconfig: cloud URL set but credentials absent/incomplete → amber.
    // Check this BEFORE the "no sync configured" path so a partially-configured
    // Supabase setup shows amber rather than the misleading grey idle dot.
    if supabase_url_set && !supabase_configured {
        return SyncBadgeState::Misconfigured;
    }

    // Recent sync: compare last_sync_ms against the 5-minute threshold.
    let recently_synced = last_sync_ms
        .map(|ts| ts > 0 && now.saturating_sub(ts as u64) <= SYNC_BADGE_RECENT_MS)
        .unwrap_or(false);

    if recently_synced {
        return SyncBadgeState::Synced;
    }

    // Active round-trip in progress and no recent completed sync → Syncing
    // (green pulse). Placed after Synced so a completed sync wins over an
    // in-flight one: if last_sync_ms is recent the round-trip is wrapping up
    // and Synced is the more accurate label.
    if in_flight {
        return SyncBadgeState::Syncing;
    }

    // Auth error: cloud is configured and URL is valid but GoTrue session failed.
    if supabase_configured && !signed_in {
        return SyncBadgeState::Error;
    }

    // No sync path configured at all AND no recent activity → Offline.
    // "No path" means neither a passphrase (P2P/relay) nor a Supabase URL.
    if !passphrase_set && !supabase_url_set {
        return SyncBadgeState::Offline;
    }

    // Configured but stale — idle grey.
    SyncBadgeState::Idle
}

/// Success payload for [`METHOD_GET_SYNC_STATUS`].
///
/// The `badge_state` field is the canonical single-value answer to "what colour
/// should the sync dot be?". Consumers MUST use it directly when present and
/// MUST NOT re-derive the badge from the raw fields. The raw fields
/// (`passphrase_set`, `supabase_configured`, `signed_in`, `last_sync_ms`, …)
/// remain for display detail (tooltip, settings view) and backward-compat with
/// older consumers.
///
/// `badge_state` is `Option` for forward-compat: a client talking to a daemon
/// older than this field receives `None` and may fall back to local derivation.
/// Once the fleet has migrated, the fallback may be dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetSyncStatusResponse {
    /// Whether a passphrase-derived sync key is loaded in the daemon.
    pub passphrase_set: bool,
    /// Whether Supabase URL + anon key are configured (or `SUPABASE_URL` env set).
    pub supabase_configured: bool,
    /// Whether the daemon's GoTrue session is authenticated.
    pub signed_in: bool,
    /// Unix epoch milliseconds of the last successful sync, or `null` / `None`.
    pub last_sync_ms: Option<i64>,
    /// Non-secret Supabase project URL (for display / prefill). `None` when
    /// not configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supabase_url: Option<String>,
    /// Masked GoTrue account email (first-char-and-domain form, e.g.
    /// `d***@example.com`). `None` when no email is configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// **Canonical badge state** — daemon-computed, single source of truth.
    ///
    /// Consumers MUST render this directly. Omitted by daemons predating this
    /// field; in that case the consumer may fall back to local derivation from
    /// `last_sync_ms` + `supabase_configured` with their own threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_state: Option<SyncBadgeState>,
}

/// Run a live connection diagnostic against the configured cloud backend.
pub const METHOD_CLOUD_TEST_CONNECTION: &str = "cloud_test_connection";

// ── Pairing ─────────────────────────────────────────────────────────────────

/// Generate a short-lived QR pairing payload.
pub const METHOD_PAIR_GENERATE_QR: &str = "pair_generate_qr";

// ── LAN/SAS discovery ────────────────────────────────────────────────────────

/// Return the list of peers currently visible via mDNS-SD, cross-referenced
/// against `peers.json` to mark each as paired or not.
///
/// Response shape: `{ devices: [{ device_id, device_name, ip_addrs, port,
/// bport, paired }] }`.  `paired` is `true` when the device's canonical
/// fingerprint matches an entry in `peers.json`.  `bport` is the bootstrap
/// port for SAS pairing (null on v1 peers); the UI should disable "Pair" when
/// `bport` is null.
pub const METHOD_LIST_DISCOVERED: &str = "list_discovered";

// ── LAN/SAS discovery-initiated pairing (Phase 2) ─────────────────────────────

/// Begin a discovery-initiated SAS pairing as the INITIATOR.
///
/// Takes `{ device_id }` (the discovered peer's mDNS `did`). The daemon resolves
/// the peer's bootstrap address (`bport`), generates an ephemeral random PAKE
/// password, runs the bootstrap handshake, and (on reaching the SAS step)
/// transitions the pairing state machine to `awaiting_sas`. The UI then polls
/// [`METHOD_PAIR_GET_SAS`] and calls [`METHOD_PAIR_CONFIRM_SAS`].
pub const METHOD_PAIR_WITH_DISCOVERED: &str = "pair_with_discovered";

/// Poll the discovery-pairing state machine.
///
/// Response: `{ state, sas?, role? }` where `state` is one of `idle`,
/// `initiating`, `awaiting_sas`, `confirmed`, `rejected`, `aborted`,
/// `timed_out`. `sas` (6 decimal digits) and `role` (`initiator`/`responder`)
/// are present only in `awaiting_sas`.
pub const METHOD_PAIR_GET_SAS: &str = "pair_get_sas";

/// Deliver the local user's SAS accept/reject decision.
///
/// Takes `{ accept: bool }`. Fires the in-flight handshake's confirmation
/// channel; the pairing succeeds (keys trusted + persisted) only when BOTH sides
/// accept. On reject the keys are dropped/zeroized and nothing is persisted.
pub const METHOD_PAIR_CONFIRM_SAS: &str = "pair_confirm_sas";

/// Abort an in-flight discovery pairing and reset the state machine to `idle`.
pub const METHOD_PAIR_ABORT: &str = "pair_abort";

/// Pair with a peer using a shared password (non-QR / non-SAS path).
///
/// Params: `{ peer_fingerprint: String, password: String }`.  Used when the
/// other device provides a fixed password instead of a QR / SAS code.
pub const METHOD_PAIR_PEER_WITH_PASSWORD: &str = "pair_peer_with_password";

// ── Peer management ──────────────────────────────────────────────────────────

/// Remove a paired peer (untrust, delete from `peers.json`, no key rotation).
///
/// Params: `{ fingerprint: String }`.  The peer is removed from the local trust
/// store; items it synced remain in history.  Use [`METHOD_REVOKE_PEER`] for a
/// stronger revoke that also logs the revocation timestamp.
pub const METHOD_UNPAIR_PEER: &str = "unpair_peer";

/// Revoke a paired peer with a logged revocation timestamp.
///
/// Params: `{ fingerprint: String }`.  More forceful than unpair: the peer's
/// entry is removed AND a `revoked_at` timestamp is persisted.
/// Returns `{ revoked_at: i64 }`.
pub const METHOD_REVOKE_PEER: &str = "revoke_peer";

/// Revoke ALL paired peers in one call.
///
/// Returns `{ revoked: u32 }` — the number of peers removed.
pub const METHOD_REVOKE_ALL_PEERS: &str = "revoke_all_peers";

/// List all paired devices.
///
/// Returns `{ peers: [PairedDevice] }` including online/offline status,
/// last-seen, latency, and sync timestamps.
pub const METHOD_LIST_PEERS: &str = "list_peers";

/// Reorder the pinned-item display sequence.
///
/// Params: `{ ids: [String] }` — complete ordered list of pinned item IDs.
/// The daemon stores the order and returns items sorted by it in subsequent
/// `history_page` responses.
pub const METHOD_REORDER_PINNED: &str = "reorder_pinned";

/// Drain all pending peer connect/disconnect events since the last call.
///
/// Returns `{ events: [{ kind: "connected" | "disconnected", fingerprint: String }] }`.
/// Used by the app-global peer-presence polling loop; individual UI components
/// subscribe to the derived presence store rather than calling this directly.
pub const METHOD_POLL_PEER_EVENTS: &str = "poll_peer_events";

/// Force an mDNS-SD rescan (restart-in-place re-browse) and return the
/// fresh discovered device list.  Same response shape as [`METHOD_LIST_DISCOVERED`].
pub const METHOD_RESCAN_DISCOVERED: &str = "rescan_discovered";

// ── Item media access ───────────────────────────────────────────────────────

/// Fetch the full image bytes for a `content_type == "image"` clipboard item.
///
/// Params: `{ id: String }`.  Returns `{ data_uri: String }` (a `data:image/…`
/// URL with base64-encoded bytes).  Use [`METHOD_GET_ITEM_THUMBNAIL`] for the
/// pre-computed low-resolution preview.
pub const METHOD_GET_ITEM_IMAGE: &str = "get_item_image";

/// Fetch the full binary payload for a `content_type == "file"` clipboard item.
///
/// Params: `{ id: String }`.  Returns `{ filename: String, mime: String,
/// data_b64: String }` where `data_b64` is standard base64.  The daemon reads
/// the encrypted blob, decrypts it, and returns the raw bytes.
pub const METHOD_GET_ITEM_FILE: &str = "get_item_file";

/// Fetch the pre-computed thumbnail for a clipboard image item.
///
/// Params: `{ id: String }`.  Returns `{ thumbnail: String | null }` where
/// `thumbnail` is a `data:image/webp;base64,…` URL.  `null` when thumbnails are
/// unavailable for this item (older daemon, non-image item, or generation
/// failed at capture time).  Callers fall back to [`METHOD_GET_ITEM_IMAGE`].
pub const METHOD_GET_ITEM_THUMBNAIL: &str = "get_item_thumbnail";

/// Resolve a macOS app bundle identifier to a 32×32 PNG icon (base64).
///
/// Params: `{ bundle_id: String }`.  Returns `{ png_b64: String | null }`.
/// `null` when the app is not installed or the daemon cannot extract the icon.
/// Results are cached in the daemon so repeated calls are fast.
pub const METHOD_GET_APP_ICON: &str = "get_app_icon";

// ── Own device identity ──────────────────────────────────────────────────────

/// Return this device's mTLS certificate fingerprint (hex SHA-256).
///
/// Returns `{ fingerprint: String }`.  Null when P2P is disabled (no cert).
/// Superseded by [`METHOD_GET_OWN_DEVICE_INFO`] which returns the full
/// identity; retained for back-compat with older callers.
pub const METHOD_GET_OWN_FINGERPRINT: &str = "get_own_fingerprint";

/// Return rich identity for THIS device: name, model, OS, version, IPs,
/// and mTLS fingerprint.
///
/// Returns `{ fingerprint, device_name, device_model, os_version,
/// app_version, local_ip, public_ip }`.  All fields except `app_version`
/// are optional — gracefully handle absent ones.
pub const METHOD_GET_OWN_DEVICE_INFO: &str = "get_own_device_info";

// ── File ingest ─────────────────────────────────────────────────────────────

/// Ingest a file directly into the clipboard history from the desktop UI.
///
/// Params: `{ filename: String, mime: String, data_b64: String }` where
/// `data_b64` is standard base64-encoded raw file bytes. The daemon encrypts,
/// stores, and deduplicates it the same way a pasteboard-captured file is
/// stored via `handle_file`.
///
/// Response: `{ id: String }` — the stable clipboard item UUID.
pub const METHOD_ADD_FILE_ITEM: &str = "add_file_item";

// ── Database maintenance ────────────────────────────────────────────────────

/// Return lightweight storage statistics for the local clipboard database.
///
/// Params: none (empty `{}`).
/// Response: `{ item_count: u64, size_bytes: u64 }`.
///
/// - `item_count` — total number of items stored (includes deleted/tombstoned rows).
/// - `size_bytes` — approximate on-disk size of the main database file in bytes.
///   Does not include the WAL file; use [`METHOD_VACUUM`] to flush WAL into the main
///   file before calling this if you need an accurate compacted size.
///
/// Used by the macOS UI's settings panel (SettingsView.gq51) to show a storage
/// usage summary without triggering the heavier `stats` computation.
pub const METHOD_DB_STATS: &str = "db_stats";

/// Success payload for [`METHOD_DB_STATS`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DbStatsResponse {
    /// Total number of items in `clipboard_items` (all rows, including tombstones).
    pub item_count: u64,
    /// On-disk size of the main database file in bytes (WAL not included).
    pub size_bytes: u64,
}

/// Run `VACUUM` (and optionally `REINDEX`) on the encrypted clipboard database.
///
/// The daemon holds the write-lock for the duration and runs the operation on a
/// blocking thread so the async executor is not starved. The daemon MUST be
/// running for this method to be callable — the client no longer needs to stop
/// the daemon, open the DB directly, or touch the macOS Keychain.
///
/// ## Parameters ([`VacuumRequest`])
/// - `reindex_only` (`bool`, default `false`): skip `VACUUM`, run only `REINDEX`.
/// - `dry_run` (`bool`, default `false`): open the DB to verify the key, report
///   current size, but do NOT mutate any data.
///
/// ## Response ([`VacuumResponse`])
/// - `size_before` (`u64`): file size in bytes before the operation.
/// - `size_after` (`u64`): file size in bytes after (same as `size_before` on
///   `dry_run`).
/// - `reclaimed` (`i64`): `size_before - size_after` (negative = file grew,
///   e.g. after `REINDEX` on a fragmented DB).
///
/// Success is conveyed solely by the outer `Response.ok` envelope field;
/// the payload carries only meaningful data fields (c4q2.22).
pub const METHOD_VACUUM: &str = "vacuum";

/// Parameters for the [`METHOD_VACUUM`] method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VacuumRequest {
    /// When `true`, skip `VACUUM` and run only `REINDEX`. Faster; does not
    /// require free space equal to the current DB size.
    #[serde(default)]
    pub reindex_only: bool,
    /// When `true`, report what would happen without mutating the database.
    #[serde(default)]
    pub dry_run: bool,
}

/// Success payload for the [`METHOD_VACUUM`] method.
///
/// The outer `Response.ok` envelope is the authoritative success indicator;
/// this struct carries only data fields that add information (c4q2.22 —
/// removed the formerly-redundant `ok: bool` field).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VacuumResponse {
    /// DB file size in bytes *before* the operation.
    pub size_before: u64,
    /// DB file size in bytes *after* the operation (equals `size_before` on
    /// `dry_run`).
    pub size_after: u64,
    /// `size_before - size_after`; negative when the file grew.
    pub reclaimed: i64,
}

/// Method name for the destructive "reset database" recovery operation.
///
/// This wipes `clipboard.db` (and its `-wal` / `-shm` siblings) and recreates a
/// fresh, empty encrypted database with the daemon's current key. It is the
/// explicit escape hatch a user invokes from the desktop UI when the daemon is
/// running DEGRADED because the existing database cannot be decrypted (key
/// mismatch / "file is not a database"). Unlike every other DB-touching method,
/// the daemon honours this one *in* degraded mode — that is the whole point.
///
/// MUST carry [`ResetDatabaseRequest::confirm`] = `true` or the daemon refuses
/// it, so it can never fire by accident.
pub const METHOD_RESET_DATABASE: &str = "reset_database";

/// Parameters for the [`METHOD_RESET_DATABASE`] method.
///
/// `confirm` is a mandatory explicit acknowledgement of the destructive intent.
/// The daemon rejects the request with `invalid_argument` unless `confirm` is
/// `true`, so a stray or replayed `reset_database` call with no/false confirm
/// cannot erase the user's history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResetDatabaseRequest {
    /// Must be `true` to authorise the destructive wipe-and-recreate.
    #[serde(default)]
    pub confirm: bool,
}

/// Success payload for the [`METHOD_RESET_DATABASE`] method.
///
/// On success the daemon has deleted the old database files, created a fresh
/// empty encrypted database with its current key, and brought itself OUT of
/// degraded mode in-place — so a subsequent `history_page` (or any DB-touching
/// method) succeeds against the new empty DB without a process restart.
///
/// The outer `Response.ok` envelope is the authoritative success indicator.
/// The former `reset: bool` field (always `true` on success) was removed as
/// redundant (c4q2.22); callers must check the envelope `ok` field instead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResetDatabaseResponse {
    /// `true` when the daemon recovered IN-PLACE (no restart needed): the new
    /// empty DB is live and the daemon is now ready. The current implementation
    /// always recovers in-place, so this is always `true` on success.
    pub ready: bool,
}

// ── Database backup / restore (CopyPaste-x94p / CopyPaste-8wbt) ─────────────

/// Create an encrypted SQLCipher backup of the local clipboard database.
///
/// The daemon owns both the database file and the encryption key, so it can
/// produce a hot, consistent backup without stopping itself. Internally the
/// handler runs `VACUUM INTO '<dest>'` which copies every non-empty page into
/// a new file encrypted with the **same key** as the source database.
///
/// ## Parameters ([`DbBackupRequest`])
/// - `dest_path` (`String`): absolute path for the output backup file.
///   The file must NOT already exist; the daemon refuses to overwrite.
///
/// ## Response ([`DbBackupResponse`])
/// - `dest_path` (`String`): the path the backup was written to.
/// - `size_bytes` (`u64`): size of the backup file in bytes.
///
/// (`ok` field removed c4q2.22 — the outer `Response.ok` envelope is authoritative.)
///
/// (CopyPaste-x94p)
pub const METHOD_DB_BACKUP: &str = "db_backup";

/// Parameters for [`METHOD_DB_BACKUP`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DbBackupRequest {
    /// Absolute path where the backup file will be written.
    /// The daemon refuses to overwrite an existing file.
    pub dest_path: String,
}

/// Success payload for [`METHOD_DB_BACKUP`].
///
/// The outer `Response.ok` envelope is the authoritative success indicator;
/// the former `ok: bool` field (always `true` on success) was removed as
/// redundant (c4q2.22).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DbBackupResponse {
    /// The path the backup was written to (mirrors `DbBackupRequest::dest_path`).
    pub dest_path: String,
    /// Size of the backup file in bytes.
    pub size_bytes: u64,
}

/// Restore the local clipboard database from an encrypted SQLCipher backup.
///
/// The daemon must be running to service this call. The handler:
///
/// 1. Validates `confirm = true` (refuses without it).
/// 2. Verifies the backup file exists and is readable.
/// 3. Swaps the live DB handle to an in-memory instance so all pending writes
///    are quiesced (mirrors the `reset_database` safe-swap pattern).
/// 4. Renames the existing `clipboard.db` (+ WAL/SHM) aside to a timestamped
///    `.before-restore-<ts>` name (or deletes them when `force = true`).
/// 5. Copies the backup file into place as `clipboard.db`.
/// 6. Reopens the database with the daemon's current key.
///    The backup **must** have been encrypted with this same key — if the key
///    mismatches, `Database::open` returns an error and the daemon remains
///    degraded (the aside file is intact for manual recovery).
/// 7. Swaps the live handle back to the restored database and returns ready.
///
/// ## Parameters ([`DbRestoreRequest`])
/// - `confirm` (`bool`): must be `true`; prevents accidental invocations.
/// - `src_path` (`String`): absolute path to the backup file to restore.
/// - `force` (`bool`, default `false`): delete the existing DB instead of
///   renaming it aside. Use when disk space is tight.
///
/// ## Response ([`DbRestoreResponse`])
/// - `ready` (`bool`): always `true`; the restored DB is live.
///
/// (`ok` field removed c4q2.22 — the outer `Response.ok` envelope is authoritative.)
///
/// (CopyPaste-8wbt)
pub const METHOD_DB_RESTORE: &str = "db_restore";

/// Parameters for [`METHOD_DB_RESTORE`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DbRestoreRequest {
    /// Must be `true` to authorise the destructive replace-in-place.
    #[serde(default)]
    pub confirm: bool,
    /// Absolute path to the backup file to restore from.
    pub src_path: String,
    /// When `true`, delete the existing live DB instead of renaming it aside.
    #[serde(default)]
    pub force: bool,
}

/// Success payload for [`METHOD_DB_RESTORE`].
///
/// The outer `Response.ok` envelope is the authoritative success indicator;
/// the former `ok: bool` field (always `true` on success) was removed as
/// redundant (c4q2.22).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DbRestoreResponse {
    /// `true` when the restored database is live (no restart needed).
    pub ready: bool,
}

// ── AppConfig IPC wire type ───────────────────────────────────────────────────
//
// This is the canonical IPC-wire representation of the daemon's application
// configuration. All fields are `Option<T>` so that a `set_config` call can
// omit fields it does not want to change ("None = preserve existing").
//
// This type lives here (copypaste-ipc) — not in copypaste-daemon — so that:
//   1. CLI and UI can deserialise `get_config` responses with the same struct.
//   2. The field set is a single source of truth: adding a new setting requires
//      touching one place (here) instead of two structs in two crates.
//
// The daemon's `ipc.rs` currently re-declares an identical struct for
// historical reasons (pre-dates this crate); the plan is to retire that copy
// and import this one directly once the ipc.rs cleanup (CopyPaste-c4q2.1 /
// c4q2.18) is complete.
//
// **SECRET FIELDS NOTE**: supabase_email, supabase_password are present here
// because they travel inbound (set_config) and must be representable. The
// daemon ALWAYS redacts them before returning them via get_config
// (replaced by `supabase_email_set: bool` / `supabase_password_set: bool`).
// Non-secret fields (supabase_url, supabase_anon_key, relay_url, limits, etc.)
// are surfaced verbatim. See `redact_config_secrets` in the daemon.

/// IPC wire type for `get_config` / `set_config` method payloads.
///
/// Every field is `Option<T>`:
/// - `None` on `set_config` means "do not change this field".
/// - `None` on `get_config` means "not set / no value stored".
///
/// The daemon merges incoming `set_config` values onto the persisted store
/// rather than replacing it wholesale, so a call that only sets `relay_url`
/// never accidentally wipes credentials or limit fields.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    /// Whether P2P sync is enabled. `None` = not specified (preserve existing).
    /// `Some(false)` = explicit opt-out; persisted to `config.json`.
    /// Defaults to `true` on first install.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p2p_enabled: Option<bool>,

    /// Supabase project URL (e.g. `https://xxxx.supabase.co`).
    /// Env override: `SUPABASE_URL`. `None` = not configured.
    #[serde(default)]
    pub supabase_url: Option<String>,

    /// Supabase publishable anon/public JWT. Safe to surface in UI.
    /// Env override: `SUPABASE_ANON_KEY`. `None` = not configured.
    #[serde(default)]
    pub supabase_anon_key: Option<String>,

    /// HTTP relay base URL for store-and-forward sync fan-out
    /// (e.g. `https://relay.example.com`). Non-secret: surfaced verbatim
    /// by `get_config`. `None` / absent on `set_config` preserves the
    /// stored value. Mirrored into core `config.toml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,

    /// GoTrue account email for the `authenticated` scope sign-in.
    /// Redacted to `supabase_email_set: bool` by `get_config`.
    /// Env override: `SUPABASE_EMAIL`.
    #[serde(default)]
    pub supabase_email: Option<String>,

    /// GoTrue account password. Never logged.
    /// Redacted to `supabase_password_set: bool` by `get_config`.
    /// Env override: `SUPABASE_PASSWORD`.
    #[serde(default)]
    pub supabase_password: Option<String>,

    // ── Limit / preference fields — persisted to config.toml via set_config ──
    // `None` means "use whatever is already in config.toml" so that a UI which
    // only touches one field never accidentally resets the others.

    /// Maximum size of a single captured text item (bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_text_size_bytes: Option<u64>,

    /// Maximum size of a captured image (bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_image_size_bytes: Option<u64>,

    /// Maximum size of a captured file reference (bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_size_bytes: Option<u64>,

    /// Maximum total byte size of unpinned clipboard items in the local DB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_quota_bytes: Option<u64>,

    /// Auto-wipe TTL for sensitive items (seconds). `0` = disabled sentinel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitive_ttl_secs: Option<u64>,

    /// Image quality (1–100; 100 = lossless).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_quality: Option<u8>,

    /// If `true`, skip cloud/P2P sync when not on Wi-Fi.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_on_wifi_only: Option<bool>,

    /// Play a soft system sound when the daemon captures a new clipboard item.
    /// `None` = not specified (preserve existing). macOS only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_on_copy: Option<bool>,

    /// Show a notification banner when the daemon captures a new clipboard item.
    /// `None` = not specified (preserve existing). macOS only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_on_copy: Option<bool>,

    /// Whether the daemon may make a one-off STUN request to discover this
    /// device's public IP. `None` = not specified (preserve existing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collect_public_ip: Option<bool>,

    /// When `true`, paste-back strips all rich types and writes plain text only.
    /// `None` = not specified (preserve existing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_as_plain_text: Option<bool>,

    /// Bundle IDs of apps whose clipboard copies are silently skipped (macOS).
    /// `None` = not specified (preserve existing); `Some(vec)` replaces the list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_app_bundle_ids: Option<Vec<String>>,

    /// Whether this device advertises via mDNS-SD and browses for LAN peers.
    /// `false` = invisible on the local network. `None` = preserve existing
    /// (default `true` on first install).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lan_visibility: Option<bool>,

    /// Master switch for all sync transports (relay, cloud, P2P).
    /// `false` = no data sent to or received from any remote device.
    /// `None` = preserve existing (default `true` on first install).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_enabled: Option<bool>,

    /// Universal Clipboard: when `true`, the daemon immediately writes a
    /// freshly-synced item to the local pasteboard. `false` = store-only.
    /// `None` = preserve existing (default `true`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_apply_synced_clip: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_cloud_password_method_has_correct_wire_name() {
        assert_eq!(METHOD_STORE_CLOUD_PASSWORD, "store_cloud_password");
    }

    #[test]
    fn store_cloud_password_request_roundtrip() {
        let req = StoreCloudPasswordRequest {
            password: "s3cr3t".into(),
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: StoreCloudPasswordRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
        assert!(s.contains("\"password\":\"s3cr3t\""));
    }

    #[test]
    fn store_cloud_password_response_roundtrip() {
        for persisted in [true, false] {
            let resp = StoreCloudPasswordResponse { persisted };
            let s = serde_json::to_string(&resp).unwrap();
            let back: StoreCloudPasswordResponse = serde_json::from_str(&s).unwrap();
            assert_eq!(resp, back);
        }
    }

    #[test]
    fn method_list_discovered_has_correct_wire_name() {
        assert_eq!(METHOD_LIST_DISCOVERED, "list_discovered");
    }

    #[test]
    fn discovery_pairing_methods_have_correct_wire_names() {
        assert_eq!(METHOD_PAIR_WITH_DISCOVERED, "pair_with_discovered");
        assert_eq!(METHOD_PAIR_GET_SAS, "pair_get_sas");
        assert_eq!(METHOD_PAIR_CONFIRM_SAS, "pair_confirm_sas");
        assert_eq!(METHOD_PAIR_ABORT, "pair_abort");
    }

    #[test]
    fn reset_request_defaults_confirm_false() {
        // An empty params object must deserialize with confirm = false so a
        // caller who forgets the flag is rejected rather than silently wiping.
        let req: ResetDatabaseRequest = serde_json::from_str("{}").unwrap();
        assert!(!req.confirm);
    }

    #[test]
    fn reset_request_roundtrip() {
        let req = ResetDatabaseRequest { confirm: true };
        let s = serde_json::to_string(&req).unwrap();
        let back: ResetDatabaseRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
        assert!(s.contains("\"confirm\":true"));
    }

    /// c4q2.22: ResetDatabaseResponse no longer has a `reset` field.
    #[test]
    fn reset_response_roundtrip() {
        let resp = ResetDatabaseResponse { ready: true };
        let s = serde_json::to_string(&resp).unwrap();
        let back: ResetDatabaseResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(!s.contains("\"reset\""), "c4q2.22: reset field must not appear in wire: {s}");
    }

    #[test]
    fn db_stats_method_has_correct_wire_name() {
        assert_eq!(METHOD_DB_STATS, "db_stats");
    }

    #[test]
    fn db_stats_response_roundtrip() {
        let resp = DbStatsResponse {
            item_count: 42,
            size_bytes: 1024 * 512,
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: DbStatsResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(s.contains("\"item_count\":42"), "wire: {s}");
        assert!(s.contains("\"size_bytes\":"), "wire: {s}");
    }

    #[test]
    fn db_stats_response_default_is_zero() {
        let resp = DbStatsResponse::default();
        assert_eq!(resp.item_count, 0);
        assert_eq!(resp.size_bytes, 0);
    }

    #[test]
    fn vacuum_method_has_correct_wire_name() {
        assert_eq!(METHOD_VACUUM, "vacuum");
    }

    #[test]
    fn vacuum_request_defaults_all_false() {
        // An empty params object must parse with all flags false so a bare
        // `{"method":"vacuum","params":{}}` call runs the full VACUUM + REINDEX.
        let req: VacuumRequest = serde_json::from_str("{}").unwrap();
        assert!(!req.reindex_only);
        assert!(!req.dry_run);
    }

    #[test]
    fn vacuum_request_roundtrip() {
        let req = VacuumRequest {
            reindex_only: true,
            dry_run: false,
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: VacuumRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
    }

    /// c4q2.22: VacuumResponse no longer has an `ok` field (removed as redundant —
    /// the outer response envelope's `ok` is the authoritative success indicator).
    #[test]
    fn vacuum_response_roundtrip() {
        let resp = VacuumResponse {
            size_before: 2048,
            size_after: 1024,
            reclaimed: 1024,
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: VacuumResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(!s.contains("\"ok\""), "c4q2.22: ok field must not appear in wire format: {s}");
    }

    // PG-62: verify all previously-missing METHOD_* constants have the correct
    // wire names (matching the string literals used in the UI's ipc.ts api object).
    #[test]
    fn pg62_history_page_method_has_correct_wire_name() {
        assert_eq!(METHOD_HISTORY_PAGE, "history_page");
    }

    #[test]
    fn pg62_copy_item_method_has_correct_wire_name() {
        assert_eq!(METHOD_COPY_ITEM, "copy_item");
    }

    #[test]
    fn pg62_delete_item_method_has_correct_wire_name() {
        assert_eq!(METHOD_DELETE_ITEM, "delete_item");
    }

    #[test]
    fn pg62_sync_key_methods_have_correct_wire_names() {
        assert_eq!(METHOD_SET_SYNC_PASSPHRASE, "set_sync_passphrase");
        assert_eq!(METHOD_ROTATE_SYNC_KEY, "rotate_sync_key");
        assert_eq!(METHOD_REVOKE_AND_ROTATE, "revoke_and_rotate");
    }

    #[test]
    fn pg62_item_media_methods_have_correct_wire_names() {
        assert_eq!(METHOD_GET_ITEM_IMAGE, "get_item_image");
        assert_eq!(METHOD_GET_ITEM_FILE, "get_item_file");
        assert_eq!(METHOD_GET_ITEM_THUMBNAIL, "get_item_thumbnail");
        assert_eq!(METHOD_GET_APP_ICON, "get_app_icon");
    }

    #[test]
    fn pg62_own_device_methods_have_correct_wire_names() {
        assert_eq!(METHOD_GET_OWN_FINGERPRINT, "get_own_fingerprint");
        assert_eq!(METHOD_GET_OWN_DEVICE_INFO, "get_own_device_info");
    }

    #[test]
    fn pg62_peer_management_methods_have_correct_wire_names() {
        assert_eq!(METHOD_LIST_PEERS, "list_peers");
        assert_eq!(METHOD_POLL_PEER_EVENTS, "poll_peer_events");
        assert_eq!(METHOD_PAIR_PEER_WITH_PASSWORD, "pair_peer_with_password");
        assert_eq!(METHOD_UNPAIR_PEER, "unpair_peer");
        assert_eq!(METHOD_REVOKE_PEER, "revoke_peer");
        assert_eq!(METHOD_REVOKE_ALL_PEERS, "revoke_all_peers");
        assert_eq!(METHOD_REORDER_PINNED, "reorder_pinned");
        assert_eq!(METHOD_RESCAN_DISCOVERED, "rescan_discovered");
    }

    // ── compute_sync_badge_state tests (CopyPaste-merc) ─────────────────────

    // Helper: a fixed "now" far enough from any test timestamp.
    const NOW_MS: u64 = 1_000_000_000_000; // 2001-09-09 in ms
                                           // "5 minutes ago minus 1 s" — inside the RECENT window.
    const RECENT_MS: i64 = (NOW_MS - SYNC_BADGE_RECENT_MS + 1_000) as i64;
    // "5 minutes ago plus 1 s" — outside the RECENT window.
    const STALE_MS: i64 = (NOW_MS - SYNC_BADGE_RECENT_MS - 1_000) as i64;

    #[test]
    fn badge_state_synced_when_recent_sync() {
        let state = compute_sync_badge_state(
            true, // passphrase_set
            true, // supabase_url_set
            true, // supabase_configured
            true, // signed_in
            Some(RECENT_MS),
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Synced);
    }

    #[test]
    fn badge_state_idle_when_stale_sync_but_configured() {
        let state = compute_sync_badge_state(
            true, // passphrase_set
            true, // supabase_url_set
            true, // supabase_configured
            true, // signed_in
            Some(STALE_MS),
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Idle);
    }

    #[test]
    fn badge_state_idle_when_never_synced_but_passphrase_set() {
        let state = compute_sync_badge_state(
            true,  // passphrase_set — a sync path exists
            false, // supabase_url_set
            false, // supabase_configured
            false, // signed_in
            None,  // never synced
            Some(NOW_MS),
        );
        // passphrase_set = true means a P2P sync path is configured → Idle, not Offline.
        assert_eq!(state, SyncBadgeState::Idle);
    }

    #[test]
    fn badge_state_offline_when_nothing_configured() {
        let state = compute_sync_badge_state(
            false, // passphrase_set
            false, // supabase_url_set
            false, // supabase_configured
            false, // signed_in
            None,  // never synced
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Offline);
    }

    #[test]
    fn badge_state_misconfigured_when_url_set_but_not_configured() {
        // Cloud URL is set but anon key / credentials are missing.
        let state = compute_sync_badge_state(
            false, // passphrase_set
            true,  // supabase_url_set
            false, // supabase_configured — anon key absent
            false, // signed_in
            None,
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Misconfigured);
    }

    #[test]
    fn badge_state_error_when_configured_but_not_signed_in() {
        // URL + anon key present, but GoTrue auth failed (signed_in = false).
        let state = compute_sync_badge_state(
            false, // passphrase_set
            true,  // supabase_url_set
            true,  // supabase_configured
            false, // signed_in — auth failure
            Some(STALE_MS),
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Error);
    }

    #[test]
    fn badge_state_synced_takes_priority_over_error() {
        // Even when signed_in=false, a RECENT sync means Synced (key rotation in
        // flight, or config changing mid-session).
        let state = compute_sync_badge_state(
            true,  // passphrase_set
            true,  // supabase_url_set
            true,  // supabase_configured
            false, // signed_in — but recent exchange happened
            Some(RECENT_MS),
            Some(NOW_MS),
        );
        assert_eq!(state, SyncBadgeState::Synced);
    }

    // ── compute_sync_badge_state_with_inflight tests (CopyPaste-1jms.22) ──────

    #[test]
    fn badge_state_syncing_when_in_flight_and_no_recent_sync() {
        // The primary acceptance criterion: in_flight=true with no recent sync
        // must return Syncing (green pulse).
        let state = compute_sync_badge_state_with_inflight(
            true,  // passphrase_set
            true,  // supabase_url_set
            true,  // supabase_configured
            true,  // signed_in
            None,  // no prior sync
            Some(NOW_MS),
            true,  // in_flight — round-trip actively running
        );
        assert_eq!(state, SyncBadgeState::Syncing);
    }

    #[test]
    fn badge_state_synced_wins_over_in_flight_when_recently_synced() {
        // A completed recent sync takes priority over an in-flight flag: the
        // round-trip is wrapping up and Synced is the more accurate label.
        let state = compute_sync_badge_state_with_inflight(
            true,
            true,
            true,
            true,
            Some(RECENT_MS),
            Some(NOW_MS),
            true, // in_flight set — but recently_synced wins
        );
        assert_eq!(state, SyncBadgeState::Synced);
    }

    #[test]
    fn badge_state_in_flight_false_behaves_identically_to_original() {
        // in_flight=false must not change the derivation — ensures backward
        // compatibility between compute_sync_badge_state and the _with_inflight
        // variant.
        // Each tuple is (passphrase_set, url_set, configured, signed_in, last_sync,
        // expected_badge).  The six-element anonymous tuple is deliberately
        // kept inline here — a named type would add noise without clarity for a
        // single test-internal table.
        #[allow(clippy::type_complexity)]
        let cases: &[(bool, bool, bool, bool, Option<i64>, SyncBadgeState)] = &[
            (true, true, true, true, Some(RECENT_MS), SyncBadgeState::Synced),
            (true, true, true, true, Some(STALE_MS), SyncBadgeState::Idle),
            (false, false, false, false, None, SyncBadgeState::Offline),
            (false, true, false, false, None, SyncBadgeState::Misconfigured),
            (false, true, true, false, Some(STALE_MS), SyncBadgeState::Error),
        ];
        for (passphrase_set, url_set, configured, signed_in, last_sync, expected) in cases {
            let via_new = compute_sync_badge_state_with_inflight(
                *passphrase_set,
                *url_set,
                *configured,
                *signed_in,
                *last_sync,
                Some(NOW_MS),
                false, // in_flight=false → should match the old function
            );
            let via_old = compute_sync_badge_state(
                *passphrase_set,
                *url_set,
                *configured,
                *signed_in,
                *last_sync,
                Some(NOW_MS),
            );
            assert_eq!(via_new, *expected, "new fn mismatch");
            assert_eq!(via_old, *expected, "old fn mismatch");
            assert_eq!(via_new, via_old, "parity between old and new(in_flight=false)");
        }
    }

    #[test]
    fn sync_badge_state_serialises_to_snake_case() {
        let cases = [
            (SyncBadgeState::Synced, r#""synced""#),
            (SyncBadgeState::Syncing, r#""syncing""#),
            (SyncBadgeState::Idle, r#""idle""#),
            (SyncBadgeState::Offline, r#""offline""#),
            (SyncBadgeState::Error, r#""error""#),
            (SyncBadgeState::Misconfigured, r#""misconfigured""#),
        ];
        for (variant, expected) in &cases {
            let s = serde_json::to_string(variant).unwrap();
            assert_eq!(&s, expected, "variant serialisation mismatch");
        }
    }

    #[test]
    fn get_sync_status_response_roundtrip_with_badge_state() {
        let resp = GetSyncStatusResponse {
            passphrase_set: true,
            supabase_configured: true,
            signed_in: true,
            last_sync_ms: Some(RECENT_MS),
            supabase_url: Some("https://example.supabase.co".into()),
            email: Some("d***@example.com".into()),
            badge_state: Some(SyncBadgeState::Synced),
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: GetSyncStatusResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        // badge_state must be on the wire with snake_case variant name.
        assert!(s.contains(r#""badge_state":"synced""#), "wire: {s}");
    }

    #[test]
    fn get_sync_status_response_badge_state_omitted_when_none() {
        // Backward-compat: older consumers that do not know badge_state must be
        // able to parse a response where the field is absent.
        let resp = GetSyncStatusResponse {
            passphrase_set: false,
            supabase_configured: false,
            signed_in: false,
            last_sync_ms: None,
            supabase_url: None,
            email: None,
            badge_state: None,
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(
            !s.contains("badge_state"),
            "badge_state must be omitted when None: {s}"
        );
        // Parse it back — badge_state defaults to None.
        let back: GetSyncStatusResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(back.badge_state, None);
    }

    #[test]
    fn get_sync_status_response_parses_without_badge_state() {
        // Simulate a response from a daemon that predates badge_state (backward
        // compat: the field is optional, missing = None).
        let legacy_json = r#"{
            "passphrase_set": false,
            "supabase_configured": true,
            "signed_in": false,
            "last_sync_ms": null
        }"#;
        let resp: GetSyncStatusResponse = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(resp.badge_state, None);
        assert!(resp.supabase_configured);
    }

    // ── db_backup / db_restore (CopyPaste-x94p / CopyPaste-8wbt) ────────────

    #[test]
    fn db_backup_method_has_correct_wire_name() {
        assert_eq!(METHOD_DB_BACKUP, "db_backup");
    }

    #[test]
    fn db_restore_method_has_correct_wire_name() {
        assert_eq!(METHOD_DB_RESTORE, "db_restore");
    }

    #[test]
    fn db_backup_request_roundtrip() {
        let req = DbBackupRequest {
            dest_path: "/tmp/backup.db.enc".to_string(),
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: DbBackupRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
        assert!(s.contains("dest_path"), "wire: {s}");
    }

    #[test]
    fn db_backup_response_roundtrip() {
        // c4q2.22: ok field removed from DbBackupResponse; success is conveyed
        // by the outer Response.ok envelope, not a redundant inner field.
        let resp = DbBackupResponse {
            dest_path: "/tmp/backup.db.enc".to_string(),
            size_bytes: 4096,
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: DbBackupResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(!s.contains("\"ok\""), "no redundant ok field on wire: {s}");
        assert!(s.contains("\"size_bytes\":4096"), "wire: {s}");
    }

    #[test]
    fn db_restore_request_defaults_confirm_false() {
        // An empty params object must parse with confirm = false so a caller who
        // forgets the flag is rejected rather than silently replacing the DB.
        let req: DbRestoreRequest =
            serde_json::from_str(r#"{"src_path": "/tmp/b.db.enc"}"#).unwrap();
        assert!(!req.confirm, "confirm must default to false");
        assert!(!req.force, "force must default to false");
    }

    #[test]
    fn db_restore_request_roundtrip() {
        let req = DbRestoreRequest {
            confirm: true,
            src_path: "/tmp/backup.db.enc".to_string(),
            force: false,
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: DbRestoreRequest = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
        assert!(s.contains("\"confirm\":true"), "wire: {s}");
        assert!(s.contains("src_path"), "wire: {s}");
    }

    #[test]
    fn db_restore_response_roundtrip() {
        // c4q2.22: ok field removed from DbRestoreResponse; success is conveyed
        // by the outer Response.ok envelope, not a redundant inner field.
        let resp = DbRestoreResponse { ready: true };
        let s = serde_json::to_string(&resp).unwrap();
        let back: DbRestoreResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(!s.contains("\"ok\""), "no redundant ok field on wire: {s}");
        assert!(s.contains("\"ready\":true"), "wire: {s}");
    }

    // ── c4q2.23: StatsResponse ───────────────────────────────────────────────

    /// c4q2.23: StatsResponse must survive a JSON round-trip with all fields
    /// intact and must NOT include any field beyond the four declared ones.
    #[test]
    fn stats_response_roundtrip() {
        let resp = StatsResponse {
            total_items: 42,
            sensitive_items: 3,
            version: "1".to_string(),
            build_version: "0.6.0".to_string(),
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: StatsResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(s.contains("\"total_items\":42"), "wire: {s}");
        assert!(s.contains("\"sensitive_items\":3"), "wire: {s}");
        assert!(s.contains("\"version\":\"1\""), "wire: {s}");
        assert!(s.contains("\"build_version\":\"0.6.0\""), "wire: {s}");
    }

    /// Default StatsResponse has all-zero/empty fields.
    #[test]
    fn stats_response_default_is_zero() {
        let resp = StatsResponse::default();
        assert_eq!(resp.total_items, 0);
        assert_eq!(resp.sensitive_items, 0);
        assert_eq!(resp.version, "");
        assert_eq!(resp.build_version, "");
    }

    /// METHOD_STATS has the correct wire name.
    #[test]
    fn stats_method_has_correct_wire_name() {
        assert_eq!(METHOD_STATS, "stats");
    }

    // ── AppConfig IPC wire type tests (CopyPaste-44rq.13 / c4q2.3) ──────────

    #[test]
    fn app_config_default_is_all_none() {
        // Every field in AppConfig is Option<T>; the Default impl must produce
        // all-None so that a bare `AppConfig::default()` sent via set_config
        // is a no-op (no field changes are applied).
        let cfg = AppConfig::default();
        assert!(cfg.p2p_enabled.is_none(), "p2p_enabled");
        assert!(cfg.supabase_url.is_none(), "supabase_url");
        assert!(cfg.supabase_anon_key.is_none(), "supabase_anon_key");
        assert!(cfg.relay_url.is_none(), "relay_url");
        assert!(cfg.supabase_email.is_none(), "supabase_email");
        assert!(cfg.supabase_password.is_none(), "supabase_password");
        assert!(cfg.max_text_size_bytes.is_none(), "max_text_size_bytes");
        assert!(cfg.max_image_size_bytes.is_none(), "max_image_size_bytes");
        assert!(cfg.max_file_size_bytes.is_none(), "max_file_size_bytes");
        assert!(cfg.storage_quota_bytes.is_none(), "storage_quota_bytes");
        assert!(cfg.sensitive_ttl_secs.is_none(), "sensitive_ttl_secs");
        assert!(cfg.image_quality.is_none(), "image_quality");
        assert!(cfg.sync_on_wifi_only.is_none(), "sync_on_wifi_only");
        assert!(cfg.sound_on_copy.is_none(), "sound_on_copy");
        assert!(cfg.notify_on_copy.is_none(), "notify_on_copy");
        assert!(cfg.collect_public_ip.is_none(), "collect_public_ip");
        assert!(cfg.paste_as_plain_text.is_none(), "paste_as_plain_text");
        assert!(
            cfg.excluded_app_bundle_ids.is_none(),
            "excluded_app_bundle_ids"
        );
        assert!(cfg.lan_visibility.is_none(), "lan_visibility");
        assert!(cfg.sync_enabled.is_none(), "sync_enabled");
        assert!(cfg.auto_apply_synced_clip.is_none(), "auto_apply_synced_clip");
    }

    #[test]
    fn app_config_empty_json_deserializes_to_all_none() {
        // An empty JSON object (what a client sends for a no-op set_config)
        // must parse cleanly with every field None.
        let cfg: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn app_config_partial_set_config_roundtrip() {
        // A typical set_config call that only sets relay_url and p2p_enabled.
        // All other fields must remain None so the daemon preserves them.
        let json = r#"{"relay_url":"https://relay.example.com","p2p_enabled":true}"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.relay_url.as_deref(), Some("https://relay.example.com"));
        assert_eq!(cfg.p2p_enabled, Some(true));
        assert!(cfg.max_text_size_bytes.is_none());
        assert!(cfg.supabase_url.is_none());
    }

    #[test]
    fn app_config_serializes_without_none_fields() {
        // skip_serializing_if = "Option::is_none" means absent fields are not
        // emitted, keeping the wire payload small.
        let cfg = AppConfig {
            relay_url: Some("https://relay.example.com".to_owned()),
            sync_enabled: Some(false),
            ..Default::default()
        };
        let s = serde_json::to_string(&cfg).unwrap();
        assert!(s.contains("relay_url"), "relay_url must be present: {s}");
        assert!(s.contains("sync_enabled"), "sync_enabled must be present: {s}");
        // p2p_enabled was None → must NOT appear (would be misread as false).
        assert!(
            !s.contains("p2p_enabled"),
            "None p2p_enabled must be omitted: {s}"
        );
        assert!(
            !s.contains("max_text_size_bytes"),
            "None limit field must be omitted: {s}"
        );
    }

    #[test]
    fn app_config_full_roundtrip() {
        // A fully populated AppConfig survives a serde round-trip.
        let original = AppConfig {
            p2p_enabled: Some(true),
            supabase_url: Some("https://x.supabase.co".to_owned()),
            supabase_anon_key: Some("anon-key".to_owned()),
            relay_url: Some("https://relay.example.com".to_owned()),
            supabase_email: Some("user@example.com".to_owned()),
            supabase_password: Some("s3cr3t".to_owned()),
            max_text_size_bytes: Some(1_048_576),
            max_image_size_bytes: Some(8_388_608),
            max_file_size_bytes: Some(104_857_600),
            storage_quota_bytes: Some(1_073_741_824),
            sensitive_ttl_secs: Some(30),
            image_quality: Some(85),
            sync_on_wifi_only: Some(false),
            sound_on_copy: Some(true),
            notify_on_copy: Some(true),
            collect_public_ip: Some(false),
            paste_as_plain_text: Some(false),
            excluded_app_bundle_ids: Some(vec!["com.1password.1password".to_owned()]),
            lan_visibility: Some(true),
            sync_enabled: Some(true),
            auto_apply_synced_clip: Some(true),
        };
        let s = serde_json::to_string(&original).unwrap();
        let back: AppConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(original, back);
    }
}
