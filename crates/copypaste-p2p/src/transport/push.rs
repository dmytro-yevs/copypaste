//! Bounded push helper and event-driven notifier for the P2P outbound loop.
//!
//! [`try_push_frame`] wraps a framed sink's `send` in a timeout so a stalled
//! peer cannot deadlock the outbound task (CopyPaste-yr00).
//!
//! [`PushNotifier`] lets the clipboard poller wake the outbound loop
//! immediately when a new item is ready, instead of waiting a full poll
//! interval (CopyPaste-mip2).

use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// CopyPaste-yr00: bounded push helper (backpressure on slow peers)
// CopyPaste-mip2: event-driven push notifier (reduce outbound latency)
// ---------------------------------------------------------------------------

/// Maximum time to wait for a single frame write to a peer's TCP/TLS
/// socket before giving up. Prevents `push_catchup` (or any caller that
/// calls [`try_push_frame`]) from blocking indefinitely on a slow or
/// stalled peer and deadlocking the outbound task.
///
/// Chosen to be long enough for a saturated LAN link to absorb a 16 MiB
/// frame (the maximum frame size, see `MAX_FRAME_BYTES`) without false
/// positive timeouts (~1 s at 100 Mbps), yet short enough to bound the
/// worst-case delay before the caller can move on or reconnect.
///
/// CopyPaste-yr00: without this timeout, `SinkExt::send().await` on a
/// `Framed<TlsStream>` will park indefinitely if the peer's TCP receive
/// window is full (the peer is not draining the socket). The connector
/// task — which calls `push_catchup` and shares the same task with `send`
/// calls — could deadlock, never making progress to reconnect or drain
/// newer items from other peers.
pub const PEER_SEND_TIMEOUT: Duration = Duration::from_secs(5);

/// Error returned by [`try_push_frame`] when the send times out or the
/// underlying socket is broken.
#[derive(Debug)]
pub enum PushError {
    /// The peer did not drain its receive buffer within [`PEER_SEND_TIMEOUT`].
    /// The caller should close and re-establish the connection.
    Timeout,
    /// The underlying framed sink returned an I/O error.
    Io(std::io::Error),
}

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushError::Timeout => write!(f, "peer send timed out after {:?}", PEER_SEND_TIMEOUT),
            PushError::Io(e) => write!(f, "peer send I/O error: {e}"),
        }
    }
}

/// Send `frame` to a peer stream with a [`PEER_SEND_TIMEOUT`] deadline.
///
/// # CopyPaste-yr00 — bounded push (backpressure)
///
/// The plain `SinkExt::send(frame).await` call used in the old `push_catchup`
/// implementation was unbounded: if the peer stopped draining its TCP receive
/// window (slow reader, busy GC, app suspend, …), the `send` future would park
/// forever. Because `push_catchup` was `await`-ed inside the connector task
/// loop, a single stalled peer could deadlock the entire outbound path — no
/// reconnects, no items flushed to any other peer.
///
/// This function wraps the send in `tokio::time::timeout` so the maximum block
/// time is bounded to [`PEER_SEND_TIMEOUT`]. On timeout the caller should treat
/// the peer as unresponsive and tear down the connection.
///
/// `frame` is any `Bytes`-compatible owned frame (the same payload that
/// `SinkExt::send` would take on `PeerStream` / `PeerClientStream`).
pub async fn try_push_frame<S>(sink: &mut S, frame: bytes::Bytes) -> Result<(), PushError>
where
    S: futures_util::SinkExt<bytes::Bytes, Error = std::io::Error> + Unpin,
{
    tokio::time::timeout(PEER_SEND_TIMEOUT, futures_util::SinkExt::send(sink, frame))
        .await
        .map_err(|_| PushError::Timeout)?
        .map_err(PushError::Io)
}

