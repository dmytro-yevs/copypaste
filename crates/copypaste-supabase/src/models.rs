use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Cloud clipboard row (PostgREST / Supabase REST API)
// ---------------------------------------------------------------------------

/// Serde representation of a `clipboard_items` row as seen by PostgREST.
///
/// Used for both:
/// - **Upload** (`POST /rest/v1/clipboard_items`): serialize a row to send to
///   Supabase; `user_id` is omitted because PostgREST fills it from
///   `auth.uid()` via the column default.
/// - **Download** (`GET /rest/v1/clipboard_items`): deserialize incoming rows
///   from the REST endpoint or from a Realtime `postgres_changes` event's
///   `record` field.
///
/// All fields that may be absent in older rows carry `#[serde(default)]` so
/// the struct can deserialize gracefully from rows that predate a column
/// addition (e.g. `deleted` / `pinned` / `pin_order` added in schema v10).
///
/// # `payload_ct` encoding
///
/// The `payload_ct` column is `bytea` in Postgres.  PostgREST returns it in
/// hex output form (`\x<hex>`); on INSERT PostgREST accepts the same
/// `\x<hex>` string assigned to a `bytea` column.  The daemon's
/// `sync_common::decode_payload_ct` / `encode_payload_ct_hex` helpers handle
/// the conversion between raw ciphertext bytes and this wire representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CloudClipboardRow {
    /// Row primary key (UUID string).
    pub id: String,
    /// Stable cross-device item identity (UUID string).
    pub item_id: String,
    /// MIME-like content type — `"text"`, `"image"`, `"file"`.
    pub content_type: String,
    /// Encrypted payload — PostgREST `bytea` in `\x<hex>` form on upload;
    /// returned as `\x<hex>` or bare base64 on download.
    ///
    /// `None` for tombstone rows where `deleted = true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_ct: Option<String>,
    /// Lamport logical clock at the time of last write (LWW conflict
    /// resolution).
    pub lamport_ts: i64,
    /// Wall-clock time (Unix milliseconds) at the time of last write.
    pub wall_time: i64,
    /// Optional TTL expiry in Unix milliseconds.
    #[serde(default)]
    pub expires_at: Option<i64>,
    /// Source app bundle identifier, e.g. `"com.apple.Safari"`.
    #[serde(default)]
    pub app_bundle_id: Option<String>,
    /// UUID of the originating device (maps to `WireItem.origin_device_id`
    /// and the `device_id` column in the Supabase schema).
    pub device_id: String,

    // ── Soft-delete tombstone (schema v10) ───────────────────────────────────
    /// Whether this row is a soft-delete tombstone.
    ///
    /// When `true` the payload was intentionally wiped on the sender.  On
    /// download the receiving daemon should apply LWW merge: if this row's
    /// `lamport_ts` is greater than the local copy, mark the local row deleted
    /// (`deleted = 1`, NULL content).
    ///
    /// `#[serde(default)]` keeps backward compatibility: rows that predate
    /// schema v10 (no `deleted` column) deserialize with `deleted = false`,
    /// which is the correct "live item" interpretation.
    #[serde(default)]
    pub deleted: bool,

    // ── Pin state (schema v10) ────────────────────────────────────────────────
    /// Whether the item is pinned by the user on the originating device.
    ///
    /// Carried so pin state propagates to all devices through the cloud.
    /// `#[serde(default)]` ensures old rows (no `pinned` column) deserialize
    /// as `false` (unpinned).
    #[serde(default)]
    pub pinned: bool,

    /// Explicit sort key among pinned items on the originating device.
    ///
    /// `None` for unpinned items or when no explicit order was assigned.
    /// `#[serde(default)]` ensures old rows (no `pin_order` column)
    /// deserialize as `None`.
    #[serde(default)]
    pub pin_order: Option<f64>,
}

impl CloudClipboardRow {
    /// Return `true` when the row represents a soft-delete tombstone (i.e.
    /// `deleted = true`).  The `payload_ct` of a tombstone is `None`; callers
    /// must apply LWW merge rather than inserting a live item.
    #[inline]
    pub fn is_tombstone(&self) -> bool {
        self.deleted
    }
}

// ---------------------------------------------------------------------------
// GoTrue response types
// ---------------------------------------------------------------------------

/// A GoTrue user object (subset of fields).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: String,
    pub email: Option<String>,
    pub role: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// An active auth session holding both tokens.
