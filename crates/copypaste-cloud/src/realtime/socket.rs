//! One session on one socket, and the schedule that starts the next one.
//!
//! The reconnect schedule lives beside the loop that consumes it because the
//! reset rule is a property of the *session*, not of the schedule: only the
//! code that knows how long a session lasted can decide it was stable.

use std::sync::Arc;
use std::time::{Duration, Instant};

use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Notify};
use tokio_tungstenite::tungstenite::Message;

use super::channel::{open_channel, WsStream};
use super::event::{RealtimeError, RealtimeEvent};
use super::frame::{dispatch, Dispatch};
use super::{JOIN_TIMEOUT, TOPIC};

/// Heartbeat cadence. The server drops a channel that has been silent for about
/// sixty seconds, so thirty gives one missed beat of slack.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Reconnect schedule floor.
const RECONNECT_INITIAL: Duration = Duration::from_secs(1);

/// Reconnect schedule ceiling.
const RECONNECT_MAX: Duration = Duration::from_secs(60);

/// The write half, after splitting. Reading and writing are split so that a
/// heartbeat and an inbound frame are not two mutable borrows of one socket
/// inside the same `select!`.
type WsSink = futures_util::stream::SplitSink<WsStream, Message>;

/// Outcome of one session on one socket.
enum Exit {
    /// The caller asked us to stop; the channel was left cleanly.
    Shutdown,
    /// The socket or the channel went away. Reconnect.
    Disconnected,
}

/// Run sessions back to back, reconnecting on the `backoff` schedule.
///
/// The reset rule is manifest 05 §4.7's: a session that ran at least as long as
/// the ceiling counts as *stable*, so the schedule goes back to its floor.
/// Without that, a healthy server that blips once an hour accumulates an
/// ever-growing delay and eventually stops reconnecting promptly at all. A
/// short-lived session does **not** reset, which is what stops a connect-crash
/// loop from hammering the endpoint.
#[allow(clippy::too_many_arguments)]
pub(super) async fn run(
    first: WsStream,
    url: String,
    anon_key: String,
    token: String,
    user_id: String,
    tx: mpsc::Sender<Result<RealtimeEvent, RealtimeError>>,
    shutdown: Arc<Notify>,
) {
    let mut policy = reconnect_policy();
    let mut stream = Some(first);

    loop {
        let started = Instant::now();
        let exit = match stream.take() {
            Some(ws) => pump(ws, &tx, &shutdown).await,
            None => Exit::Disconnected,
        };
        if matches!(exit, Exit::Shutdown) || tx.is_closed() {
            return;
        }

        if started.elapsed() >= RECONNECT_MAX {
            policy.reset();
        }

        let Some(delay) = policy.next_backoff() else {
            // `max_elapsed_time` is `None`, so this is unreachable; treat it as
            // "give up" rather than as a reason to spin.
            let _ = tx
                .send(Err(RealtimeError::Connect("reconnects exhausted")))
                .await;
            return;
        };

        tokio::select! {
            biased;
            () = shutdown.notified() => return,
            () = tokio::time::sleep(delay) => {}
        }

        match open_channel(&url, &anon_key, &token, &user_id).await {
            Ok(ws) => stream = Some(ws),
            Err(RealtimeError::MissingUserId) => {
                // Cannot be recovered by retrying; the token is the problem.
                let _ = tx.send(Err(RealtimeError::MissingUserId)).await;
                return;
            }
            Err(e) => {
                tracing::debug!(error = %e, "realtime reconnect attempt failed");
            }
        }
    }
}

