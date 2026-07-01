//! A single WebSocket session: connect → Phoenix join → heartbeat + receive
//! loop → leave.
//!
//! Invoked once per iteration of [`super::reconnect::connection_loop`].

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::sync::Notify;
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{ChangeEvent, PhoenixEvent, PhoenixMessage};
use crate::realtime::{build_rustls_connector, build_ws_request, RealtimeConfig};
use futures_util::{SinkExt, StreamExt};

use super::dispatch::handle_message;
use super::join::build_join_payload;
use super::reconnect::SessionResult;

/// Run a single WebSocket session: connect → join channel → heartbeat + receive loop.
pub(super) async fn run_session(
    config: &RealtimeConfig,
    tx: &mpsc::Sender<ChangeEvent>,
    shutdown: &Arc<Notify>,
    channel_joined: &Arc<Notify>,
) -> SessionResult {
    // CopyPaste-lnjm: build a proper HTTP upgrade request with the apikey
    // in a request header (not the URL query string).
    let request = match build_ws_request(&config.ws_url, &config.anon_key) {
        Ok(r) => r,
        Err(e) => return SessionResult::ConnectError(format!("request build: {e}")),
    };

    // CopyPaste-qkao: attach a custom TLS connector with SPKI pinning when
    // the URL is wss:// and pins are configured. For plain ws:// (loopback
    // dev) no connector is returned and we fall back to the plain path.
    let connector = build_rustls_connector(&config.ws_url, &config.spki_pins);

    // Establish the WebSocket connection.
    let ws_stream = match connect_async_tls_with_config(request, None, false, connector).await {
        Ok((ws, _)) => ws,
        Err(e) => return SessionResult::ConnectError(e.to_string()),
    };

    tracing::info!("WebSocket connected to Supabase Realtime");

    // Track how long this session runs so the caller can reset backoff when
    // the session was long enough to be considered "stable".
    let session_started = std::time::Instant::now();

    let (mut sink, mut stream) = ws_stream.split();

    // Fix HIGH #2: read the CURRENT bearer token for this reconnect so that a
    // refreshed JWT (pushed via `RealtimeClient::update_jwt`) is always used
    // rather than the stale value captured at client creation time.
    //
    // Fix MED #3: build_join_payload registers event:"*" (INSERT + UPDATE +
    // DELETE) instead of INSERT-only, so cross-device UPDATE/DELETE are delivered.
    //
    // CopyPaste-nr2y (defense-in-depth): a missing user_id is a hard error —
    // we must never silently subscribe without the row filter and rely solely on
    // server-side RLS. Fail the session here; the connection_loop will back off
    // and retry once the caller has populated `config.user_id`.
    let user_id = match config.user_id.as_deref() {
        Some(uid) => uid,
        None => {
            return SessionResult::ConnectError(
                "user_id is required for the Realtime row filter (CopyPaste-nr2y): \
                 set RealtimeConfig::user_id to the GoTrue user UUID before connecting"
                    .into(),
            )
        }
    };
    let current_jwt = config.user_jwt.read().await.clone();
    let join_payload = build_join_payload(&current_jwt, user_id);
    let join_msg = PhoenixMessage {
        join_ref: Some("1".to_owned()),
        msg_ref: Some("1".to_owned()),
        topic: config.topic.clone(),
        event: PhoenixEvent::JOIN.to_owned(),
        payload: join_payload,
    };
    let join_wire = match join_msg.to_wire() {
        Ok(w) => w,
        Err(e) => return SessionResult::ConnectError(format!("join serialise: {e}")),
    };

    if let Err(e) = sink.send(Message::Text(join_wire)).await {
        return SessionResult::ConnectError(format!("join send: {e}"));
    }

    tracing::info!(topic = %config.topic, "Phoenix Channel join sent");

    // Heartbeat task: sends heartbeat every `heartbeat_interval`.
    let heartbeat_interval = config.heartbeat_interval;
    let (hb_stop_tx, mut hb_stop_rx) = tokio::sync::oneshot::channel::<()>();

    // Channel to carry serialised heartbeat payloads from the heartbeat task to sink.
    let (hb_payload_tx, mut hb_payload_rx) = mpsc::channel::<String>(4);

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(heartbeat_interval);
        let mut ref_counter: u64 = 2; // 1 was used for join
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let msg_ref = ref_counter.to_string();
                    ref_counter += 1;
                    let msg = PhoenixMessage::heartbeat(&msg_ref);
                    match msg.to_wire() {
                        Ok(w) => {
                            if hb_payload_tx.send(w).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => tracing::warn!("heartbeat serialise error: {e}"),
                    }
                }
                _ = &mut hb_stop_rx => {
                    break;
                }
            }
        }
    });

    // Main receive + heartbeat forward loop.
    loop {
        tokio::select! {
            // Incoming WebSocket message.
            maybe_msg = stream.next() => {
                match maybe_msg {
                    None => {
                        // Stream ended.
                        let _ = hb_stop_tx.send(());
                        return SessionResult::Disconnected(session_started.elapsed());
                    }
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "WebSocket receive error");
                        let _ = hb_stop_tx.send(());
                        return SessionResult::Disconnected(session_started.elapsed());
                    }
                    Some(Ok(msg)) => {
                        if let Some(result) = handle_message(msg, tx, &config.topic, channel_joined).await {
                            let _ = hb_stop_tx.send(());
                            // For Disconnected results from handle_message, replace
                            // the placeholder duration with the actual session age.
                            return match result {
                                SessionResult::Disconnected(_) => {
                                    SessionResult::Disconnected(session_started.elapsed())
                                }
                                other => other,
                            };
                        }
                    }
                }
            }

            // Heartbeat payload ready to send.
            Some(payload) = hb_payload_rx.recv() => {
                tracing::debug!("sending heartbeat");
                // Bound the write: on a half-open socket `send` can stall
                // indefinitely, silently starving heartbeats until the ~60s
                // server timeout kills us. Treat a write that doesn't complete
                // within one heartbeat interval as a disconnect and reconnect.
                match tokio::time::timeout(
                    heartbeat_interval,
                    sink.send(Message::Text(payload)),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "heartbeat send failed");
                        let _ = hb_stop_tx.send(());
                        return SessionResult::Disconnected(session_started.elapsed());
                    }
                    Err(_) => {
                        tracing::warn!("heartbeat send timed out; treating as disconnect");
                        let _ = hb_stop_tx.send(());
                        return SessionResult::Disconnected(session_started.elapsed());
                    }
                }
            }

            // Shutdown signal.
            _ = shutdown.notified() => {
                // Send phx_leave before closing.
                let leave = PhoenixMessage {
                    join_ref: Some("1".to_owned()),
                    msg_ref: Some("leave".to_owned()),
                    topic: config.topic.clone(),
                    event: "phx_leave".to_owned(),
                    payload: serde_json::json!({}),
                };
                if let Ok(wire) = leave.to_wire() {
                    let _ = sink.send(Message::Text(wire)).await;
                }
                let _ = sink.send(Message::Close(None)).await;
                let _ = hb_stop_tx.send(());
                return SessionResult::Shutdown;
            }
        }
    }
}
