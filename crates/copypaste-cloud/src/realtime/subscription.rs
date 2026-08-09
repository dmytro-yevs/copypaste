//! The handle a caller holds, and the lifetime of the task behind it.

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::channel::{jwt_subject, open_channel, websocket_url};
use super::event::{RealtimeError, RealtimeEvent};
use super::socket::run;
use crate::CloudConfig;

/// Bounded queue between the socket task and [`RealtimeSubscription::next_event`].
///
/// Bounded on purpose. If the consumer stalls, backpressure reaches the socket
/// and the server stops being told we are healthy, which is the correct signal;
/// an unbounded queue would instead grow without limit holding decrypted-row
/// metadata in memory. Missing an event is safe — the poll loop is the backstop.
const EVENT_QUEUE: usize = 64;

/// A live subscription to `clipboard_items`.
///
/// Owns a background task that holds the socket, sends heartbeats and
/// reconnects. Drop it or call [`RealtimeSubscription::close`] to stop; `close`
/// is preferable because it sends `phx_leave` first, so the server tears the
/// channel down immediately instead of waiting for a heartbeat timeout.
pub struct RealtimeSubscription {
    events: mpsc::Receiver<Result<RealtimeEvent, RealtimeError>>,
    token: watch::Sender<String>,
    // Cancellation must be sticky because close can precede the task's first wait.
    shutdown: CancellationToken,
    task: JoinHandle<()>,
}

impl std::fmt::Debug for RealtimeSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealtimeSubscription")
            .finish_non_exhaustive()
    }
}

impl RealtimeSubscription {
    /// Open the socket, join the channel, and start delivering events.
    ///
    /// The first connect and join happen before this returns, so a bad URL, a
    /// rejected token or a missing subject claim surface here rather than as a
    /// silent no-op. Everything after that — disconnects, reconnects,
    /// heartbeats — is handled by the background task.
    ///
    /// `access_token` is the current user JWT, and it is the *initial* value of
    /// something the caller must keep current: every session refresh has to
    /// reach this subscription through
    /// [`RealtimeSubscription::set_access_token`]. A JWT captured once and used
    /// forever expires on a connection that stays open for hours, and re-joins
    /// with a dead token after that (manifest 05 §4.7, non-negotiable 1).
    ///
    /// # Errors
    ///
    /// [`RealtimeError::MissingUserId`] if the token has no `sub` claim.
    ///
    /// [`RealtimeError::Connect`] if the socket cannot be opened.
    ///
    /// [`RealtimeError::JoinRefused`] if the channel join is not confirmed
    /// within [`JOIN_TIMEOUT`](super::JOIN_TIMEOUT).
    pub async fn connect(config: &CloudConfig, access_token: &str) -> Result<Self, RealtimeError> {
        let user_id = jwt_subject(access_token).ok_or(RealtimeError::MissingUserId)?;
        let url = websocket_url(&config.url);
        let anon_key = config.anon_key.clone();
        let token = access_token.to_owned();

        let stream = open_channel(&url, &anon_key, &token, &user_id).await?;

        let (tx, events) = mpsc::channel(EVENT_QUEUE);
        let (token_tx, token_rx) = watch::channel(token);
        let shutdown = CancellationToken::new();
        let task = tokio::spawn(run(
            stream,
            url,
            anon_key,
            token_rx,
            user_id,
            tx,
            shutdown.clone(),
        ));

        Ok(Self {
            events,
            token: token_tx,
            shutdown,
            task,
        })
    }

    /// Hand the socket a refreshed JWT.
    ///
    /// Call this on every session refresh. It pushes an `access_token` frame
    /// down the live channel — Supabase closes a channel whose token has
    /// expired, so a long-lived subscription that never re-authenticates dies
    /// quietly an hour in — and it is what the next reconnect will join with.
    ///
    /// Cheap and idempotent: a token identical to the current one still counts
    /// as a change, which is harmless.
    pub fn set_access_token(&self, access_token: &str) {
        // An error means the socket task has stopped, in which case the
        // subscription is finished and the token no longer matters.
        let _ = self.token.send(access_token.to_owned());
    }

    /// The next event, or `None` once the subscription has stopped for good.
    ///
    /// An `Err` here is informational: the task keeps running and reconnecting.
    /// `None` is terminal.
    pub async fn next_event(&mut self) -> Option<Result<RealtimeEvent, RealtimeError>> {
        self.events.recv().await
    }

    /// Leave the channel and stop the background task.
    ///
    /// Waits for the task to finish so that the `phx_leave` and the close frame
    /// are actually sent before the caller moves on.
    pub async fn close(self) {
        self.shutdown.cancel();
        // The task also observes the dropped receiver, so it cannot deadlock on
        // a full queue while shutting down.
        drop(self.events);
        let _ = self.task.await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::oneshot;

    use super::*;

    #[tokio::test]
    async fn close_before_the_task_waits_is_not_lost() {
        let (tx, events) = mpsc::channel(1);
        let (token, token_rx) = watch::channel(String::from("token"));
        let shutdown = CancellationToken::new();
        let observer = shutdown.clone();
        let (start_tx, start_rx) = oneshot::channel();
        let task = tokio::spawn({
            let shutdown = shutdown.clone();
            async move {
                start_rx.await.unwrap();
                shutdown.cancelled().await;
            }
        });
        let subscription = RealtimeSubscription {
            events,
            token,
            shutdown,
            task,
        };

        let closing = tokio::spawn(subscription.close());
        while !observer.is_cancelled() {
            tokio::task::yield_now().await;
        }
        start_tx.send(()).unwrap();

        tokio::time::timeout(Duration::from_secs(1), closing)
            .await
            .expect("close did not wake a later waiter")
            .expect("close task panicked");
        drop((tx, token_rx));
    }
}