///
/// `Debug` is implemented manually to **redact the access and refresh tokens** —
/// these are bearer secrets and must never reach logs, error payloads, or panic
/// messages. Derived `Debug` would print them verbatim.
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    /// Seconds until the access token expires (from the time it was issued).
    pub expires_in: u64,
    /// Absolute Unix timestamp (seconds) when the access token expires.
    /// Computed locally from `expires_in` at the moment the session is created.
    pub expires_at: u64,
    pub token_type: String,
    pub user: User,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("expires_in", &self.expires_in)
            .field("expires_at", &self.expires_at)
            .field("token_type", &self.token_type)
            .field("user", &self.user)
            .finish()
    }
}

impl Session {
    /// Returns `true` when the access token has expired (or will expire within
    /// the provided `margin_secs` seconds).
    pub fn is_expired_with_margin(&self, margin_secs: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // saturating_add prevents u64 overflow when margin_secs is very large
        // (e.g. u64::MAX in tests or a misconfigured caller).
        now.saturating_add(margin_secs) >= self.expires_at
    }
}

// ---------------------------------------------------------------------------
// GoTrue request / response shapes (crate-internal)
// ---------------------------------------------------------------------------

/// Body sent to `POST /auth/v1/token?grant_type=password`.
#[derive(Debug, Serialize)]
pub(crate) struct PasswordGrantRequest<'a> {
    pub email: &'a str,
    pub password: &'a str,
}

/// Body sent to `POST /auth/v1/token?grant_type=refresh_token`.
#[derive(Debug, Serialize)]
pub(crate) struct RefreshGrantRequest<'a> {
    pub refresh_token: &'a str,
}

/// Raw GoTrue token response (both grant types share this shape).
#[derive(Debug, Deserialize)]
pub(crate) struct GoTrueTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
    pub user: User,
}

/// GoTrue error body.
///
/// `error`/`error_code` are the OAuth/structured machine codes GoTrue returns
/// (e.g. `invalid_grant`, `refresh_token_not_found`); both feed [`Self::message`]
/// as last-resort fallbacks. Note `invalid_grant` is emitted for *both* a bad
/// password and a bad refresh token, so callers must classify by grant kind,
/// not by this body — see `auth::AuthClient::post_json`.
#[derive(Debug, Deserialize)]
pub(crate) struct GoTrueErrorBody {
    pub error: Option<String>,
    pub error_description: Option<String>,
    /// Newer GoTrue structured code, e.g. `refresh_token_not_found`.
    pub error_code: Option<String>,
    pub msg: Option<String>,
    pub message: Option<String>,
}

