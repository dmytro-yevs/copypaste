use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// IPC protocol versioning (ADR-007)
// ---------------------------------------------------------------------------
//
// The alpha line (v0.1.x) shipped without an explicit protocol version field,
// which made it impossible for the daemon and clients to negotiate
// breaking changes safely. Starting with the beta line (v0.2.x) every
// `Request` and `Response` carries an integer `protocol_version`.
//
// Policy:
// * `CURRENT_PROTOCOL_VERSION` is bumped on any **breaking** change to the
//   on-wire shape of `Request`, `Response`, or method semantics
//   (renamed fields, removed methods, changed result types).
// * Backwards-compatible additions (new optional fields, new methods,
//   new error codes) DO NOT bump the version.
// * The daemon accepts any version in `[1..=CURRENT_PROTOCOL_VERSION]`.
//   Requests with `protocol_version` missing are treated as version 1
//   (so legacy alpha clients keep working until they upgrade).
// * Clients that receive `ERR_CODE_VERSION_MISMATCH` MUST refuse to retry
//   the request and surface an upgrade prompt to the user.

/// Wire-format version produced and accepted by this build of the daemon.
/// Bump on every breaking change. See ADR-007 for the full versioning policy.
pub const CURRENT_PROTOCOL_VERSION: u32 = 1;

/// Inclusive lower bound of protocol versions this daemon will still accept.
/// Keep this at 1 until we actively drop alpha-era clients.
pub const MIN_SUPPORTED_PROTOCOL_VERSION: u32 = 1;

/// Default value injected when `protocol_version` is missing from an
/// incoming request — preserves compatibility with alpha clients that
/// pre-date the version field.
fn default_protocol_version() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    /// Wire-format version the client speaks. Missing = treated as `1`
    /// for forward-compat with alpha clients. See ADR-007.
    #[serde(default = "default_protocol_version")]
    pub protocol_version: u32,
}

// ---------------------------------------------------------------------------
// Machine-readable error codes
// ---------------------------------------------------------------------------
//
// Stable identifiers attached alongside the human-readable `error` string so
// clients (UI, CLI, third-party integrations) can branch deterministically
// without parsing English error text. Keep this set small and additive — once
// a code is shipped, never repurpose it.

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
/// Client sent a `protocol_version` outside the daemon's supported range.
/// Surface as an upgrade prompt — DO NOT retry the request. See ADR-007.
pub const ERR_CODE_VERSION_MISMATCH: &str = "version_mismatch";

#[derive(Debug, Serialize)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Stable machine-readable error code. Present on every error response
    /// emitted via [`Response::err_with_code`] or its helpers. Legacy
    /// [`Response::err`] omits this field for back-compat — new code should
    /// prefer the typed helpers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<&'static str>,
    /// Wire-format version this daemon speaks. Always present — clients use
    /// it to detect a mismatch even on error responses. See ADR-007.
    pub protocol_version: u32,
}

impl Response {
    pub fn ok(id: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            ok: true,
            data: Some(data),
            error: None,
            error_code: None,
            protocol_version: CURRENT_PROTOCOL_VERSION,
        }
    }

    /// Untagged error (no machine code). Prefer [`Response::err_with_code`]
    /// for new callsites.
    pub fn err(id: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ok: false,
            data: None,
            error: Some(msg.into()),
            error_code: None,
            protocol_version: CURRENT_PROTOCOL_VERSION,
        }
    }

    /// Error tagged with a stable machine-readable code (`not_found`,
    /// `auth_failed`, `invalid_argument`, `not_implemented`, `ipc_not_ready`,
    /// `internal_error`, `version_mismatch`). Clients should branch on
    /// `error_code`, not on the `error` string.
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
            protocol_version: CURRENT_PROTOCOL_VERSION,
        }
    }

    /// Convenience wrapper for unimplemented methods (cloud-sync stubs, etc.).
    /// Always sets `error_code = "not_implemented"`.
    pub fn not_implemented(id: impl Into<String>, feature: &'static str) -> Self {
        Self::err_with_code(id, ERR_CODE_NOT_IMPLEMENTED, format!("not implemented: {feature}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_deserializes() {
        let json = r#"{"id":"1","method":"list","params":{"limit":10,"offset":0}}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "list");
        assert_eq!(req.id, "1");
    }

    #[test]
    fn response_ok_serializes() {
        let resp = Response::ok("1", serde_json::json!({"total": 0, "items": []}));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(!s.contains("\"error\""));
    }

    #[test]
    fn response_err_serializes() {
        let resp = Response::err("2", "not found");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":false"));
        assert!(s.contains("\"error\":\"not found\""));
        assert!(!s.contains("\"data\""));
        // Legacy `err` omits the code field for back-compat.
        assert!(!s.contains("\"error_code\""));
    }

    #[test]
    fn response_err_with_code_serializes() {
        let resp = Response::err_with_code("9", ERR_CODE_INVALID_ARGUMENT, "bad param");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":false"));
        assert!(s.contains("\"error\":\"bad param\""));
        assert!(s.contains("\"error_code\":\"invalid_argument\""));
    }

    #[test]
    fn response_not_implemented_uses_stable_code() {
        let resp = Response::not_implemented("11", "cloud-sync");
        assert_eq!(resp.error_code, Some(ERR_CODE_NOT_IMPLEMENTED));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"error_code\":\"not_implemented\""));
        assert!(s.contains("not implemented: cloud-sync"));
    }

    // ----- ADR-007: protocol versioning -------------------------------------

    /// Forward-compat: alpha clients that omit `protocol_version` must still
    /// be accepted, and the missing field defaults to `1`.
    #[test]
    fn request_default_version_is_1() {
        let json = r#"{"id":"v1","method":"status","params":{}}"#;
        let req: Request = serde_json::from_str(json).unwrap();
        assert_eq!(req.protocol_version, 1);
    }

    /// A request explicitly carrying the current supported version
    /// deserializes cleanly and round-trips the version.
    #[test]
    fn request_with_supported_version_accepted() {
        let json = format!(
            r#"{{"id":"v2","method":"status","params":{{}},"protocol_version":{}}}"#,
            CURRENT_PROTOCOL_VERSION
        );
        let req: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(req.protocol_version, CURRENT_PROTOCOL_VERSION);
        assert!(req.protocol_version >= MIN_SUPPORTED_PROTOCOL_VERSION);
        assert!(req.protocol_version <= CURRENT_PROTOCOL_VERSION);
    }

    /// Every response — success or error — carries the daemon's version so
    /// clients can detect a downgrade or mismatch even on failure paths.
    #[test]
    fn response_carries_protocol_version() {
        let ok = Response::ok("v3", serde_json::json!({}));
        assert_eq!(ok.protocol_version, CURRENT_PROTOCOL_VERSION);
        let ok_s = serde_json::to_string(&ok).unwrap();
        assert!(ok_s.contains(&format!("\"protocol_version\":{}", CURRENT_PROTOCOL_VERSION)));

        let err = Response::err_with_code("v4", ERR_CODE_VERSION_MISMATCH, "bad version");
        assert_eq!(err.protocol_version, CURRENT_PROTOCOL_VERSION);
        let err_s = serde_json::to_string(&err).unwrap();
        assert!(err_s.contains(&format!("\"protocol_version\":{}", CURRENT_PROTOCOL_VERSION)));
        assert!(err_s.contains("\"error_code\":\"version_mismatch\""));
    }
}
