//! The sync driver: local history in, cloud rows out, and back again.
//!
//! # This module does not decide which version wins
//!
//! Manifest 05 INV-C2 requires every transport to use the same full ordering.
//! The comparator lives behind [`CloudSource::apply_remote`], in the daemon's
//! store, alongside the one the P2P transport calls. This module hands a version
//! down and is told whether it won; there is no second comparator to drift.
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
#[cfg(feature = "storage")]
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
pub use source::{Applied, CloudSource, LocalItem};
#[cfg(feature = "storage")]
pub use store::{floor_after_round, Offer, Scan, StoreView, UPLOAD_SCAN_LIMIT};
pub use transport::{AuthApi, AuthFault, RestApi, TransportFault};
pub use unreadable::{Sweep, UnreadableUploads, UploadFloor};

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::fakes::{config, item, key, session, FakeAuth, FakeRest, FakeSource, ACCOUNT, PASS};
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
        let metadata = r#"{"filename":"report.pdf","mime_type":"application/pdf","source_reference":"/Users/person/Documents/report.pdf"}"#;
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
        );
        device_a.push(&a_source).await.unwrap();

        let b_source = FakeSource::default();
        let device_b = CloudSync::new(
            SharedRest(Arc::clone(&backend)),
            FakeAuth::default(),
            crate::crypto::derive_sync_key(PASS, ACCOUNT).unwrap(),
            config(),
            session("token-2"),
        );
        assert_eq!(device_b.pull(&b_source).await.unwrap().applied, 1);

        let received = b_source.get("file-a").unwrap();
        assert_eq!(received.content.as_slice(), b"%PDF-binary");
        assert_eq!(received.payload_metadata.as_deref(), Some(metadata));
    }

    #[cfg(feature = "storage")]
    #[tokio::test(flavor = "multi_thread")]
    async fn explicit_privacy_survives_cloud_and_relay_without_inventing_legacy_flags() {
        use copypaste_core::{Keyring, PayloadMetadata, SourceAppIconMetadata, Store, StoreSource};
        use copypaste_ipc::{ClipboardPrivacy, ConfigData};
        let make = |secret, name: &str| {
            let keys = Arc::new(Keyring::from_secret(&[secret; 32]));
            let store = Store::open_in_memory(&keys.db_key()).unwrap();
            let identity = store.device_identity(name).unwrap();
            let shared = StoreSource::new(
                store.clone(),
                Arc::clone(&keys),
                identity.device_id.clone(),
                identity.device_name,
                ConfigData::default(),
            );
            let view = StoreView::new(shared, identity.device_id);
            (keys, store, view)
        };
        let (sender_keys, sender, sender_view) = make(111, "sender");
        let (receiver_keys, receiver, receiver_view) = make(112, "receiver");
        let (_, relay, relay_view) = make(113, "relay");
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let privacy = ClipboardPrivacy {
            secret: true,
            transient: true,
        };
        let metadata = PayloadMetadata {
            secret_override: None,
            privacy,
            file: None,
            source_app_icon: SourceAppIconMetadata::new(&png.into_inner(), 1, 1),
        }
        .to_json("text")
        .unwrap();
        for (id, metadata) in [
            ("private", metadata),
            ("legacy", r#"{"legacy_private":true}"#.into()),
        ] {
            let text = format!("synthetic {id} cloud record");
            let (nonce, content_ciphertext) =
                copypaste_core::encrypt(text.as_bytes(), &sender_keys.item_key(), id).unwrap();
            sender
                .insert(copypaste_core::NewItem {
                    id: id.into(),
                    nonce,
                    content_ciphertext,
                    content_type: "text".into(),
                    content_hash: copypaste_core::compute_content_hash(text.as_bytes()),
                    search_text: None,
                    created_at: 1000,
                    app_bundle_id: Some(format!("com.example.{id}")),
                    app_name: Some("Synthetic".into()),
                    payload_metadata: Some(metadata),
                })
                .unwrap();
        }
        let backend = Arc::new(FakeRest::default());
        let device_a = CloudSync::new(
            SharedRest(Arc::clone(&backend)),
            FakeAuth::default(),
            key(),
            config(),
            session("token-1"),
        );
        let outgoing = sender_view
            .scan(&sender, 0, None, 0, &UnreadableUploads::default())
            .unwrap()
            .items;
        assert_eq!(outgoing.len(), 2);
        let a_source = FakeSource::with_outgoing(outgoing);
        assert_eq!(device_a.push(&a_source).await.unwrap().uploaded, 2);
        let device_b = CloudSync::new(
            SharedRest(Arc::clone(&backend)),
            FakeAuth::default(),
            key(),
            config(),
            session("token-2"),
        );
        let b_source = FakeSource::default();
        let downloaded = device_b.pull(&b_source).await.unwrap();
        assert_eq!(
            (
                downloaded.applied,
                downloaded.skipped_forged,
                downloaded.skipped_undecryptable
            ),
            (2, 0, 0)
        );
        let incoming = ["private", "legacy"]
            .map(|id| b_source.get(id).unwrap())
            .to_vec();
        assert!(receiver_view
            .apply_page(incoming)
            .unwrap()
            .iter()
            .all(|outcome| matches!(outcome, Applied::Merged)));
        let forwarded = receiver_view
            .scan(&receiver, 0, None, 0, &UnreadableUploads::default())
            .unwrap()
            .items;
        assert!(relay_view
            .apply_page(forwarded)
            .unwrap()
            .iter()
            .all(|outcome| matches!(outcome, Applied::Merged)));
        for id in ["private", "legacy"] {
            let row = receiver.get(id).unwrap().unwrap();
            let clear = copypaste_core::decrypt(
                &row.content_ciphertext,
                &row.nonce,
                &receiver_keys.item_key(),
                id,
            )
            .unwrap();
            assert_eq!(
                clear.as_slice(),
                format!("synthetic {id} cloud record").as_bytes()
            );
            for (store, view) in [(&receiver, &receiver_view), (&relay, &relay_view)] {
                let row = store.get(id).unwrap().unwrap();
                assert_eq!(row.clipboard_privacy().secret, id == "private");
                assert_eq!(
                    store
                        .search("synthetic", 10)
                        .unwrap()
                        .into_iter()
                        .map(|item| item.id)
                        .collect::<Vec<_>>(),
                    ["legacy"],
                );
                let metadata = view
                    .shared()
                    .store()
                    .payload_metadata_for_sync(&row)
                    .unwrap();
                if id == "private" {
                    let envelope = PayloadMetadata::from_json(&metadata.unwrap(), "text").unwrap();
                    assert_eq!(envelope.privacy, privacy);
                    assert!(envelope.source_app_icon.is_some());
                } else {
                    assert!(row.clipboard_privacy().is_empty());
                    assert!(metadata.is_none());
                }
            }
        }
        assert_eq!(
            sender
                .get("legacy")
                .unwrap()
                .unwrap()
                .payload_metadata
                .as_deref(),
            Some(r#"{"legacy_private":true}"#)
        );
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
