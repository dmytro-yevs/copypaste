//! A [`SyncChannel`] over the Noise session.
//!
//! Two things this adds to [`crate::transport::Session`], both of which the sync
//! engine explicitly leaves to the channel:
//!
//! * **A read deadline.** Either half can abort mid-session — a bound was
//!   exceeded, the store failed — and it does so without a goodbye, so the other
//!   end is left waiting for a message that will never arrive. A channel with no
//!   deadline turns that into a task that never finishes and a peer slot that is
//!   never freed.
//! * **`encode`/`decode` rather than serde on `SyncMessage` directly.** Those
//!   two functions are where the wire bounds and the timestamp clamp live, so
//!   the payload is moved as a raw JSON document and handed to them verbatim.
//!   Calling `Session::recv::<SyncMessage>()` would deserialise the same bytes
//!   while quietly skipping both — a second, weaker copy of the wire contract.

use std::time::Duration;

use crate::protocol::SyncMessage;
use crate::sync::{SyncChannel, SyncError};
use crate::transport::Session;
use serde_json::value::RawValue;

/// How long one half waits for the other's next message.
///
/// Generous: the peer may be decrypting a batch or writing to a busy database.
/// It only has to be short enough that a peer which has gone away is noticed
/// before the session slot matters, and the whole session has its own outer
/// timeout on top.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// The whole of one session, handshake excluded.
///
/// Bounds the case the read deadline cannot: a peer that keeps answering, on
/// time, forever.
pub const SESSION_TIMEOUT: Duration = Duration::from_secs(120);

pub struct NoiseChannel {
    session: Session,
    read_timeout: Duration,
}

impl NoiseChannel {
    pub fn new(session: Session) -> Self {
        Self {
            session,
            read_timeout: READ_TIMEOUT,
        }
    }

    /// Wait for the peer to hang up.
    ///
    /// The initiating half sends the last message of a session — its `Done` —
    /// and `run_initiator` returns the moment that is written. The responder
    /// still has to *apply* what it was just sent, and it closes the connection
    /// when it has. Waiting for that close is what makes `copypaste sync` mean
    /// "the other device has these items" rather than "the bytes have left".
    ///
    /// Never an error. A peer that says something else, or that never closes,
    /// has still received everything this side sent; the session succeeded
    /// either way and the deadline keeps a rude peer from holding the caller.
    pub async fn wait_for_close(&mut self) {
        match tokio::time::timeout(self.read_timeout, self.session.recv::<Box<RawValue>>()).await {
            Ok(Ok(None)) => {}
            Ok(Ok(Some(_))) => tracing::debug!("peer sent more after the session ended"),
            Ok(Err(e)) => tracing::debug!(error = %e, "peer session ended untidily"),
            Err(_) => tracing::debug!("peer did not close the session"),
        }
    }

    /// Flush and shut the write half down, so the peer's next read sees a clean
    /// close rather than a reset.
    pub async fn close(self) {
        if let Err(e) = self.session.close().await {
            tracing::debug!(error = %e, "peer session did not close cleanly");
        }
    }
}

impl SyncChannel for NoiseChannel {
    async fn send(&mut self, msg: SyncMessage) -> Result<(), SyncError> {
        let bytes = msg.encode()?;
        // `RawValue` is emitted verbatim, so what reaches the wire is exactly
        // the bytes `encode` validated.
        let raw =
            RawValue::from_string(String::from_utf8(bytes).map_err(|_| {
                SyncError::Channel("outgoing message was not valid text".to_string())
            })?)
            .map_err(|_| SyncError::Channel("outgoing message could not be framed".to_string()))?;

        self.session
            .send(&raw)
            .await
            .map_err(|e| SyncError::Channel(e.to_string()))
    }

