//! The Phoenix envelope and the `postgres_changes` payload.
//!
//! Everything a hostile or merely surprising server can put on the wire is
//! parsed here, and nowhere else. The two rules that matter — reject anything
//! that is not exactly the envelope, and never log the bytes — hold because
//! this is the only door.

use std::fmt;

use serde::de::{self, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use super::event::{RealtimeError, RealtimeEvent};
use super::TABLE;

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
    Event(Box<RealtimeEvent>),
    Failed(RealtimeError),
    Closed,
    Nothing,
}

pub(super) fn dispatch(text: &str) -> Dispatch {
    let inbound: Inbound = match serde_json::from_str(text) {
        Ok(inbound) => inbound,
        Err(_) => {
            log_unparseable(text);
            return Dispatch::Failed(RealtimeError::Protocol("frame is not a known envelope"));
        }
    };

    match inbound.event.as_str() {
        "postgres_changes" => match change_event(inbound.change) {
            Ok(Some(event)) => Dispatch::Event(Box::new(event)),
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

/// An inbound frame as dispatch reads it: the event name and, for a change,
/// only what identifies it. Every other value, the row's ciphertext included,
/// is skipped without being allocated.
struct Inbound {
    event: String,
    change: Option<ChangeData>,
}

impl<'de> Deserialize<'de> for Inbound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(InboundVisitor)
    }
}

struct InboundVisitor;

impl<'de> Visitor<'de> for InboundVisitor {
    type Value = Inbound;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a five-element Phoenix frame")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Inbound, A::Error> {
        let short = |len| de::Error::invalid_length(len, &"five elements");
        seq.next_element::<IgnoredAny>()?.ok_or_else(|| short(0))?;
        seq.next_element::<IgnoredAny>()?.ok_or_else(|| short(1))?;
        seq.next_element::<String>()?.ok_or_else(|| short(2))?;
        let event: String = seq.next_element()?.ok_or_else(|| short(3))?;
        let change = if event == "postgres_changes" {
            seq.next_element::<ChangePayload>()?
                .ok_or_else(|| short(4))?
                .data
        } else {
            seq.next_element::<IgnoredAny>()?.ok_or_else(|| short(4))?;
            None
        };
        if seq.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::invalid_length(6, &"five elements"));
        }
        Ok(Inbound { event, change })
    }
}

#[derive(Deserialize)]
struct ChangePayload {
    data: Option<ChangeData>,
}

/// `record`/`new` and `old_record`/`old` are both accepted spellings.
#[derive(Deserialize)]
struct ChangeData {
    table: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    record: Option<RowId>,
    new: Option<RowId>,
    old_record: Option<RowId>,
    old: Option<RowId>,
}

#[derive(Deserialize)]
struct RowId {
    item_id: Option<String>,
}

