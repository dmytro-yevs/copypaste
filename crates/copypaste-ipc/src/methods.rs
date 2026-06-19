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

/// Full-text search over clipboard items.
pub const METHOD_SEARCH: &str = "search";

/// Copy a clipboard item back to the system clipboard by id.
pub const METHOD_COPY: &str = "copy";

/// Delete a single clipboard item by id.
pub const METHOD_DELETE: &str = "delete";

/// Delete all clipboard items (clear history).
pub const METHOD_DELETE_ALL: &str = "delete_all";

/// Return the total count of stored clipboard items.
pub const METHOD_COUNT: &str = "count";

/// Return aggregate statistics about the clipboard database.
pub const METHOD_STATS: &str = "stats";

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
/// - `ok` (`bool`): always `true` on the happy path.
/// - `size_before` (`u64`): file size in bytes before the operation.
/// - `size_after` (`u64`): file size in bytes after (same as `size_before` on
///   `dry_run`).
/// - `reclaimed` (`i64`): `size_before - size_after` (negative = file grew,
///   e.g. after `REINDEX` on a fragmented DB).
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VacuumResponse {
    /// Always `true` when the daemon returns `ok` for this method.
    pub ok: bool,
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResetDatabaseResponse {
    /// Always `true` when the daemon returns `ok` for this method; present so
    /// the client can branch on a typed field rather than the bare `ok` flag.
    pub reset: bool,
    /// `true` when the daemon recovered IN-PLACE (no restart needed): the new
    /// empty DB is live and `ready` is now `true`. `false` would tell the UI to
    /// expect the daemon to re-initialise itself; the current implementation
    /// always recovers in-place, so this is `true` on success.
    pub ready: bool,
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

    #[test]
    fn reset_response_roundtrip() {
        let resp = ResetDatabaseResponse {
            reset: true,
            ready: true,
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: ResetDatabaseResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
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

    #[test]
    fn vacuum_response_roundtrip() {
        let resp = VacuumResponse {
            ok: true,
            size_before: 2048,
            size_after: 1024,
            reclaimed: 1024,
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: VacuumResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
    }
}
