use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
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
}

impl Response {
    pub fn ok(id: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            ok: true,
            data: Some(data),
            error: None,
            error_code: None,
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
}