/// Turn a `postgres_changes` payload into an event.
///
/// `Ok(None)` means "a change we do not model" — a change to another table, or
/// an event type we do not handle — which is not an error. The type is
/// compared case-insensitively (manifest 05 §4.7).
fn change_event(data: Option<ChangeData>) -> Result<Option<RealtimeEvent>, RealtimeError> {
    let data = data.ok_or(RealtimeError::Protocol("change payload has no data"))?;

    if let Some(table) = data.table.as_deref() {
        if table == crate::rest::group::GROUP_TABLE {
            return Ok(data
                .kind
                .as_deref()
                .is_some_and(|kind| {
                    kind.eq_ignore_ascii_case("INSERT") || kind.eq_ignore_ascii_case("UPDATE")
                })
                .then_some(RealtimeEvent::GroupChanged));
        }
        if table != TABLE {
            return Ok(None);
        }
    }

    let kind = data
        .kind
        .ok_or(RealtimeError::Protocol("change payload has no type"))?
        .to_ascii_uppercase();

    match kind.as_str() {
        "INSERT" | "UPDATE" => {
            let item_id = data
                .record
                .or(data.new)
                .ok_or(RealtimeError::Protocol("change payload has no record"))?
                .item_id
                .ok_or(RealtimeError::Protocol("record is not a clipboard row"))?;
            Ok(Some(if kind == "INSERT" {
                RealtimeEvent::Insert { item_id }
            } else {
                RealtimeEvent::Update { item_id }
            }))
        }
        "DELETE" => {
            let item_id = data
                .old_record
                .or(data.old)
                .and_then(|old| old.item_id)
                .ok_or(RealtimeError::Protocol("delete payload carries no item id"))?;
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

    fn change(payload: &str) -> Result<Option<RealtimeEvent>, RealtimeError> {
        match dispatch(&format!(
            r#"["1","1","realtime:clipboard_items","postgres_changes",{payload}]"#
        )) {
            Dispatch::Event(event) => Ok(Some(*event)),
            Dispatch::Nothing => Ok(None),
            Dispatch::Failed(error) => Err(error),
            Dispatch::Closed => panic!("a change closed the channel"),
        }
    }

    #[test]
    fn a_change_carries_only_the_id_of_a_large_record() {
        let record = format!(
            r#"{{"item_id":"item-1","ciphertext":"{}","nonce":"bm9uY2U=","created_at":1}}"#,
            "A".repeat(1024 * 1024)
        );
        let insert = change(&format!(
            r#"{{"data":{{"type":"INSERT","table":"clipboard_items","record":{record}}}}}"#
        ));
        assert_eq!(
            insert,
            Ok(Some(RealtimeEvent::Insert {
                item_id: "item-1".into()
            }))
        );
        assert!(std::mem::size_of::<RealtimeEvent>() <= 4 * std::mem::size_of::<usize>());
    }

    #[test]
    fn an_update_carrying_a_tombstone_becomes_an_update_event() {
        // A delete is a tombstone — an UPDATE with `deleted = true`. This is the
        // path a real delete travels; the DELETE event type is the exception.
        let update = change(&format!(
            r#"{{"data":{{"type":"UPDATE","table":"clipboard_items","new":{}}}}}"#,
            row_json("item-2", true)
        ));
        assert_eq!(
            update,
            Ok(Some(RealtimeEvent::Update {
                item_id: "item-2".into()
            }))
        );
    }

    #[test]
    fn the_new_and_old_spellings_are_both_accepted() {
        assert!(matches!(
            change(&format!(
                r#"{{"data":{{"type":"insert","new":{}}}}}"#,
                row_json("item-3", false)
            )),
            Ok(Some(RealtimeEvent::Insert { .. }))
        ));
        assert_eq!(
            change(r#"{"data":{"type":"delete","old":{"item_id":"item-4"}}}"#),
            Ok(Some(RealtimeEvent::Delete {
                item_id: "item-4".into()
            }))
        );
    }

    #[test]
    fn a_delete_without_an_item_id_is_a_protocol_error_not_a_guess() {
        // Guessing an id here would delete the wrong row. The correct response
        // is to report and let the poll loop reconcile.
        assert!(change(r#"{"data":{"type":"DELETE","old_record":{"id":"row-pk"}}}"#).is_err());
    }

    #[test]
    fn a_change_to_another_table_is_ignored_not_an_error() {
        assert_eq!(
            change(r#"{"data":{"type":"INSERT","table":"other","record":{"item_id":"x"}}}"#),
            Ok(None)
        );
        assert_eq!(
            change(r#"{"data":{"type":"UPDATE","table":"device_group_states","record":{}}}"#),
            Ok(Some(RealtimeEvent::GroupChanged))
        );
    }

    #[test]
    fn a_record_without_an_id_is_an_error_not_a_guess() {
        assert!(change(r#"{"data":{"type":"INSERT","record":{"ciphertext":"x"}}}"#).is_err());
        assert!(change(r#"{"data":{"type":"INSERT"}}"#).is_err());
        assert!(change(r#"{"nodata":{}}"#).is_err());
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
            Dispatch::Event(event) if matches!(*event, RealtimeEvent::Insert { .. })
        ));
    }
}
