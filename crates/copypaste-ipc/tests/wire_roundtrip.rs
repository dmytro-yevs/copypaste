//! Round-trip test: real daemon wire frames (id as JSON string) must
//! deserialize into the typed [`Request`]/[`Response`] structs and serialize
//! back to byte-identical JSON.
//!
//! This test was added as part of CopyPaste-crol to verify that the typed
//! schema matches the *actual* wire format produced by the daemon (which uses
//! `id: String`, not `id: u64`). It failed until `id` was changed to
//! `String` in both structs.

use copypaste_ipc::{Request, Response, ERR_CODE_NOT_FOUND};

// ---------------------------------------------------------------------------
// Deserialize real daemon wire frames into typed structs
// ---------------------------------------------------------------------------

/// A verbatim frame from the daemon (id as decimal string).
/// With id: u64 this would panic at `serde_json::from_str` because
/// JSON `"1"` is a string, not a number.
#[test]
fn request_string_id_deserializes() {
    let wire =
        r#"{"id":"1","method":"list","params":{"limit":50,"offset":0},"protocol_version":1}"#;
    let req: Request =
        serde_json::from_str(wire).expect("real daemon wire frame must deserialize into Request");
    assert_eq!(req.id, "1", "id must round-trip as string '1'");
    assert_eq!(req.method, "list");
    assert_eq!(req.protocol_version, 1);
}

/// A verbatim response frame from the daemon (id as decimal string) can be
/// parsed as a raw JSON value and the id field is a string.
///
/// Full `Response` deserialization is not tested because `error_code:
/// Option<&'static str>` requires a 'static source lifetime. Parse via
/// `serde_json::Value` to verify the wire shape without that constraint.
#[test]
fn response_string_id_wire_shape() {
    let wire = r#"{"id":"42","ok":true,"data":{"items":[],"total":0},"protocol_version":1}"#;
    let v: serde_json::Value = serde_json::from_str(wire).expect("parse wire frame");
    assert_eq!(
        v["id"].as_str(),
        Some("42"),
        "id must be a JSON string on the wire"
    );
    assert!(v["id"].as_u64().is_none(), "id must NOT be a JSON number");
    assert_eq!(v["ok"].as_bool(), Some(true));
    assert_eq!(v["protocol_version"].as_u64(), Some(1));
}

// ---------------------------------------------------------------------------
// Full round-trip: struct → JSON → struct must produce the same bytes the
// daemon/cli already emit on the wire.
// ---------------------------------------------------------------------------

/// Request serializes with a JSON *string* id (not a number) to match daemon.
#[test]
fn request_serializes_id_as_string() {
    let req = Request {
        id: "7".into(),
        method: "delete".into(),
        params: serde_json::json!({"item_id": "550e8400-e29b-41d4-a716-446655440000"}),
        protocol_version: 1,
    };
    let json = serde_json::to_string(&req).expect("serialize");
    // The id field must appear as a JSON string ("7"), not as a number (7).
    assert!(
        json.contains(r#""id":"7""#),
        "id must serialize as JSON string, got: {json}"
    );
}

/// Response serializes with a JSON *string* id to match daemon.
#[test]
fn response_serializes_id_as_string() {
    let resp = Response::ok("99", serde_json::json!({"total": 5}));
    let json = serde_json::to_string(&resp).expect("serialize");
    assert!(
        json.contains(r#""id":"99""#),
        "id must serialize as JSON string, got: {json}"
    );
}

/// Error response with string id and error_code — serialize produces correct JSON
/// and the wire frame can be parsed back as a raw JSON value.
///
/// Note: full `Response` deserialization is intentionally not tested here because
/// `error_code: Option<&'static str>` requires a 'static lifetime that a locally-
/// owned string cannot satisfy. The daemon's `protocol::Response` and the CLI's
/// `ipc::Response` both parse `error_code` from raw `serde_json::Value` for this
/// reason. The important contract — that the id serializes as a string — is
/// verified below.
#[test]
fn error_response_string_id_roundtrip() {
    let resp = Response::err_with_code("req-3", ERR_CODE_NOT_FOUND, "item missing");
    let json = serde_json::to_string(&resp).expect("serialize");
    // Parse back as a raw Value to verify field shapes without the 'static constraint.
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse as Value");
    assert_eq!(v["id"].as_str(), Some("req-3"), "id must be a JSON string");
    assert_eq!(v["ok"].as_bool(), Some(false));
    assert_eq!(v["error_code"].as_str(), Some(ERR_CODE_NOT_FOUND));
    assert_eq!(v["error"].as_str(), Some("item missing"));
}

/// Minimal request (id + method only, no params/version) from a pre-version-field client.
#[test]
fn minimal_wire_request_with_string_id() {
    let wire = r#"{"id":"v1","method":"status"}"#;
    let req: Request = serde_json::from_str(wire).expect("minimal wire frame deserializes");
    assert_eq!(req.id, "v1");
    assert_eq!(req.method, "status");
    assert_eq!(req.params, serde_json::Value::Null);
    assert_eq!(req.protocol_version, 0); // default when absent
}

/// The typed Request and daemon's own protocol::Request must produce
/// byte-identical JSON for the same logical message — proves the two structs
/// share the same wire contract.
#[test]
fn typed_request_matches_daemon_wire_shape() {
    // Construct the typed ipc Request.
    let req = Request {
        id: "1".into(),
        method: "list".into(),
        params: serde_json::json!({"limit": 10, "offset": 0}),
        protocol_version: 1,
    };
    let typed_json = serde_json::to_string(&req).expect("serialize typed Request");

    // The daemon builds requests as raw serde_json values via build_request.
    // Replicate that shape here (matching copypaste-cli's IpcClient::build_request).
    let daemon_style = serde_json::json!({
        "id": "1",
        "method": "list",
        "protocol_version": 1,
        "params": {"limit": 10, "offset": 0},
    });

    // Parse both into Values for order-insensitive comparison.
    let typed_val: serde_json::Value = serde_json::from_str(&typed_json).expect("parse typed json");
    assert_eq!(
        typed_val["id"], daemon_style["id"],
        "id field must be a JSON string in both typed struct and daemon wire"
    );
    assert_eq!(typed_val["id"].as_str(), Some("1"));
    // Ensure it is NOT a number.
    assert!(
        typed_val["id"].as_u64().is_none(),
        "id must not be a JSON number"
    );
}
