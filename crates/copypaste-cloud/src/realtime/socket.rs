//! One session on one socket, and the schedule that starts the next one.
//!
//! The reconnect schedule lives beside the loop that consumes it because the
//! reset rule is a property of the *session*, not of the schedule: only the
//! code that knows how long a session lasted can decide it was stable.

use std::time::{Duration, Instant};

use backon::{BackoffBuilder, ExponentialBuilder};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;
use url::Url;

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

/// Run sessions back to back, reconnecting on the `backon` schedule.
///
/// The reset rule is manifest 05 §4.7's: a session that ran at least as long as
/// the ceiling counts as *stable*, so the schedule goes back to its floor.
/// Without that, a healthy server that blips once an hour accumulates an
/// ever-growing delay and eventually stops reconnecting promptly at all. A
/// short-lived session does **not** reset, which is what stops a connect-crash
/// loop from hammering the endpoint.
///
/// `token` is a watch channel rather than a `String` because this task outlives
/// any one JWT: a daemon that stays signed in for days refreshes its session
/// hourly, and a reconnect that re-joined with the token captured at
/// subscription time would be refused from the first refresh onward — silently,
/// since a refused join only slows the poll loop down.
#[allow(clippy::too_many_arguments)]
pub(super) async fn run(
    first: WsStream,
    endpoint: Url,
    anon_key: String,
    mut token: watch::Receiver<String>,
    user_id: String,
    tx: mpsc::Sender<Result<RealtimeEvent, RealtimeError>>,
    shutdown: CancellationToken,
) {
    let mut schedule = reconnect_policy().build();
    let mut stream = Some(first);

    loop {
        let started = Instant::now();
        let exit = match stream.take() {
            Some(ws) => pump(ws, &tx, &shutdown, &mut token).await,
            None => Exit::Disconnected,
        };
        if matches!(exit, Exit::Shutdown) || tx.is_closed() {
            return;
        }

        if started.elapsed() >= RECONNECT_MAX {
            schedule = reconnect_policy().build();
        }

        let Some(delay) = schedule.next() else {
            // The schedule has no attempt cap, so this is unreachable; treat it
            // as "give up" rather than as a reason to spin.
            let _ = tx
                .send(Err(RealtimeError::Connect("reconnects exhausted")))
                .await;
            return;
        };

        if wait_to_reconnect(delay, &shutdown).await {
            return;
        }

        // Read the token at each attempt, not once at startup.
        let current = token.borrow_and_update().clone();
        match open_channel(endpoint.clone(), &anon_key, &current, &user_id).await {
            Ok(ws) => {
                stream = Some(ws);
                // The gap is not replayed. Say so, so the subscriber polls
                // rather than trusting the channel to have carried everything
                // (manifest 05 §5.1 row 9a).
                if tx.send(Ok(RealtimeEvent::Resubscribed)).await.is_err() {
                    return;
                }
            }
            Err(RealtimeError::MissingUserId) => {
                // Cannot be recovered by retrying; the token is the problem.
                let _ = tx.send(Err(RealtimeError::MissingUserId)).await;
                return;
            }
            Err(e) => {
                // Reported as well as logged: a join that is being refused
                // because the JWT died is indistinguishable, from the outside,
                // from a quiet account. The subscriber is the only thing that
                // can refresh the session and hand the new token back.
                tracing::debug!(error = %e, "realtime reconnect attempt failed");
                if tx.send(Err(e)).await.is_err() {
                    return;
                }
            }
        }
    }
}

