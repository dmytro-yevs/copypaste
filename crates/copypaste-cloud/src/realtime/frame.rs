//! The Phoenix envelope and the `postgres_changes` payload.
//!
//! Everything a hostile or merely surprising server can put on the wire is
//! parsed here, and nowhere else. The two rules that matter — reject anything
//! that is not exactly the envelope, and never log the bytes — hold because
//! this is the only door.

use serde_json::Value;

use super::event::{RealtimeError, RealtimeEvent};
use super::TABLE;
use crate::rest::CloudItem;

/// The Phoenix envelope.
#[derive(Debug, PartialEq)]
pub(super) struct Frame {
    /// `None` when the field is null *or* not a string. See the module docs.
    pub join_ref: Option<String>,
    /// Same rule as `join_ref`.
    pub msg_ref: Option<String>,
    pub topic: String,
    pub event: String,
    pub payload: Value,
}

/// Reject anything that is not exactly a five-element array of the right
/// shapes. A frame is attacker-influenced input; it gets a parser, not an
/// index.
pub(super) fn parse_frame(text: &str) -> Result<Frame, RealtimeError> {
    let value: Value =
        serde_json::from_str(text).map_err(|_| RealtimeError::Protocol("frame is not json"))?;
    let array = value
        .as_array()
        .ok_or(RealtimeError::Protocol("frame is not an array"))?;
    if array.len() != 5 {
        return Err(RealtimeError::Protocol("frame is not five elements"));
    }

    // A ref that is not a string is *absent*, not `Some("")`. See the module
    // docs and `CopyPaste-crh3.97`.
    let as_ref = |v: &Value| v.as_str().map(str::to_owned);

    Ok(Frame {
        join_ref: as_ref(&array[0]),
        msg_ref: as_ref(&array[1]),
        topic: array[2]
            .as_str()
            .ok_or(RealtimeError::Protocol("frame topic is not a string"))?
            .to_owned(),
        event: array[3]
            .as_str()
            .ok_or(RealtimeError::Protocol("frame event is not a string"))?
            .to_owned(),
        payload: array[4].clone(),
    })
}

/// What one inbound frame means to us.
pub(super) enum Dispatch {
    Event(RealtimeEvent),
    Failed(RealtimeError),
    Closed,
    Nothing,
}

pub(super) fn dispatch(text: &str) -> Dispatch {
    let frame = match parse_frame(text) {
        Ok(f) => f,
        Err(e) => {
            log_unparseable(text);
            return Dispatch::Failed(e);
        }
    };

    match frame.event.as_str() {
        "postgres_changes" => match change_event(&frame.payload) {
            Ok(Some(event)) => Dispatch::Event(event),
            Ok(None) => Dispatch::Nothing,
            Err(e) => {
                log_unparseable(text);
                Dispatch::Failed(e)
            }
        },
        // A re-join confirmation after a reconnect, or a heartbeat reply.
        "phx_reply" => Dispatch::Nothing,
        "phx_close" => Dispatch::Closed,
        "phx_error" => {
            // The payload can embed row data; log that it happened, not what
            // it said.
            tracing::warn!("realtime channel reported an error");
            Dispatch::Closed
        }
        _ => Dispatch::Nothing,
    }
}