    async fn recv(&mut self) -> Result<SyncMessage, SyncError> {
        let received =
            tokio::time::timeout(self.read_timeout, self.session.recv::<Box<RawValue>>()).await;

        match received {
            Err(_) => Err(SyncError::Channel(
                "the peer stopped responding".to_string(),
            )),
            // A clean close at a message boundary. The session functions expect
            // a message; ending here is the peer walking away mid-protocol.
            Ok(Ok(None)) => Err(SyncError::Channel(
                "the peer closed the session".to_string(),
            )),
            Ok(Ok(Some(raw))) => Ok(SyncMessage::decode(raw.get().as_bytes())?),
            Ok(Err(e)) => Err(SyncError::Channel(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::PairingToken;
    use tokio::net::TcpListener;

    async fn pair() -> (NoiseChannel, NoiseChannel) {
        let token = PairingToken::generate();
        let psk = token.psk();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let accepting = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            Session::accept(stream, &psk).await.unwrap()
        });
        let dialled = Session::connect(addr, &token.psk()).await.unwrap();
        let accepted = accepting.await.unwrap();
        (NoiseChannel::new(dialled), NoiseChannel::new(accepted))
    }

    #[tokio::test]
    async fn messages_round_trip_through_encode_and_decode() {
        let (mut a, mut b) = pair().await;
        a.send(SyncMessage::Hello {
            protocol_version: crate::PROTOCOL_VERSION,
            device_id: "device-a".into(),
            device_name: "A".into(),
        })
        .await
        .unwrap();

        match b.recv().await.unwrap() {
            SyncMessage::Hello { device_id, .. } => assert_eq!(device_id, "device-a"),
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn pin_metadata_round_trips_inside_the_authenticated_noise_record() {
        let (mut a, mut b) = pair().await;
        a.send(SyncMessage::Items {
            items: vec![crate::protocol::SyncItem {
                item_id: "item".into(),
                content: "shared".into(),
                content_type: "text".into(),
                created_at: 1,
                deleted: false,
                content_hash: crate::protocol::content_hash("shared"),
                origin_device_id: "device-a".into(),
                pinned: true,
                pin_order: Some(7.0),
                pin_updated_at: 2,
            }],
        })
        .await
        .unwrap();

        let SyncMessage::Items { items } = b.recv().await.unwrap() else {
            panic!("wrong message kind");
        };
        assert!(items[0].pinned);
        assert_eq!(items[0].pin_order, Some(7.0));
        assert_eq!(items[0].pin_updated_at, 2);
    }

    /// The case the sync author warned about: the far half aborts without a
    /// goodbye. The near half must fail, not wait forever.
    #[tokio::test]
    async fn a_peer_that_goes_silent_is_a_timeout_not_a_stuck_task() {
        let token = PairingToken::generate();
        let psk = token.psk();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let accepting = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let session = Session::accept(stream, &psk).await.unwrap();
            // Hold the connection open and say nothing.
            tokio::time::sleep(Duration::from_secs(30)).await;
            drop(session);
        });

        let mut channel = NoiseChannel::new(Session::connect(addr, &token.psk()).await.unwrap());
        channel.read_timeout = Duration::from_millis(150);

        let err = channel
            .recv()
            .await
            .expect_err("a silent peer must time out");
        assert!(matches!(err, SyncError::Channel(_)), "{err:?}");
        accepting.abort();
    }

    #[tokio::test]
    async fn a_peer_that_hangs_up_is_an_error_not_an_empty_message() {
        let (a, mut b) = pair().await;
        a.close().await;
        let err = b
            .recv()
            .await
            .expect_err("a closed session is not a message");
        assert!(matches!(err, SyncError::Channel(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_message_over_the_wire_bounds_is_refused_before_it_is_sent() {
        let (mut a, _b) = pair().await;
        // One item past the per-message item cap.
        let items = std::iter::repeat_with(|| crate::protocol::SyncItem {
            item_id: "x".into(),
            content: String::new(),
            content_type: "text".into(),
            created_at: 1,
            deleted: false,
            content_hash: "h".into(),
            origin_device_id: "d".into(),
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
        })
        .take(crate::protocol::MAX_ITEMS_PER_MESSAGE + 1)
        .collect();

        let err = a
            .send(SyncMessage::Items { items })
            .await
            .expect_err("the bound must be applied on the way out");
        assert!(matches!(err, SyncError::Protocol(_)), "{err:?}");
    }

    #[test]
    fn channel_errors_contain_no_paths() {
        for message in [
            SyncError::Channel("the peer stopped responding".into()).to_string(),
            SyncError::Channel("the peer closed the session".into()).to_string(),
        ] {
            assert!(!message.contains('/'), "{message}");
        }
    }
}