impl GoTrueErrorBody {
    /// Best-effort human-readable message from any of the possible fields.
    pub fn message(&self) -> String {
        self.error_description
            .clone()
            .or_else(|| self.message.clone())
            .or_else(|| self.msg.clone())
            .or_else(|| self.error_code.clone())
            .or_else(|| self.error.clone())
            .unwrap_or_else(|| "unknown error".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> GoTrueErrorBody {
        serde_json::from_str(json).expect("valid GoTrue error body")
    }

    #[test]
    fn message_prefers_description_then_structured_code() {
        // error_description wins when present.
        let b = parse(
            r#"{"error":"invalid_grant","error_description":"Invalid Refresh Token: Already Used"}"#,
        );
        assert_eq!(b.message(), "Invalid Refresh Token: Already Used");

        // Newer GoTrue: structured error_code surfaces when no human text.
        let b = parse(r#"{"code":400,"error_code":"refresh_token_not_found"}"#);
        assert_eq!(b.message(), "refresh_token_not_found");

        // Legacy OAuth code is the last resort.
        let b = parse(r#"{"error":"invalid_grant"}"#);
        assert_eq!(b.message(), "invalid_grant");

        // Nothing usable.
        let b = parse(r#"{}"#);
        assert_eq!(b.message(), "unknown error");
    }

    // ── is_expired_with_margin ─────────────────────────────────────────────────

    /// u64::MAX + any margin_secs must not panic (saturating_add, not wrapping add).
    #[test]
    fn is_expired_with_margin_does_not_overflow_on_max_margin() {
        // A session that expires far in the future (max u64) should not panic
        // even when called with margin_secs = u64::MAX.
        let session = Session {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_in: 3600,
            expires_at: u64::MAX,
            token_type: "bearer".into(),
            user: User {
                id: "u".into(),
                email: None,
                role: None,
                created_at: None,
                updated_at: None,
            },
        };
        // Must not panic — saturating_add prevents overflow.
        let result = session.is_expired_with_margin(u64::MAX);
        // now (real time) << u64::MAX, so (now).saturating_add(u64::MAX) == u64::MAX == expires_at
        // so the result is "expired" (>= boundary). The important thing is it doesn't panic.
        let _ = result; // panic-free is the assertion
    }

    /// A session expiring exactly at now+margin should be considered expired.
    #[test]
    fn is_expired_with_margin_zero_margin_expired_in_past() {
        let session = Session {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_in: 0,
            // expires_at = 0 means it expired at the Unix epoch — definitely expired
            expires_at: 0,
            token_type: "bearer".into(),
            user: User {
                id: "u".into(),
                email: None,
                role: None,
                created_at: None,
                updated_at: None,
            },
        };
        assert!(
            session.is_expired_with_margin(0),
            "session expiring at epoch should always be expired"
        );
    }

    /// A session expiring far in the future should NOT be considered expired.
    #[test]
    fn is_expired_with_margin_future_session_not_expired() {
        let session = Session {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_in: 3600,
            // Year 2100 in Unix seconds — well in the future
            expires_at: 4_102_444_800,
            token_type: "bearer".into(),
            user: User {
                id: "u".into(),
                email: None,
                role: None,
                created_at: None,
                updated_at: None,
            },
        };
        // With a 60-second margin, a session expiring in 2100 is not expired.
        assert!(
            !session.is_expired_with_margin(60),
            "session expiring in 2100 should not be expired"
        );
    }

    // ── CloudClipboardRow ─────────────────────────────────────────────────────

    fn live_row() -> CloudClipboardRow {
        CloudClipboardRow {
            id: "row-uuid-1".into(),
            item_id: "item-uuid-1".into(),
            content_type: "text".into(),
            payload_ct: Some("\\xdeadbeef".into()),
            lamport_ts: 42,
            wall_time: 1_700_000_000_000,
            expires_at: None,
            app_bundle_id: None,
            device_id: "device-a".into(),
            deleted: false,
            pinned: false,
            pin_order: None,
        }
    }

    /// A round-trip through serde_json must be lossless for all fields.
    #[test]
    fn cloud_clipboard_row_round_trips() {
        let row = live_row();
        let json = serde_json::to_string(&row).expect("serialize");
        let decoded: CloudClipboardRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, row);
    }

    /// `deleted`, `pinned`, and `pin_order` must be present in the serialized
    /// output so they are sent to PostgREST on upload.
    #[test]
    fn cloud_clipboard_row_serializes_deleted_pinned_pin_order() {
        let mut row = live_row();
        row.deleted = true;
        row.pinned = true;
        row.pin_order = Some(1.5);

        let json = serde_json::to_string(&row).expect("serialize");
        assert!(
            json.contains("\"deleted\":true"),
            "deleted must be in JSON: {json}"
        );
        assert!(
            json.contains("\"pinned\":true"),
            "pinned must be in JSON: {json}"
        );
        assert!(
            json.contains("\"pin_order\":1.5"),
            "pin_order must be in JSON: {json}"
        );
    }

    /// Rows from an older schema (no `deleted`, `pinned`, `pin_order` columns)
    /// must deserialize with safe defaults: `deleted=false`, `pinned=false`,
    /// `pin_order=None`.
    #[test]
    fn cloud_clipboard_row_defaults_for_missing_schema_v10_fields() {
        // JSON as a pre-v10 PostgREST response would look — no deleted/pinned/pin_order.
        let json = r#"{
            "id": "r1",
            "item_id": "i1",
            "content_type": "text",
            "payload_ct": "\\xabcd",
            "lamport_ts": 1,
            "wall_time": 1000,
            "device_id": "dev-x"
        }"#;
        let row: CloudClipboardRow = serde_json::from_str(json).expect("deserialize legacy row");
        assert!(!row.deleted, "absent `deleted` must default to false");
        assert!(!row.pinned, "absent `pinned` must default to false");
        assert!(
            row.pin_order.is_none(),
            "absent `pin_order` must default to None"
        );
    }

    /// A tombstone row (deleted=true) must be identified by `is_tombstone()`.
    #[test]
    fn cloud_clipboard_row_is_tombstone_on_deleted() {
        let mut row = live_row();
        assert!(!row.is_tombstone(), "live row must not be a tombstone");
        row.deleted = true;
        assert!(row.is_tombstone(), "deleted=true row must be a tombstone");
    }

    /// `payload_ct: None` must be omitted from the serialized JSON (not sent as
    /// `"payload_ct":null`) so tombstone rows don't accidentally overwrite a
    /// valid bytea column with null.
    #[test]
    fn cloud_clipboard_row_tombstone_omits_payload_ct_from_json() {
        let mut row = live_row();
        row.deleted = true;
        row.payload_ct = None;

        let json = serde_json::to_string(&row).expect("serialize tombstone");
        assert!(
            !json.contains("payload_ct"),
            "payload_ct must be omitted when None; got: {json}"
        );
    }
}
