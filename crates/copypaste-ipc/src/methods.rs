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

/// Query the current cloud-sync state.
pub const METHOD_GET_SYNC_STATUS: &str = "get_sync_status";

/// Run a live connection diagnostic against the configured cloud backend.
pub const METHOD_CLOUD_TEST_CONNECTION: &str = "cloud_test_connection";

// ── Pairing ─────────────────────────────────────────────────────────────────

/// Generate a short-lived QR pairing payload.
pub const METHOD_PAIR_GENERATE_QR: &str = "pair_generate_qr";

// ── Database maintenance ────────────────────────────────────────────────────

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
}
