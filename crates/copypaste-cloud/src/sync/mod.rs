//! The sync driver: local history in, cloud rows out, and back again.
//!
//! # This module does not decide which version wins
//!
//! Manifest 05 INV-C2: v1 had P2P using the full ordering while the cloud and
//! relay paths each used a cut-down comparison, so two devices holding
//! different content at an equal timestamp never converged. Here the comparator
//! lives behind [`CloudSource::apply_remote`], in the daemon's store, alongside
//! the one the P2P transport calls. This module hands a version down and is told
//! whether it won; there is no second comparator to drift.
//!
//! # `created_at` is a wall clock
//!
//! v2 has no Lamport stamp. What the stamp bought was monotonicity — a device
//! with a wrong clock could not outrank a newer write — and a wall clock can.
//! [`MAX_FUTURE_SKEW_MS`] is what replaces it: a version stamped further ahead
//! than that is refused, so one row stamped `i64::MAX` cannot win every future
//! comparison for its `item_id` and censor that item everywhere. Refusal skips
//! one version; it never fails the round and never deletes (R-CLK-2). No skew
//! correction is attempted.
//!
//! # The watermark
//!
//! One cursor, in milliseconds. Both bounds are **inclusive**: manifest 05 §4.4
//! records that a strict `>` on a millisecond cursor permanently lost every row
//! sharing the boundary millisecond once a burst filled a page. Re-offering a
//! boundary row is free — upsert and LWW both absorb it.
//!
//! The source stores it independently of the item rows, so evicting history to
//! a storage cap cannot move it backwards (INV-N5), and it only ever moves
//! forward, including past rows that were skipped as undecryptable, so an
//! unreadable row is not re-fetched forever (INV-I4).
//!
//! # A version has to prove where it came from
//!
//! [`CloudSync::pull`] verifies each row's metadata signature before anything
//! else looks at it, and refuses what does not verify — an unsigned or wrongly
//! signed row is not a version of anything, it is a write by something that
//! does not hold the sync key (manifest 05 §5.3). Refusing is not free of
//! consequences for the cursor, and the rule that follows from that is on
//! [`pull`].
//!
//! # Errors never carry a secret
//!
//! Every [`SyncError`] payload is a `&'static str`, so there is nothing to
//! interpolate a path, a token or a passphrase into.

pub mod adapters;
pub mod cadence;
pub mod driver;
pub mod outcome;
pub mod pull;
pub mod push;
pub mod retry;
pub mod source;
pub mod store;
pub mod transport;
pub mod unreadable;

#[cfg(test)]
mod fakes;

