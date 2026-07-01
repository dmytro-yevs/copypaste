//! Per-frame WebSocket dispatch: `handle_message` (frame decode) and
//! `dispatch_event` (Phoenix event routing, with redacted logging).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{ChangeEvent, PhoenixEvent, PhoenixMessage};
use crate::realtime::redact_payload;

use super::reconnect::SessionResult;

/// Process a single WebSocket frame.
///
/// Returns `Some(SessionResult)` to terminate the session loop, or `None` to continue.
pub(super) async fn handle_message(
    msg: Message,
    tx: &mpsc::Sender<ChangeEvent>,
    topic: &str,
    channel_joined: &Arc<Notify>,
) -> Option<SessionResult> {
    match msg {
        Message::Text(text) => {
            match PhoenixMessage::from_wire(&text) {
                Err(e) => {
                    // Wave 2.7 sec #17: raw frame can embed clipboard plaintext.
                    // Log length + 16-byte prefix only, never the full text.
                    let bytes = text.as_bytes();
                    let take = bytes.len().min(16);
                    let prefix =
                        bytes[..take]
                            .iter()
                            .fold(String::with_capacity(take * 2), |mut acc, b| {
                                use std::fmt::Write as _;
                                let _ = write!(acc, "{:02x}", b);
                                acc
                            });
                    tracing::warn!(
                        error = %e,
                        raw_len = bytes.len(),
                        raw_prefix = %prefix,
                        "failed to parse Phoenix message"
                    );
                }
                Ok(phoenix_msg) => {
                    dispatch_event(&phoenix_msg, tx, topic, channel_joined).await;
                }
            }
            None
        }
        Message::Binary(data) => {
            tracing::debug!(bytes = data.len(), "received binary frame (ignored)");
            None
        }
        Message::Ping(data) => {
            // tungstenite auto-replies to Ping; we just log.
            tracing::trace!(bytes = data.len(), "received Ping");
            None
        }
        Message::Pong(_) => None,
        Message::Close(_) => {
            tracing::info!("received WebSocket Close frame");
            // Duration::ZERO is a placeholder; run_session replaces it with the
            // actual elapsed time before returning to connection_loop.
            Some(SessionResult::Disconnected(Duration::ZERO))
        }
        Message::Frame(_) => None,
    }
}