/// Turn a `postgres_changes` payload into an event.
///
/// `Ok(None)` means "a change we do not model" — a change to another table, or
/// an event type we do not handle — which is not an error.
///
/// The payload shape is defensively read (manifest 05 §4.7): the change lives
/// under `payload.data`, `record`/`new` and `old_record`/`old` are both
/// accepted spellings, and the type is compared case-insensitively.
fn change_event(payload: &Value) -> Result<Option<RealtimeEvent>, RealtimeError> {
    let data = payload
        .get("data")
        .ok_or(RealtimeError::Protocol("change payload has no data"))?;

    if let Some(table) = data.get("table").and_then(Value::as_str) {
        if table != TABLE {
            return Ok(None);
        }
    }

    let kind = data
        .get("type")
        .and_then(Value::as_str)
        .ok_or(RealtimeError::Protocol("change payload has no type"))?
        .to_ascii_uppercase();

    let record = || data.get("record").or_else(|| data.get("new"));
    let old = || data.get("old_record").or_else(|| data.get("old"));

    match kind.as_str() {
        "INSERT" | "UPDATE" => {
            let row = record().ok_or(RealtimeError::Protocol("change payload has no record"))?;
            let item: CloudItem = serde_json::from_value(row.clone())
                .map_err(|_| RealtimeError::Protocol("record is not a clipboard row"))?;
            Ok(Some(if kind == "INSERT" {
                RealtimeEvent::Insert(item)
            } else {
                RealtimeEvent::Update(item)
            }))
        }
        "DELETE" => {
            let item_id = old()
                .and_then(|o| o.get("item_id"))
                .and_then(Value::as_str)
                .ok_or(RealtimeError::Protocol("delete payload carries no item id"))?
                .to_owned();
            Ok(Some(RealtimeEvent::Delete { item_id }))
        }
        _ => Ok(None),
    }
}

/// Log that a frame could not be parsed, without logging the frame.
///
/// Frame bytes may contain ciphertext, metadata, or a bearer token. Length is
/// sufficient to identify a repeated protocol failure without retaining any
/// reversible part of attacker-controlled input.
fn log_unparseable(text: &str) {
    tracing::warn!(len = text.len(), "realtime frame could not be parsed");
}

// Tests
//
// Frames are strings; nothing here opens a socket.

#[cfg(test)]
mod tests {
    use super::super::TOPIC;
    use super::*;

    // --- the envelope -----------------------------------------------------