/// A lightweight, clone-able handle for waking the outbound sync loop from a
/// producer context (e.g. the daemon's clipboard poller) when a new item is
/// available.
///
/// # CopyPaste-mip2 — event-driven push (reduce outbound latency)
///
/// The old outbound loop polled for new items on a fixed ~30 s interval
/// (matching the relay poll interval), so a clipboard item on a LAN peer
/// could take up to 30 s to propagate. By contrast, direct P2P should be
/// near-instant.
///
/// `PushNotifier` wraps a `tokio::sync::Notify` so the daemon can call
/// [`notify`](Self::notify) immediately after writing a new clipboard item.
/// The outbound loop waits on [`wait`](Self::wait), which returns as soon as
/// `notify` is called rather than sleeping for a full interval. A ticker-based
/// fallback (call `notify` from a periodic task) retains the polling backstop.
///
/// # Usage pattern
///
/// ```no_run
/// use copypaste_p2p::PushNotifier;
///
/// let notifier = PushNotifier::new();
/// let waker = notifier.clone();
///
/// // Outbound loop (P2P sender task):
/// tokio::spawn(async move {
///     loop {
///         notifier.wait().await;          // parks here until notified
///         // ... flush pending items to connected peers ...
///     }
/// });
///
/// // Producer (clipboard poller or IPC handler):
/// // After writing a new clipboard item:
/// waker.notify();
/// ```
#[derive(Clone, Debug)]
pub struct PushNotifier(Arc<tokio::sync::Notify>);

impl PushNotifier {
    /// Create a fresh `PushNotifier`. The outbound loop receives one
    /// notification immediately on construction so it runs once at startup to
    /// flush any items buffered while disconnected.
    pub fn new() -> Self {
        let inner = Arc::new(tokio::sync::Notify::new());
        // Pre-notify so the first `wait()` call doesn't park before the loop
        // has a chance to do its initial sync.
        inner.notify_one();
        Self(inner)
    }

    /// Wake the outbound loop. Idempotent: multiple calls between loop
    /// iterations collapse to a single wakeup (tokio `Notify` semantics —
    /// up to one queued permit at a time).
    pub fn notify(&self) {
        self.0.notify_one();
    }

    /// Wait until [`notify`](Self::notify) is called, then return.
    /// If a notification is already queued, returns immediately.
    pub async fn wait(&self) {
        self.0.notified().await;
    }
}

