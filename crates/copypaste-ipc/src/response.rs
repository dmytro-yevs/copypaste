//! IPC response wire type + stable error codes.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Machine-readable error codes
// ---------------------------------------------------------------------------
//
// Stable identifiers attached alongside the human-readable `error` string so
// clients (UI, CLI, third-party integrations) can branch deterministically
// without parsing English error text. Keep this set small and additive —
// once a code is shipped, never repurpose it.

/// Requested resource (item id, peer, etc.) does not exist.
pub const ERR_CODE_NOT_FOUND: &str = "not_found";
/// Authentication failed — bad credentials, expired token, missing keychain entry.
pub const ERR_CODE_AUTH_FAILED: &str = "auth_failed";
/// Request was structurally valid JSON but violated parameter contract
/// (missing field, wrong type, invalid format).
pub const ERR_CODE_INVALID_ARGUMENT: &str = "invalid_argument";
/// Method is recognised but not yet implemented (cloud-sync stubs, etc.).
pub const ERR_CODE_NOT_IMPLEMENTED: &str = "not_implemented";
/// Daemon is still booting — database/cloud not yet ready to serve requests.
pub const ERR_CODE_IPC_NOT_READY: &str = "ipc_not_ready";
/// Catch-all for unexpected daemon-side failures (I/O, panics, db errors).
pub const ERR_CODE_INTERNAL_ERROR: &str = "internal_error";
/// The v4 key-rotation sweep is still in progress. Ingest paths return this
/// rather than writing new items to avoid mixing key versions during the
/// sweep. Clients should back off and retry after a short delay.
pub const ERR_CODE_MIGRATION_IN_PROGRESS: &str = "migration_in_progress";
/// Wire protocol version mismatch between peers. The receiver must reject the
/// message and the sender must upgrade.
pub const ERR_CODE_VERSION_MISMATCH: &str = "version_mismatch";
/// Request rejected because the caller exceeded a rate limit. Clients should
/// back off and retry after a delay.
pub const ERR_CODE_RATE_LIMITED: &str = "rate_limited";
/// Daemon socket is missing or refused connection. Emitted by the UI/CLI
/// client when the daemon is not running.
pub const ERR_CODE_DAEMON_OFFLINE: &str = "daemon_offline";

/// A single JSON-RPC-style response emitted by the daemon for a matching
/// [`crate::Request`].
///
/// On success `ok = true`, `data = Some(payload)`, `error*` are `None`.
/// On failure `ok = false`, `data = None`, `error = Some(message)`, and
/// (preferred) `error_code = Some(stable_machine_code)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Response {
    /// Echoed [`crate::Request::id`]. Serialised as a JSON **string** to match
    /// the daemon wire format — see [`crate::Request::id`] for rationale.
    pub id: String,
    /// `true` on success, `false` on failure.
    pub ok: bool,
    /// Method-specific success payload. Omitted from the wire when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Human-readable error message. Omitted from the wire when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Stable machine-readable error code (one of the `ERR_CODE_*` constants
    /// in this module). Clients should branch on this field, *not* on the
    /// `error` string. Omitted on success and on legacy [`Response::err`]
    /// constructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<&'static str>,
    /// Wire protocol version. See [`crate::PROTOCOL_VERSION`].
    #[serde(default)]
    pub protocol_version: u32,
}

impl Response {
    /// Build a success response carrying `data`.
    pub fn ok(id: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            ok: true,
            data: Some(data),
            error: None,
            error_code: None,
            protocol_version: crate::PROTOCOL_VERSION,
        }
    }

    /// Untagged error (no machine code). Prefer [`Response::err_with_code`]
    /// for new call sites so clients can branch deterministically.
    pub fn err(id: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ok: false,
            data: None,
            error: Some(msg.into()),
            error_code: None,
            protocol_version: crate::PROTOCOL_VERSION,
        }
    }

    /// Error tagged with a stable machine-readable code (`not_found`,
    /// `auth_failed`, `invalid_argument`, `not_implemented`, `ipc_not_ready`,
    /// `internal_error`). Clients should branch on `error_code`, not on the
    /// `error` string.
    pub fn err_with_code(
        id: impl Into<String>,
        code: &'static str,
        msg: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            ok: false,
            data: None,
            error: Some(msg.into()),
            error_code: Some(code),
            protocol_version: crate::PROTOCOL_VERSION,
        }
    }

    /// Convenience wrapper for unimplemented methods (cloud-sync stubs, etc.).
    /// Always sets `error_code = "not_implemented"`.
    pub fn not_implemented(id: impl Into<String>, feature: &'static str) -> Self {
        Self::err_with_code(
            id,
            ERR_CODE_NOT_IMPLEMENTED,
            format!("not implemented: {feature}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Request;

    #[test]
    fn request_serialize_roundtrip() {
        let req = Request {
            id: "42".into(),
            method: "list".into(),
            params: serde_json::json!({"limit": 10, "offset": 0}),
            protocol_version: 1,
        };
        let s = serde_json::to_string(&req).unwrap();
        let back: Request = serde_json::from_str(&s).unwrap();
        assert_eq!(req, back);
        // id must be serialised as a JSON string, not a number
        assert!(s.contains(r#""id":"42""#), "id must be JSON string, got: {s}");
        // params default applies when absent on the wire (string id)
        let minimal: Request =
            serde_json::from_str(r#"{"id":"7","method":"ping"}"#).unwrap();
        assert_eq!(minimal.id, "7");
        assert_eq!(minimal.method, "ping");
        assert_eq!(minimal.params, serde_json::Value::Null);
        assert_eq!(minimal.protocol_version, 0);
    }

    #[test]
    fn response_omits_none_fields() {
        let ok = Response::ok("1", serde_json::json!({"total": 0}));
        let s = serde_json::to_string(&ok).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("\"data\""));
        assert!(!s.contains("\"error\""));
        assert!(!s.contains("\"error_code\""));

        let legacy_err = Response::err("2", "boom");
        let s = serde_json::to_string(&legacy_err).unwrap();
        assert!(s.contains("\"ok\":false"));
        assert!(s.contains("\"error\":\"boom\""));
        assert!(!s.contains("\"data\""));
        assert!(!s.contains("\"error_code\""));

        let tagged = Response::err_with_code("3", ERR_CODE_INVALID_ARGUMENT, "bad param");
        let s = serde_json::to_string(&tagged).unwrap();
        assert!(s.contains("\"error_code\":\"invalid_argument\""));
    }

    #[test]
    fn response_not_implemented_helper() {
        let resp = Response::not_implemented("11", "cloud-sync");
        assert_eq!(resp.id, "11");
        assert!(!resp.ok);
        assert_eq!(resp.error_code, Some(ERR_CODE_NOT_IMPLEMENTED));
        assert_eq!(resp.error.as_deref(), Some("not implemented: cloud-sync"));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"error_code\":\"not_implemented\""));
    }
}
