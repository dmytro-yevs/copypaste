//! Shared IPC parameter-extraction helpers (ADR-017 Wave-2 dedup,
//! CopyPaste-vp63.52).
//!
//! `extract_str_param` consolidates the
//! `req.params.get(name).and_then(|v| v.as_str())` + `Response::err_with_code(
//! ERR_CODE_INVALID_ARGUMENT, …)` boilerplate that was duplicated verbatim
//! across `handlers_items*`, `handlers_pairing*`, and `handlers_sync*`.
//!
//! Only call sites whose missing-param handling was byte-identical to this
//! shape (`Some(s) => s.to_string()`, `None => return
//! Response::err_with_code(req.id, ERR_CODE_INVALID_ARGUMENT, msg)`) were
//! converged onto this helper. Sites with additional validation (e.g. a
//! non-empty check via `Some(p) if !p.is_empty()`) or the legacy untyped
//! `Response::err` (no `error_code`) are semantically different and were left
//! unchanged so this pass stays strictly behavior-preserving.

/// Extract a required string parameter from `params`, returning a fully
/// built `ERR_CODE_INVALID_ARGUMENT` error [`crate::protocol::Response`]
/// (bound to `req_id`) when the field is absent or not a JSON string.
///
/// `missing_msg` is the exact error message the caller previously hardcoded
/// (these vary across call sites — e.g. `"missing param: id"` vs `"missing
/// peer_fingerprint"` vs `"missing session_id for step=finish"` — so it is
/// threaded through rather than derived from `name`, keeping every existing
/// wire response byte-for-byte unchanged).
pub(crate) fn extract_str_param(
    params: &serde_json::Value,
    req_id: String,
    name: &str,
    missing_msg: &str,
) -> Result<String, crate::protocol::Response> {
    match params.get(name).and_then(|v| v.as_str()) {
        Some(s) => Ok(s.to_string()),
        None => Err(crate::protocol::Response::err_with_code(
            req_id,
            crate::protocol::ERR_CODE_INVALID_ARGUMENT,
            missing_msg.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_str_param_returns_value_when_present() {
        let params = serde_json::json!({"fingerprint": "aabbcc"});
        let result = extract_str_param(
            &params,
            "1".to_string(),
            "fingerprint",
            "missing param: fingerprint",
        );
        assert_eq!(result.ok(), Some("aabbcc".to_string()));
    }

    #[test]
    fn extract_str_param_errors_with_invalid_argument_when_missing() {
        let params = serde_json::json!({});
        let err = extract_str_param(
            &params,
            "1".to_string(),
            "fingerprint",
            "missing param: fingerprint",
        )
        .expect_err("must error when field is absent");
        assert_eq!(
            err.error_code,
            Some(crate::protocol::ERR_CODE_INVALID_ARGUMENT)
        );
        assert_eq!(err.error.as_deref(), Some("missing param: fingerprint"));
    }

    #[test]
    fn extract_str_param_errors_when_field_is_not_a_string() {
        let params = serde_json::json!({"fingerprint": 42});
        let err = extract_str_param(
            &params,
            "1".to_string(),
            "fingerprint",
            "missing param: fingerprint",
        )
        .expect_err("must error when field is not a string");
        assert_eq!(
            err.error_code,
            Some(crate::protocol::ERR_CODE_INVALID_ARGUMENT)
        );
    }
}