impl Default for PushNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::SelfSignedCert;
    use crate::{PairedPeers, PeerTransport};

    // ── CopyPaste-yr00: try_push_frame bounded send ──────────────────────────

    /// CopyPaste-yr00: `PEER_SEND_TIMEOUT` must be defined and be at least 1 s.
    /// Fail-safe: if it's 0, the very first frame on a normal connection times out.
    #[test]
    fn peer_send_timeout_is_at_least_one_second() {
        assert!(
            PEER_SEND_TIMEOUT >= Duration::from_secs(1),
            "CopyPaste-yr00: PEER_SEND_TIMEOUT ({PEER_SEND_TIMEOUT:?}) must be at least 1 s \
             to allow normal frame delivery on a loaded LAN"
        );
    }

    /// CopyPaste-yr00: verify that `try_push_frame` wraps the send in a timeout
    /// by using a real `PeerStream`-compatible framed stream obtained from a
    /// loopback TLS handshake. The happy path (peer reads) must succeed.
    ///
    /// Note: the stalled-peer timeout case can't be tested in a unit test without
    /// freezing for PEER_SEND_TIMEOUT seconds; the const-is-set guard above and
    /// the integration of tokio::time::timeout in the implementation provide the
    /// correctness guarantee. A separate integration test with time-mocking is
    /// the appropriate venue for the negative case.
    #[tokio::test(flavor = "current_thread")]
    async fn try_push_frame_succeeds_on_loopback_connection() {
        let server_cert = SelfSignedCert::generate("server-yr00").unwrap();
        let client_cert = SelfSignedCert::generate("client-yr00").unwrap();

        let server_fp = server_cert.fingerprint();
        let client_fp = client_cert.fingerprint();

        let server_peers = PairedPeers::new();
        server_peers.add(client_fp.clone(), "client-yr00");
        let client_peers = PairedPeers::new();
        client_peers.add(server_fp.clone(), "server-yr00");

        let server_transport =
            PeerTransport::from_cert(server_cert.cert_der, server_cert.key_der, server_peers);
        let client_transport =
            PeerTransport::from_cert(client_cert.cert_der, client_cert.key_der, client_peers);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Server accepts while client connects — both complete the TLS handshake.
        // `accept` returns (SocketAddr, DeviceFingerprint, PeerStream);
        // `connect` returns PeerClientStream directly.
        let (server_result, client_result) = tokio::join!(
            server_transport.accept(&listener),
            client_transport.connect(addr, &server_fp),
        );
        let (_, _, mut server_stream) = server_result.unwrap();
        let mut client_stream = client_result.unwrap();

        let frame = bytes::Bytes::from_static(b"yr00-test-frame");
        // Send a frame from client to server using the bounded helper.
        try_push_frame(&mut client_stream, frame.clone())
            .await
            .expect("CopyPaste-yr00: try_push_frame to a reading peer must succeed");

        // Server must receive the frame.
        use futures_util::StreamExt as _;
        let received = server_stream.next().await.unwrap().unwrap();
        assert_eq!(received.as_ref(), frame.as_ref());
    }

    // ── CopyPaste-mip2: PushNotifier event-driven wakeup ────────────────────

    /// CopyPaste-mip2: a freshly created `PushNotifier` must be pre-notified so
    /// the outbound loop does NOT block on the first `wait()` call — it must
    /// immediately start flushing any items buffered while disconnected.
    #[tokio::test(flavor = "current_thread")]
    async fn push_notifier_is_pre_notified_on_construction() {
        let notifier = PushNotifier::new();
        // The first wait() must complete immediately (pre-notification).
        let result = tokio::time::timeout(Duration::from_millis(10), notifier.wait()).await;
        assert!(
            result.is_ok(),
            "CopyPaste-mip2: PushNotifier::new() must pre-notify so the first wait() is immediate"
        );
    }

    /// CopyPaste-mip2: calling `notify()` must wake a waiting outbound loop.
    #[tokio::test(flavor = "current_thread")]
    async fn push_notifier_wakes_waiting_loop() {
        let notifier = PushNotifier::new();
        // Drain the initial pre-notification.
        notifier.wait().await;

        let waker = notifier.clone();
        // The loop parks because there's no pending notification.
        let wait_fut = notifier.wait();

        // Notify from "producer" side, then await the wait.
        waker.notify();
        let result = tokio::time::timeout(Duration::from_millis(50), wait_fut).await;
        assert!(
            result.is_ok(),
            "CopyPaste-mip2: notify() must wake a parked wait() within 50 ms"
        );
    }

    /// CopyPaste-mip2: multiple `notify()` calls between `wait()` calls must
    /// collapse to a single wakeup (tokio `Notify` permits at most one queued
    /// token), preventing the outbound loop from spinning on a burst of updates.
    #[tokio::test(flavor = "current_thread")]
    async fn push_notifier_collapses_burst_to_single_wakeup() {
        let notifier = PushNotifier::new();
        notifier.wait().await; // drain pre-notification

        let waker = notifier.clone();
        // Fire three notifications in a row.
        waker.notify();
        waker.notify();
        waker.notify();

        // First wait() must complete immediately (one queued permit).
        tokio::time::timeout(Duration::from_millis(10), notifier.wait())
            .await
            .expect("first wait() must complete");

        // Second wait() must park — no additional permit was stored.
        let result = tokio::time::timeout(Duration::from_millis(10), notifier.wait()).await;
        assert!(
            result.is_err(),
            "CopyPaste-mip2: burst of notify() calls must collapse to one wakeup"
        );
    }
}