/// One session on one socket: dispatch inbound frames, send heartbeats, and
/// return when the socket dies or the caller asks us to stop.
async fn pump(
    ws: WsStream,
    tx: &mpsc::Sender<Result<RealtimeEvent, RealtimeError>>,
    shutdown: &Notify,
) -> Exit {
    // Split so the heartbeat write and the inbound read are disjoint borrows.
    let (mut write, mut read) = ws.split();

    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    heartbeat.tick().await; // the first tick is immediate; the join just happened.

    // Ref 1 is the join (see `join_frame`), so the heartbeat counter starts at 2.
    let mut next_ref: u64 = 2;

    loop {
        tokio::select! {
            biased;

            () = shutdown.notified() => {
                let _ = leave(&mut write, next_ref).await;
                return Exit::Shutdown;
            }

            _ = heartbeat.tick() => {
                let frame = json!([Value::Null, next_ref.to_string(), "phoenix", "heartbeat", {}]);
                next_ref += 1;
                // Bound the write. On a half-open socket `send` can stall
                // indefinitely, starving heartbeats until the server-side
                // timeout kills the connection ~60 s later. A write that does
                // not complete within one heartbeat interval *is* a disconnect
                // (manifest 05 §4.7).
                match tokio::time::timeout(
                    HEARTBEAT_INTERVAL,
                    write.send(Message::Text(frame.to_string())),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(_)) | Err(_) => {
                        tracing::debug!("realtime heartbeat write failed or stalled; reconnecting");
                        return Exit::Disconnected;
                    }
                }
            }

            frame = read.next() => {
                let Some(frame) = frame else { return Exit::Disconnected };
                let text = match frame {
                    Ok(Message::Text(t)) => t,
                    // Binary frames are not part of the Phoenix protocol here.
                    Ok(Message::Binary(_)) => continue,
                    Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => continue,
                    Ok(Message::Close(_)) => return Exit::Disconnected,
                    Err(_) => return Exit::Disconnected,
                };

                match dispatch(&text) {
                    Dispatch::Nothing => {}
                    Dispatch::Closed => return Exit::Disconnected,
                    Dispatch::Event(event) => {
                        if tx.send(Ok(event)).await.is_err() {
                            return Exit::Shutdown;
                        }
                    }
                    Dispatch::Failed(e) => {
                        if tx.send(Err(e)).await.is_err() {
                            return Exit::Shutdown;
                        }
                    }
                }
            }
        }
    }
}

/// Send `phx_leave` and close. Best effort — we are shutting down either way.
async fn leave(write: &mut WsSink, msg_ref: u64) -> Result<(), ()> {
    let frame = json!(["1", msg_ref.to_string(), TOPIC, "phx_leave", {}]);
    let farewell = async {
        write
            .send(Message::Text(frame.to_string()))
            .await
            .map_err(|_| ())?;
        SinkExt::close(write).await.map_err(|_| ())
    };
    tokio::time::timeout(JOIN_TIMEOUT, farewell)
        .await
        .map_err(|_| ())?
}

/// Build the reconnect schedule.
///
/// `max_elapsed_time` is `None` so the subscription retries for as long as it
/// is alive; a clipboard that stops syncing after fifteen minutes offline would
/// be worse than useless. The randomisation factor is the crate's default,
/// which is what stops every device of a large account reconnecting in lockstep
/// after a server restart.
fn reconnect_policy() -> ExponentialBackoff {
    ExponentialBackoff {
        initial_interval: RECONNECT_INITIAL,
        max_interval: RECONNECT_MAX,
        max_elapsed_time: None,
        ..ExponentialBackoff::default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// The schedule is arithmetic; nothing here opens a socket.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reconnect_schedule_grows_and_is_bounded() {
        let mut policy = reconnect_policy();
        let mut last = Duration::ZERO;
        let mut grew = false;

        for _ in 0..40 {
            let delay = policy.next_backoff().expect("the schedule never gives up");
            // The crate applies a randomisation factor, so assert the envelope
            // rather than an exact doubling.
            assert!(
                delay <= RECONNECT_MAX.mul_f64(1.5),
                "delay {delay:?} exceeded the ceiling"
            );
            if delay > last {
                grew = true;
            }
            last = delay;
        }
        assert!(grew, "the schedule never grew");

        // A stable session resets it to the floor.
        policy.reset();
        let first = policy.next_backoff().unwrap();
        assert!(
            first <= RECONNECT_INITIAL.mul_f64(1.5),
            "reset did not return to the floor: {first:?}"
        );
    }

    #[test]
    fn the_reconnect_schedule_never_expires() {
        // `max_elapsed_time = None`. A device that has been offline overnight
        // must still reconnect in the morning.
        assert!(reconnect_policy().max_elapsed_time.is_none());
    }
}