/// One session on one socket: dispatch inbound frames, send heartbeats, and
/// return when the socket dies or the caller asks us to stop.
async fn pump(
    ws: WsStream,
    tx: &mpsc::Sender<Result<RealtimeEvent, RealtimeError>>,
    shutdown: &CancellationToken,
    token: &mut watch::Receiver<String>,
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

            () = shutdown.cancelled() => {
                let _ = leave(&mut write, next_ref).await;
                return Exit::Shutdown;
            }

            // A refreshed session, pushed down without dropping the socket.
            // Supabase closes a channel whose JWT has expired, so a connection
            // that has been open for hours needs the new token *before* the old
            // one dies; re-joining afterwards would lose every event in between.
            changed = token.changed() => {
                if changed.is_err() {
                    // The subscription handle is gone.
                    return Exit::Shutdown;
                }
                let frame = access_token_frame(next_ref, &token.borrow_and_update());
                next_ref += 1;
                match tokio::time::timeout(
                    HEARTBEAT_INTERVAL,
                    write.send(Message::Text(frame.to_string())),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(_)) | Err(_) => {
                        tracing::debug!("could not push a refreshed token; reconnecting");
                        return Exit::Disconnected;
                    }
                }
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

async fn wait_to_reconnect(delay: Duration, shutdown: &CancellationToken) -> bool {
    tokio::select! {
        biased;
        () = shutdown.cancelled() => true,
        () = tokio::time::sleep(delay) => false,
    }
}

/// The frame that re-authenticates a channel that is already joined.
///
/// Phoenix's `access_token` event, which is how Supabase is told the JWT has
/// been rotated. Without it the server closes the channel when the old token
/// expires, and a subscription that has been up for an hour goes quiet.
fn access_token_frame(msg_ref: u64, access_token: &str) -> Value {
    json!([
        Value::Null,
        msg_ref.to_string(),
        TOPIC,
        "access_token",
        { "access_token": access_token }
    ])
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
/// There is no attempt cap: the subscription retries for as long as it is
/// alive, because a clipboard that stops syncing after fifteen minutes offline
/// would be worse than useless. Jitter is on, which is what stops every device
/// of a large account reconnecting in lockstep after a server restart.
fn reconnect_policy() -> ExponentialBuilder {
    ExponentialBuilder::new()
        .with_min_delay(RECONNECT_INITIAL)
        .with_max_delay(RECONNECT_MAX)
        .without_max_times()
        .with_jitter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, connect_async};

    #[test]
    fn the_reauth_frame_is_a_phoenix_event_on_our_topic() {
        let frame = access_token_frame(7, "the.new.jwt");
        let parts = frame.as_array().expect("five-element envelope");
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[1], "7");
        assert_eq!(parts[2], TOPIC);
        assert_eq!(parts[3], "access_token");
        assert_eq!(parts[4]["access_token"], "the.new.jwt");
    }

    #[test]
    fn the_reconnect_schedule_grows_and_is_bounded() {
        let mut schedule = reconnect_policy().build();
        let mut last = Duration::ZERO;
        let mut grew = false;

        for _ in 0..40 {
            let delay = schedule.next().expect("the schedule never gives up");
            // Jitter adds up to the delay again, so assert the envelope rather
            // than an exact doubling.
            assert!(
                delay <= RECONNECT_MAX * 2,
                "delay {delay:?} exceeded the ceiling"
            );
            if delay > last {
                grew = true;
            }
            last = delay;
        }
        assert!(grew, "the schedule never grew");

        // A stable session rebuilds it, which returns it to the floor.
        let first = reconnect_policy().build().next().unwrap();
        assert!(
            first <= RECONNECT_INITIAL * 2,
            "a rebuilt schedule did not start at the floor: {first:?}"
        );
    }

    #[test]
    fn the_reconnect_schedule_never_expires() {
        // No attempt cap. A device that has been offline overnight must still
        // reconnect in the morning.
        let mut schedule = reconnect_policy().build();
        assert!(
            schedule.nth(10_000).is_some(),
            "the reconnect schedule gave up"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_interrupts_reconnect_backoff() {
        let shutdown = CancellationToken::new();
        let waiting = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { wait_to_reconnect(RECONNECT_MAX, &shutdown).await }
        });
        tokio::task::yield_now().await;

        shutdown.cancel();
        tokio::task::yield_now().await;

        assert!(waiting.is_finished(), "cancellation left backoff running");
        assert!(waiting.await.expect("backoff task panicked"));
    }

    #[tokio::test]
    async fn cancellation_stops_an_active_pump_after_leaving() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(tcp).await.unwrap();
            ws.send(Message::Text("not json".into())).await.unwrap();

            let leave = ws.next().await.unwrap().unwrap();
            let Message::Text(leave) = leave else {
                panic!("close arrived before phx_leave");
            };
            let frame: Value = serde_json::from_str(&leave).unwrap();
            assert_eq!(frame[3], "phx_leave");
            assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
        });
        let (client, _) = connect_async(format!("ws://{address}")).await.unwrap();
        let (tx, mut events) = mpsc::channel(1);
        let (token_tx, mut token_rx) = watch::channel(String::from("token"));
        let shutdown = CancellationToken::new();
        let pumping = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { pump(client, &tx, &shutdown, &mut token_rx).await }
        });

        assert!(events.recv().await.unwrap().is_err());
        shutdown.cancel();

        assert!(matches!(pumping.await.unwrap(), Exit::Shutdown));
        server.await.unwrap();
        drop(token_tx);
    }
}