pub use cadence::{
    AdaptiveCadence, MAX_POLL_INTERVAL, MAX_POLL_INTERVAL_WITHOUT_PUSH, MIN_POLL_INTERVAL,
};
pub use driver::CloudSync;
pub use outcome::{SyncError, SyncStats};
pub use pull::MAX_FUTURE_SKEW_MS;
pub use push::{too_large_to_sync, MAX_BINARY_BYTES, MAX_TEXT_BYTES};
pub use source::{Applied, CloudSource, LocalItem, SensitiveGuard};
pub use store::{floor_after_round, Offer, Scan, StoreView, UPLOAD_SCAN_LIMIT};
pub use transport::{AuthApi, AuthFault, RestApi, TransportFault};
pub use unreadable::{Sweep, UnreadableUploads, UploadFloor};

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::fakes::{
        allow_everything, config, item, key, session, FakeAuth, FakeRest, FakeSource, ACCOUNT, PASS,
    };
    use super::retry::{MAX_RETRY_AFTER, RETRY_AFTER_FALLBACK};
    use super::*;
    use crate::rest::CloudItem;

    #[tokio::test]
    async fn two_devices_converge_through_the_backend() {
        // Device A pushes; device B, holding the same passphrase and account,
        // pulls and reads the plaintext back. This is the round trip that the
        // whole crate exists for.
        let backend = Arc::new(FakeRest::default());

        let a_source = FakeSource::with_outgoing(vec![item("a", 1_000, "shared clipboard text")]);
        let device_a = CloudSync::new(
            SharedRest(Arc::clone(&backend)),
            FakeAuth::default(),
            key(),
            config(),
            session("token-1"),
            allow_everything(),
        );
        device_a.push(&a_source).await.unwrap();

        let b_source = FakeSource::default();
        let device_b = CloudSync::new(
            SharedRest(Arc::clone(&backend)),
            FakeAuth::default(),
            // Re-derived independently: no shared state but the passphrase.
            crate::crypto::derive_sync_key(PASS, ACCOUNT).unwrap(),
            config(),
            session("token-2"),
            allow_everything(),
        );
        let stats = device_b.pull(&b_source).await.unwrap();

        assert_eq!(stats.applied, 1);
        assert_eq!(
            b_source.get("a").unwrap().content.as_slice(),
            b"shared clipboard text"
        );

        // Replaying the whole exchange changes nothing on either side.
        device_a.push(&a_source).await.unwrap();
        let replay = device_b.pull(&b_source).await.unwrap();
        assert_eq!(replay.applied, 0);
    }

    #[tokio::test]
    async fn a_file_payload_keeps_its_signed_metadata_through_cloud() {
        let backend = Arc::new(FakeRest::default());
        let metadata = r#"{"filename":"report.pdf","mime_type":"application/pdf"}"#;
        let a_source = FakeSource::with_outgoing(vec![LocalItem {
            item_id: "file-a".into(),
            content: zeroize::Zeroizing::new(b"%PDF-binary".to_vec()),
            content_type: "file".into(),
            payload_metadata: Some(metadata.into()),
            source_app_bundle_id: None,
            source_app_name: None,
            created_at: 1_000,
            deleted: false,
            origin_device_id: "device-a".into(),
        }]);
        let device_a = CloudSync::new(
            SharedRest(Arc::clone(&backend)),
            FakeAuth::default(),
            key(),
            config(),
            session("token-1"),
            allow_everything(),
        );
        device_a.push(&a_source).await.unwrap();

        let b_source = FakeSource::default();
        let device_b = CloudSync::new(
            SharedRest(Arc::clone(&backend)),
            FakeAuth::default(),
            crate::crypto::derive_sync_key(PASS, ACCOUNT).unwrap(),
            config(),
            session("token-2"),
            allow_everything(),
        );
        assert_eq!(device_b.pull(&b_source).await.unwrap().applied, 1);

        let received = b_source.get("file-a").unwrap();
        assert_eq!(received.content.as_slice(), b"%PDF-binary");
        assert_eq!(received.payload_metadata.as_deref(), Some(metadata));
    }

    /// One backend shared by two drivers.
    struct SharedRest(Arc<FakeRest>);

    impl RestApi for SharedRest {
        async fn fetch_since(
            &self,
            token: &str,
            since_ms: i64,
            after_item_id: Option<&str>,
            limit: u32,
        ) -> Result<Vec<CloudItem>, TransportFault> {
            self.0
                .fetch_since(token, since_ms, after_item_id, limit)
                .await
        }
        async fn upsert(&self, token: &str, items: &[CloudItem]) -> Result<(), TransportFault> {
            self.0.upsert(token, items).await
        }
    }

    #[test]
    fn the_tunables_are_the_documented_ones() {
        assert_eq!(MIN_POLL_INTERVAL, Duration::from_secs(5));
        assert_eq!(MAX_POLL_INTERVAL, Duration::from_secs(300));
        // Manifest 05 §4.8's "WS down / never connected" interval, exactly.
        assert_eq!(MAX_POLL_INTERVAL_WITHOUT_PUSH, Duration::from_secs(10));
        assert_eq!(RETRY_AFTER_FALLBACK, Duration::from_secs(1));
        assert_eq!(MAX_RETRY_AFTER, Duration::from_secs(30));
        // Mirrors `copypaste_p2p::sync::MAX_FUTURE_SKEW_MS`; the two transports
        // share an ordering, so they must share what counts as a valid stamp.
        assert_eq!(MAX_FUTURE_SKEW_MS, 24 * 60 * 60 * 1000);
    }
}
