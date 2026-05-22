//! Phoenix Channel protocol types for Supabase Realtime.
//!
//! Wire format: `[join_ref, ref, topic, event, payload]` as a JSON array.
//!
//! Reference: <https://hexdocs.pm/phoenix/Phoenix.Channel.html>

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A Phoenix Channel message in its serialised form.
///
/// Serialised as a 5-element JSON array:
/// `[join_ref, ref, topic, event, payload]`
#[derive(Debug, Clone, PartialEq)]
pub struct PhoenixMessage {
    /// Reference for the join operation (null for server-pushed messages).
    pub join_ref: Option<String>,
    /// Per-message reference used to correlate replies.
    pub msg_ref: Option<String>,
    /// Channel topic, e.g. `"realtime:clipboard_items"`.
    pub topic: String,
    /// Phoenix event name (e.g. `"phx_join"`, `"heartbeat"`).
    pub event: String,
    /// Arbitrary JSON payload.
    pub payload: Value,
}

impl PhoenixMessage {
    /// Serialise to the Phoenix wire format (5-element JSON array).
    pub fn to_wire(&self) -> Result<String, serde_json::Error> {
        let arr: (Option<&str>, Option<&str>, &str, &str, &Value) = (
            self.join_ref.as_deref(),
            self.msg_ref.as_deref(),
            &self.topic,
            &self.event,
            &self.payload,
        );
        serde_json::to_string(&arr)
    }

    /// Parse a Phoenix wire-format string into a [`PhoenixMessage`].
    pub fn from_wire(s: &str) -> Result<Self, serde_json::Error> {
        let raw: (Value, Value, String, String, Value) = serde_json::from_str(s)?;
        Ok(Self {
            join_ref: match &raw.0 {
                Value::Null => None,
                v => Some(v.as_str().unwrap_or("").to_owned()),
            },
            msg_ref: match &raw.1 {
                Value::Null => None,
                v => Some(v.as_str().unwrap_or("").to_owned()),
            },
            topic: raw.2,
            event: raw.3,
            payload: raw.4,
        })
    }

    /// Build a `phx_join` message for a given topic.
    pub fn join(join_ref: &str, msg_ref: &str, topic: &str) -> Self {
        Self {
            join_ref: Some(join_ref.to_owned()),
            msg_ref: Some(msg_ref.to_owned()),
            topic: topic.to_owned(),
            event: PhoenixEvent::JOIN.to_owned(),
            payload: serde_json::json!({}),
        }
    }

    /// Build a heartbeat message for the `"phoenix"` system topic.
    pub fn heartbeat(msg_ref: &str) -> Self {
        Self {
            join_ref: None,
            msg_ref: Some(msg_ref.to_owned()),
            topic: "phoenix".to_owned(),
            event: PhoenixEvent::HEARTBEAT.to_owned(),
            payload: serde_json::json!({}),
        }
    }
}

/// Well-known Phoenix event name constants.
pub struct PhoenixEvent;

impl PhoenixEvent {
    pub const JOIN: &'static str = "phx_join";
    pub const REPLY: &'static str = "phx_reply";
    pub const ERROR: &'static str = "phx_error";
    pub const CLOSE: &'static str = "phx_close";
    pub const HEARTBEAT: &'static str = "heartbeat";
    /// Postgres INSERT/UPDATE/DELETE row events from Supabase Realtime.
    pub const POSTGRES_CHANGES: &'static str = "postgres_changes";
}

/// The type of database change event received from Supabase Realtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ChangeType {
    Insert,
    Update,
    Delete,
}

/// A parsed Supabase Realtime change event for the `clipboard_items` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    /// The kind of database operation.
    pub change_type: ChangeType,
    /// The affected table name.
    pub table: String,
    /// The raw record data (new row for INSERT/UPDATE, old row for DELETE).
    pub record: Value,
    /// Old record values (populated for UPDATE/DELETE when `old_record` config is enabled).
    pub old_record: Option<Value>,
    /// The original Phoenix message topic.
    pub topic: String,
}