    #[test]
    fn a_five_element_frame_parses() {
        let frame =
            parse_frame(r#"["1","2","realtime:clipboard_items","phx_reply",{"status":"ok"}]"#)
                .unwrap();
        assert_eq!(frame.join_ref.as_deref(), Some("1"));
        assert_eq!(frame.msg_ref.as_deref(), Some("2"));
        assert_eq!(frame.topic, TOPIC);
        assert_eq!(frame.event, "phx_reply");
        assert_eq!(frame.payload["status"], "ok");
    }

    #[test]
    fn a_numeric_ref_is_absent_not_empty() {
        // CopyPaste-crh3.97. `Some("")` here made every heartbeat reply look
        // like it belonged to a different push.
        let frame = parse_frame(r#"[1,2,"phoenix","phx_reply",{}]"#).unwrap();
        assert_eq!(frame.join_ref, None);
        assert_eq!(frame.msg_ref, None);
    }

    #[test]
    fn a_null_ref_is_absent() {
        let frame = parse_frame(r#"[null,"3","phoenix","heartbeat",{}]"#).unwrap();
        assert_eq!(frame.join_ref, None);
        assert_eq!(frame.msg_ref.as_deref(), Some("3"));
    }

    #[test]
    fn frames_of_the_wrong_arity_are_rejected() {
        for text in [
            r#"["1","1","t","e"]"#,
            r#"["1","1","t","e",{},"extra"]"#,
            r#"[]"#,
            r#"{"join_ref":"1"}"#,
            r#"not json"#,
        ] {
            assert!(parse_frame(text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn a_frame_with_a_non_string_topic_or_event_is_rejected() {
        assert!(parse_frame(r#"["1","1",7,"phx_reply",{}]"#).is_err());
        assert!(parse_frame(r#"["1","1","t",7,{}]"#).is_err());
    }

    // --- change payloads --------------------------------------------------

    fn row_json(item_id: &str, deleted: bool) -> String {
        format!(
            r#"{{"item_id":"{item_id}","ciphertext":"Y3Q=","nonce":"bm9uY2U=",
                 "content_type":"text","created_at":1700000000000,
                 "deleted":{deleted},"origin_device_id":"dev-a"}}"#
        )
    }

    #[test]
    fn an_insert_becomes_an_insert_event() {
        let payload: Value = serde_json::from_str(&format!(
            r#"{{"data":{{"type":"INSERT","table":"clipboard_items","record":{}}}}}"#,
            row_json("item-1", false)
        ))
        .unwrap();

        match change_event(&payload).unwrap().unwrap() {
            RealtimeEvent::Insert(item) => assert_eq!(item.item_id, "item-1"),
            other => panic!("expected an insert, got {other:?}"),
        }
    }

    #[test]
    fn an_update_carrying_a_tombstone_becomes_an_update_event() {
        // A delete is a tombstone — an UPDATE with `deleted = true`. This is the
        // path a real delete travels; the DELETE event type is the exception.
        let payload: Value = serde_json::from_str(&format!(
            r#"{{"data":{{"type":"UPDATE","table":"clipboard_items","new":{}}}}}"#,
            row_json("item-2", true)
        ))
        .unwrap();

        match change_event(&payload).unwrap().unwrap() {
            RealtimeEvent::Update(item) => {
                assert_eq!(item.item_id, "item-2");
                assert!(item.deleted);
            }
            other => panic!("expected an update, got {other:?}"),
        }
    }

    #[test]
    fn the_new_and_old_spellings_are_both_accepted() {
        let with_new: Value = serde_json::from_str(&format!(
            r#"{{"data":{{"type":"insert","new":{}}}}}"#,
            row_json("item-3", false)
        ))
        .unwrap();
        assert!(matches!(
            change_event(&with_new).unwrap().unwrap(),
            RealtimeEvent::Insert(_)
        ));

        let with_old: Value =
            serde_json::from_str(r#"{"data":{"type":"delete","old":{"item_id":"item-4"}}}"#)
                .unwrap();
        match change_event(&with_old).unwrap().unwrap() {
            RealtimeEvent::Delete { item_id } => assert_eq!(item_id, "item-4"),
            other => panic!("expected a delete, got {other:?}"),
        }
    }

    #[test]
    fn a_delete_without_an_item_id_is_a_protocol_error_not_a_guess() {
        // Guessing an id here would delete the wrong row. The correct response
        // is to report and let the poll loop reconcile.
        let payload: Value =
            serde_json::from_str(r#"{"data":{"type":"DELETE","old_record":{"id":"row-pk"}}}"#)
                .unwrap();
        assert!(change_event(&payload).is_err());
    }

    #[test]
    fn a_change_to_another_table_is_ignored_not_an_error() {
        let payload: Value = serde_json::from_str(
            r#"{"data":{"type":"INSERT","table":"other","record":{"item_id":"x"}}}"#,
        )
        .unwrap();
        assert!(change_event(&payload).unwrap().is_none());
    }

    #[test]
    fn a_malformed_record_is_an_error_not_a_partial_item() {
        // INV-N3 in miniature: never manufacture a half-filled row.
        let payload: Value =
            serde_json::from_str(r#"{"data":{"type":"INSERT","record":{"item_id":"x"}}}"#).unwrap();
        assert!(change_event(&payload).is_err());
    }

    #[test]
    fn dispatch_routes_the_lifecycle_events() {
        assert!(matches!(
            dispatch(r#"["1","1","realtime:clipboard_items","phx_reply",{"status":"ok"}]"#),
            Dispatch::Nothing
        ));
        assert!(matches!(
            dispatch(r#"["1","1","realtime:clipboard_items","phx_close",{}]"#),
            Dispatch::Closed
        ));
        assert!(matches!(
            dispatch(r#"["1","1","realtime:clipboard_items","phx_error",{}]"#),
            Dispatch::Closed
        ));
        assert!(matches!(dispatch("{not a frame"), Dispatch::Failed(_)));
        assert!(matches!(
            dispatch(&format!(
                r#"["1","1","realtime:clipboard_items","postgres_changes",{{"data":{{"type":"INSERT","record":{}}}}}]"#,
                row_json("item-5", false)
            )),
            Dispatch::Event(RealtimeEvent::Insert(_))
        ));
    }
}