/// Route a parsed Phoenix message to the appropriate handler.
///
/// The `channel_joined` notify is fired when a `phx_reply` with
/// `status == "ok"` is observed — indicating the Phoenix Channel join has been
/// confirmed by the server.  The daemon's `ws_ingest_loop` awaits this signal
/// before setting `ws_connected = true` so the HTTP catch-up poll does not back
/// off to the slow rate until the channel is actually delivering events.
async fn dispatch_event(
    msg: &PhoenixMessage,
    tx: &mpsc::Sender<ChangeEvent>,
    topic: &str,
    channel_joined: &Arc<Notify>,
) {
    match msg.event.as_str() {
        PhoenixEvent::REPLY => {
            let status = msg.payload.get("status").and_then(|s| s.as_str());
            if status == Some("ok") {
                tracing::info!(topic = %msg.topic, "Phoenix Channel join confirmed (phx_reply ok)");
                // Signal the daemon that the channel subscription is live.
                // `notify_one` stores a permit so the next call to
                // `channel_joined.notified().await` completes immediately even
                // if the waiter hasn't registered yet (i.e. the phx_reply
                // arrives before ws_ingest_loop reaches its select! branch).
                // `notify_waiters` would only wake *current* waiters and the
                // permit would be lost if no one was waiting at that instant.
                channel_joined.notify_one();
            } else {
                tracing::warn!(topic = %msg.topic, ?status, "Phoenix reply with non-ok status");
            }
        }

        PhoenixEvent::ERROR => {
            tracing::error!(
                topic = %msg.topic,
                payload_redacted = %redact_payload(&msg.payload),
                "Phoenix channel error"
            );
        }

        PhoenixEvent::CLOSE => {
            tracing::info!(topic = %msg.topic, "Phoenix channel closed by server");
        }

        PhoenixEvent::POSTGRES_CHANGES => {
            if let Some(event) = ChangeEvent::from_payload(topic, &msg.payload) {
                tracing::debug!(
                    change_type = ?event.change_type,
                    table = %event.table,
                    "Supabase change event received"
                );
                if tx.send(event).await.is_err() {
                    tracing::debug!("change event receiver dropped; ignoring event");
                }
            } else {
                tracing::warn!(
                    payload_redacted = %redact_payload(&msg.payload),
                    "could not parse postgres_changes payload"
                );
            }
        }

        other => {
            tracing::trace!(event = %other, topic = %msg.topic, "unhandled Phoenix event");
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ChangeType;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn dispatch_postgres_changes_sends_to_channel() {
        let (tx, mut rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: None,
            msg_ref: None,
            topic: topic.to_owned(),
            event: PhoenixEvent::POSTGRES_CHANGES.to_owned(),
            payload: serde_json::json!({
                "data": {
                    "type": "INSERT",
                    "table": "clipboard_items",
                    "record": { "id": "item-1", "content_type": "text" },
                }
            }),
        };

        dispatch_event(&msg, &tx, topic, &joined).await;

        let event = rx.try_recv().expect("event should be in channel");
        assert_eq!(event.change_type, ChangeType::Insert);
        assert_eq!(event.record["id"], "item-1");
    }

    #[tokio::test]
    async fn dispatch_phx_reply_ok_does_not_send_event() {
        let (tx, mut rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: Some("1".to_owned()),
            msg_ref: Some("1".to_owned()),
            topic: topic.to_owned(),
            event: PhoenixEvent::REPLY.to_owned(),
            payload: serde_json::json!({ "status": "ok", "response": {} }),
        };

        dispatch_event(&msg, &tx, topic, &joined).await;
        assert!(
            rx.try_recv().is_err(),
            "phx_reply should not produce a ChangeEvent"
        );
    }

    #[tokio::test]
    async fn dispatch_unknown_event_does_not_send_event() {
        let (tx, mut rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: None,
            msg_ref: None,
            topic: topic.to_owned(),
            event: "presence_state".to_owned(),
            payload: serde_json::json!({}),
        };

        dispatch_event(&msg, &tx, topic, &joined).await;
        assert!(rx.try_recv().is_err());
    }

    // ── channel_joined signal (Phase 3) ──────────────────────────────────────

    /// `dispatch_event` must fire the `channel_joined` notify when it sees
    /// `phx_reply` with `status == "ok"`.
    ///
    /// Contract: `ClientHandle::channel_joined()` must return an `Arc<Notify>`
    /// that is notified by `dispatch_event` so that `ws_ingest_loop` in the
    /// daemon can gate `ws_connected=true` on channel confirmation instead of
    /// bare socket-open.
    #[tokio::test]
    async fn dispatch_phx_reply_ok_fires_channel_joined_notify() {
        let (tx, _rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: Some("1".to_owned()),
            msg_ref: Some("1".to_owned()),
            topic: topic.to_owned(),
            event: PhoenixEvent::REPLY.to_owned(),
            payload: serde_json::json!({ "status": "ok", "response": {} }),
        };

        // Must not be notified before calling dispatch_event.
        let notified_before = tokio::time::timeout(
            std::time::Duration::from_millis(0),
            joined.clone().notified(),
        )
        .await
        .is_ok();
        assert!(
            !notified_before,
            "channel_joined must not fire before dispatch_event"
        );

        dispatch_event(&msg, &tx, topic, &joined).await;

        // Must be notified now (use a tight timeout to stay deterministic).
        let notified_after =
            tokio::time::timeout(std::time::Duration::from_millis(50), joined.notified())
                .await
                .is_ok();
        assert!(
            notified_after,
            "dispatch_event must fire channel_joined on phx_reply ok"
        );
    }

    /// A non-ok `phx_reply` must NOT fire the `channel_joined` notify.
    #[tokio::test]
    async fn dispatch_phx_reply_error_does_not_fire_channel_joined() {
        let (tx, _rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: Some("1".to_owned()),
            msg_ref: Some("1".to_owned()),
            topic: topic.to_owned(),
            event: PhoenixEvent::REPLY.to_owned(),
            payload: serde_json::json!({ "status": "error", "response": {} }),
        };

        dispatch_event(&msg, &tx, topic, &joined).await;

        let notified =
            tokio::time::timeout(std::time::Duration::from_millis(10), joined.notified())
                .await
                .is_ok();
        assert!(
            !notified,
            "dispatch_event must NOT fire channel_joined on non-ok phx_reply"
        );
    }
}
