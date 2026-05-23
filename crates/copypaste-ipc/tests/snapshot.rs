//! Beta-bonus wire-format snapshot tests for [`copypaste_ipc::Request`] and
//! [`copypaste_ipc::Response`].
//!
//! These tests pin the **exact serialised JSON shape** that travels over the
//! Unix domain socket so that an accidental field rename, reordering, or
//! `serde` attribute change is caught at `cargo test` time rather than at
//! runtime by a peer that suddenly fails to parse messages.
//!
//! ## Why hand-rolled `assert_eq!` instead of `insta`?
//!
//! `copypaste-ipc` deliberately keeps its dev-dependency surface minimal
//! (only `serde_json` is needed). Each test below builds the expected wire
//! string as a literal so the diff on failure is obvious — no snapshot files
//! to review, no extra crate in the lockfile.
//!
//! ## Field ordering guarantee
//!
//! `serde_json` serialises struct fields in declaration order. The literal
//! JSON strings asserted below intentionally match that declaration order
//! (`Request`: `id, method, params, protocol_version`; `Response`:
//! `id, ok, data, error, error_code, protocol_version`). Reordering fields
//! in the source structs will break these tests on purpose — wire order is
//! part of the observable contract for any peer doing byte-level diffing.

use copypaste_ipc::{Request, Response, ERR_CODE_NOT_FOUND};

// ---------------------------------------------------------------------------
// Request shape
// ---------------------------------------------------------------------------

#[test]
fn request_list_serializes_to_expected_json() {
    let req = Request {
        id: 1,
        method: "list".into(),
        params: serde_json::json!({"limit": 50, "offset": 0}),
        protocol_version: 1,
    };
    let actual = serde_json::to_string(&req).expect("serialize list request");
    let expected =
        r#"{"id":1,"method":"list","params":{"limit":50,"offset":0},"protocol_version":1}"#;
    assert_eq!(actual, expected);
}

#[test]
fn request_insert_with_image_payload_base64_encoded() {
    // Binary payloads (e.g. clipboard images) ride inside `params` as base64
    // strings — the wire is always UTF-8 JSON. Pin that convention here so a
    // future refactor doesn't quietly switch to a binary side-channel.
    let req = Request {
        id: 7,
        method: "insert".into(),
        params: serde_json::json!({
            "kind": "image",
            "mime": "image/png",
            "data_b64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB",
        }),
        protocol_version: 1,
    };
    let actual = serde_json::to_string(&req).expect("serialize insert request");
    let expected = r#"{"id":7,"method":"insert","params":{"data_b64":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB","kind":"image","mime":"image/png"},"protocol_version":1}"#;
    assert_eq!(actual, expected);
}

#[test]
fn request_delete_serializes_id_as_uuid_string() {
    // The clipboard item id sits inside `params`, not at the top level — the
    // top-level `id` is the request correlation id. This test guards the
    // distinction (a regression that swapped the two would silently break
    // every `delete` call from the UI).
    let req = Request {
        id: 99,
        method: "delete".into(),
        params: serde_json::json!({
            "item_id": "550e8400-e29b-41d4-a716-446655440000",
        }),
        protocol_version: 1,
    };
    let actual = serde_json::to_string(&req).expect("serialize delete request");
    let expected = r#"{"id":99,"method":"delete","params":{"item_id":"550e8400-e29b-41d4-a716-446655440000"},"protocol_version":1}"#;
    assert_eq!(actual, expected);
}

#[test]
fn protocol_version_field_always_present_on_requests() {
    // Even a `protocol_version: 0` request (legacy peer) must serialise the
    // field — clients parsing the wire rely on its presence for negotiation.
    let req = Request {
        id: 0,
        method: "ping".into(),
        params: serde_json::Value::Null,
        protocol_version: 0,
    };
    let actual = serde_json::to_string(&req).expect("serialize ping request");
    assert!(
        actual.contains(r#""protocol_version":0"#),
        "protocol_version must always be on the wire, got: {actual}"
    );
    // And explicit full-shape pin for the same case:
    let expected = r#"{"id":0,"method":"ping","params":null,"protocol_version":0}"#;
    assert_eq!(actual, expected);
}

// ---------------------------------------------------------------------------
// Response shape
// ---------------------------------------------------------------------------

#[test]
fn response_ok_with_history_items_array() {
    let resp = Response::ok(
        1,
        serde_json::json!({
            "items": [
                {"id": "a", "preview": "hello"},
                {"id": "b", "preview": "world"},
            ],
            "total": 2,
        }),
    );
    let actual = serde_json::to_string(&resp).expect("serialize ok response");
    // `data` precedes `error*` in struct declaration order; `error` and
    // `error_code` are omitted on success via `skip_serializing_if`.
    let expected = r#"{"id":1,"ok":true,"data":{"items":[{"id":"a","preview":"hello"},{"id":"b","preview":"world"}],"total":2},"protocol_version":0}"#;
    assert_eq!(actual, expected);
}

#[test]
fn response_err_with_error_code_present() {
    // W3.3 contract: `error_code` is snake_case and travels as a top-level
    // sibling of `error`. Clients branch on it, not on the English `error`
    // string.
    let resp = Response::err_with_code(42, ERR_CODE_NOT_FOUND, "item missing");
    let actual = serde_json::to_string(&resp).expect("serialize err response");
    let expected = r#"{"id":42,"ok":false,"error":"item missing","error_code":"not_found","protocol_version":0}"#;
    assert_eq!(actual, expected);
    // Explicit snake_case guard — a rename to e.g. `errorCode` would be a
    // breaking wire change.
    assert!(actual.contains(r#""error_code":"not_found""#));
}

#[test]
fn response_err_without_error_code_omits_field() {
    // Legacy `Response::err` (no machine code) MUST NOT emit an
    // `"error_code":null` field — `skip_serializing_if = "Option::is_none"`
    // guarantees the key is absent entirely. Older peers parsing the wire
    // distinguish "no code provided" from "explicit null code".
    let resp = Response::err(2, "boom");
    let actual = serde_json::to_string(&resp).expect("serialize legacy err response");
    let expected = r#"{"id":2,"ok":false,"error":"boom","protocol_version":0}"#;
    assert_eq!(actual, expected);
    assert!(
        !actual.contains("error_code"),
        "error_code key must be omitted when None, got: {actual}"
    );
    assert!(
        !actual.contains("\"data\""),
        "data key must be omitted when None, got: {actual}"
    );
}