impl ChangeEvent {
    /// Try to extract a [`ChangeEvent`] from the payload of a `postgres_changes` message.
    pub fn from_payload(topic: &str, payload: &Value) -> Option<Self> {
        // Supabase Realtime wraps the event under payload.data
        let data = payload.get("data").unwrap_or(payload);

        let change_type_str = data.get("type")?.as_str()?;
        let change_type: ChangeType = serde_json::from_value(
            Value::String(change_type_str.to_uppercase())
        ).ok()?;

        let table = data.get("table")
            .and_then(|v| v.as_str())
            .unwrap_or("clipboard_items")
            .to_owned();

        let record = data.get("record")
            .or_else(|| data.get("new"))
            .cloned()
            .unwrap_or(Value::Null);

        let old_record = data.get("old_record")
            .or_else(|| data.get("old"))
            .cloned();

        Some(Self {
            change_type,
            table,
            record,
            old_record,
            topic: topic.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PhoenixMessage serialisation ──────────────────────────────────────────

    #[test]
    fn heartbeat_round_trips() {
        let msg = PhoenixMessage::heartbeat("42");
        let wire = msg.to_wire().expect("serialise");
        let parsed = PhoenixMessage::from_wire(&wire).expect("deserialise");

        assert_eq!(parsed.topic, "phoenix");
        assert_eq!(parsed.event, PhoenixEvent::HEARTBEAT);
        assert_eq!(parsed.msg_ref.as_deref(), Some("42"));
        assert_eq!(parsed.join_ref, None);
    }

    #[test]
    fn join_message_round_trips() {
        let msg = PhoenixMessage::join("1", "2", "realtime:clipboard_items");
        let wire = msg.to_wire().expect("serialise");
        let parsed = PhoenixMessage::from_wire(&wire).expect("deserialise");

        assert_eq!(parsed.event, PhoenixEvent::JOIN);
        assert_eq!(parsed.topic, "realtime:clipboard_items");
        assert_eq!(parsed.join_ref.as_deref(), Some("1"));
        assert_eq!(parsed.msg_ref.as_deref(), Some("2"));
    }

    #[test]
    fn wire_format_is_5_element_array() {
        let msg = PhoenixMessage::heartbeat("1");
        let wire = msg.to_wire().expect("serialise");
        let arr: Vec<Value> = serde_json::from_str(&wire).expect("parse as array");
        assert_eq!(arr.len(), 5, "Phoenix wire format must be a 5-element array");
    }

    #[test]
    fn null_join_ref_survives_round_trip() {
        let raw = r#"[null,"1","phoenix","heartbeat",{}]"#;
        let msg = PhoenixMessage::from_wire(raw).expect("parse");
        assert_eq!(msg.join_ref, None);
        assert_eq!(msg.msg_ref.as_deref(), Some("1"));
    }

    #[test]
    fn from_wire_rejects_wrong_array_length() {
        // Only 4 elements — should fail
        let bad = r#"[null,"1","phoenix","heartbeat"]"#;
        assert!(PhoenixMessage::from_wire(bad).is_err());
    }

    // ── ChangeEvent extraction ────────────────────────────────────────────────

    #[test]
    fn change_event_insert_from_payload() {
        let payload = serde_json::json!({
            "data": {
                "type": "INSERT",
                "table": "clipboard_items",
                "record": { "id": "abc", "content_type": "text" },
                "old_record": null
            }
        });
        let ev = ChangeEvent::from_payload("realtime:clipboard_items", &payload)
            .expect("should parse");

        assert_eq!(ev.change_type, ChangeType::Insert);
        assert_eq!(ev.table, "clipboard_items");
        assert_eq!(ev.record["id"], "abc");
        assert_eq!(ev.topic, "realtime:clipboard_items");
    }

    #[test]
    fn change_event_delete_has_old_record() {
        let payload = serde_json::json!({
            "data": {
                "type": "DELETE",
                "table": "clipboard_items",
                "record": {},
                "old_record": { "id": "xyz" }
            }
        });
        let ev = ChangeEvent::from_payload("realtime:clipboard_items", &payload)
            .expect("should parse");

        assert_eq!(ev.change_type, ChangeType::Delete);
        let old = ev.old_record.expect("old_record present");
        assert_eq!(old["id"], "xyz");
    }

    #[test]
    fn change_event_returns_none_for_missing_type() {
        let payload = serde_json::json!({ "data": { "table": "clipboard_items" } });
        assert!(ChangeEvent::from_payload("realtime:clipboard_items", &payload).is_none());
    }

    #[test]
    fn change_event_accepts_lowercase_type() {
        let payload = serde_json::json!({
            "data": {
                "type": "update",
                "table": "clipboard_items",
                "record": { "id": "r1" }
            }
        });
        let ev = ChangeEvent::from_payload("realtime:clipboard_items", &payload)
            .expect("lowercase type should be accepted");
        assert_eq!(ev.change_type, ChangeType::Update);
    }
}
