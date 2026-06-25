//! Sync-key and cloud-sync METHOD_* constants, plus their DTO types.

use serde::{Deserialize, Serialize};

use super::badge::SyncBadgeState;

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

    /// Canonical Supabase account identity for this device (CopyPaste-1jms.34).
    ///
    /// Computed by `copypaste_supabase::supabase_account_id` from the
    /// combination of the Supabase project URL and the signed-in GoTrue user UUID.
    /// Two paired devices MUST share the same token for RLS to let them see each
    /// other's rows; a mismatch means they are using different Supabase projects
    /// or different GoTrue accounts — their items are silently invisible to each
    /// other.
    ///
    /// This is a **non-secret** stable identifier (not a token/key). `None` when
    /// cloud-sync is off, not configured, or the daemon is in anon-key-only mode
    /// (no GoTrue session).
    ///
    /// Omitted from the wire when `None` for back-compat: daemons that predate
    /// this field simply omit it; consumers must treat absence as `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supabase_account_id: Option<String>,
}

/// Run a live connection diagnostic against the configured cloud backend.
pub const METHOD_CLOUD_TEST_CONNECTION: &str = "cloud_test_connection";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::methods::badge::{SyncBadgeState, SYNC_BADGE_RECENT_MS};

    // Helper constants shared by the sync-status tests.
    const NOW_MS: u64 = 1_000_000_000_000;
    const RECENT_MS: i64 = (NOW_MS - SYNC_BADGE_RECENT_MS + 1_000) as i64;

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
    fn pg62_sync_key_methods_have_correct_wire_names() {
        assert_eq!(METHOD_SET_SYNC_PASSPHRASE, "set_sync_passphrase");
        assert_eq!(METHOD_ROTATE_SYNC_KEY, "rotate_sync_key");
        assert_eq!(METHOD_REVOKE_AND_ROTATE, "revoke_and_rotate");
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
            supabase_account_id: None,
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
            supabase_account_id: None,
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

    /// CopyPaste-1jms.34: `supabase_account_id` must round-trip on the wire when
    /// present and must be omitted from the JSON when `None` (backward-compat with
    /// older consumers that don't know the field).
    #[test]
    fn get_sync_status_response_supabase_account_id_wire() {
        // Present: must survive serde round-trip.
        let with_id = GetSyncStatusResponse {
            passphrase_set: true,
            supabase_configured: true,
            signed_in: true,
            last_sync_ms: None,
            supabase_url: None,
            email: None,
            badge_state: None,
            supabase_account_id: Some(
                "proj_abc123/uid_00000000-0000-0000-0000-000000000001".into(),
            ),
        };
        let s = serde_json::to_string(&with_id).unwrap();
        assert!(
            s.contains("\"supabase_account_id\":\"proj_abc123/uid_00000000"),
            "account_id must be present on the wire: {s}"
        );
        let back: GetSyncStatusResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(back.supabase_account_id, with_id.supabase_account_id);

        // Absent: must be omitted so older consumers can parse without error.
        let without_id = GetSyncStatusResponse {
            passphrase_set: false,
            supabase_configured: false,
            signed_in: false,
            last_sync_ms: None,
            supabase_url: None,
            email: None,
            badge_state: None,
            supabase_account_id: None,
        };
        let s2 = serde_json::to_string(&without_id).unwrap();
        assert!(
            !s2.contains("supabase_account_id"),
            "absent account_id must not appear on the wire: {s2}"
        );
        // Parse a legacy response missing the field — must default to None.
        let legacy = r#"{"passphrase_set":false,"supabase_configured":false,"signed_in":false,"last_sync_ms":null}"#;
        let parsed: GetSyncStatusResponse = serde_json::from_str(legacy).unwrap();
        assert_eq!(
            parsed.supabase_account_id, None,
            "absent field must default to None"
        );
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
}
