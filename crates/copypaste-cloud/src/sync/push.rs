//! The upload path: what leaves the device, sealed, signed, and what never does.
//!
//! Three rules are enforced here and nowhere else in this module: a tombstone
//! is sent as a tombstone rather than as a row that happens to have a flag set,
//! the sensitive gate runs on live rows before anything is sealed or counted,
//! and every row is signed before it is handed to the transport.

use super::driver::CloudSync;
use super::outcome::{SyncError, SyncStats};
use super::source::{CloudSource, LocalItem};
use super::transport::{AuthApi, RestApi};
use crate::crypto::encrypt_row;
use crate::rest::CloudItem;

/// Rows per upsert request. Bounds request size without needing to measure it.
const UPLOAD_BATCH: usize = 50;

/// Per-item upload cap for text.
///
/// Split from [`MAX_BINARY_BYTES`] rather than unified at the larger number
/// because that is what the relay enforced (manifest 05 §5.1 row 13) and the
/// split exists so a text item large enough to be suspicious is refused while
/// an image of the same size is not.
pub const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;

/// Per-item upload cap for anything that is not text.
pub const MAX_BINARY_BYTES: usize = 10 * 1024 * 1024;

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    /// Seal every local change since the upload floor and upsert it.
    ///
    /// Moves no cursor at all. The download watermark is
    /// [`CloudSync::pull`]'s, and moving it here would advance it past rows
    /// another device wrote in the same window; the upload floor belongs to the
    /// source, which is the only side that knows when a round finished — see
    /// [`CloudSource::upload_floor`].
    ///
    /// Idempotent. Running it twice sends the same rows twice, and the second
    /// send is an upsert onto identical values.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] if the store fails, [`SyncError::Encrypt`] if a
    /// row cannot be sealed, and the auth and transport variants per
    /// [`CloudSync::pull`].
    pub async fn push(&self, source: &dyn CloudSource) -> Result<SyncStats, SyncError> {
        let since = source.upload_floor()?;
        let after_item_id = source.upload_floor_item_id()?;
        let device_id = source.device_id();
        let mut stats = SyncStats::default();

        let mut rows: Vec<CloudItem> = Vec::new();

        for mut item in source.local_changes_after(since, after_item_id.as_deref())? {
            // Preserve the original origin across hops; restamping it breaks
            // the ordering's final tie-break. Taken rather than cloned because
            // both branches below consume the rest of `item`.
            let origin = if item.origin_device_id.is_empty() {
                device_id.clone()
            } else {
                std::mem::take(&mut item.origin_device_id)
            };

            if item.deleted {
                // A tombstone carries no ciphertext, even if the caller handed
                // us a version that still has content in memory (T-4), and it
                // travels on the same upsert as a live row — the id-only PATCH
                // path is gone, because a partial write cannot sign the columns
                // it does not send.
                stats.tombstoned += 1;
                rows.push(self.signed(CloudItem::tombstone(
                    item.item_id,
                    item.content_type,
                    item.created_at,
                    origin,
                )));
                continue;
            }

            // The gate, before anything is sealed or counted. A sensitive item
            // is not merely withheld from this request — it is never given an
            // opportunity to reach the network at all.
            if self.sensitive.is_sensitive(&item) {
                stats.skipped_sensitive += 1;
                tracing::debug!(
                    item_id = %item.item_id,
                    "withholding a sensitive item from upload"
                );
                continue;
            }

            // Before sealing, so an item that cannot be uploaded is not paid
            // for in AEAD work, and so the limit is expressed in the size the
            // user can see rather than in base64-of-ciphertext.
            if let Some(limit) = over_size_limit(&item) {
                // Withheld, never deleted: the local copy is the durable one,
                // and refusing to upload an item is not a reason to lose it
                // (`AGENTS.md` rule 4).
                stats.skipped_too_large += 1;
                tracing::warn!(
                    item_id = %item.item_id,
                    bytes = item.content.len(),
                    limit,
                    "withholding an item larger than the per-item upload cap"
                );
                continue;
            }

            let (nonce, ciphertext) = encrypt_row(&item.content, &self.key, &item.item_id)
                .map_err(|_| SyncError::Encrypt)?;

            stats.uploaded += 1;
            rows.push(self.signed(CloudItem {
                item_id: item.item_id,
                ciphertext,
                nonce,
                content_type: item.content_type,
                payload_metadata: item.payload_metadata,
                source_app_bundle_id: item.source_app_bundle_id,
                source_app_name: item.source_app_name,
                created_at: item.created_at,
                // Always explicit, never left to the column default (T-5).
                deleted: false,
                origin_device_id: origin,
                signature: String::new(),
            }));
        }

        for batch in rows.chunks(UPLOAD_BATCH) {
            self.execute(|token| async move { self.rest.upsert(&token, batch).await })
                .await?;
        }

        Ok(stats)
    }

    /// Stamp the metadata signature.
    ///
    /// Here, on the way out, rather than in each constructor: this is the last
    /// point every outbound row passes through, so there is no second place a
    /// row can be built and shipped unsigned. `CloudItem::validate` refuses an
    /// unsigned row at the transport as the backstop.
    fn signed(&self, mut row: CloudItem) -> CloudItem {
        row.sign(&self.key);
        row
    }
}

/// The cap this item exceeded, or `None` if it fits.
///
/// Enforced here rather than left to the backend so the failure is local and
/// legible: the server's `octet_length(ciphertext) <= 16 MiB` check is a
/// backstop against a writer that is not this client, not the product limit,
/// and an item that trips it comes back as an opaque rejection with the whole
/// round already spent uploading it (manifest 05 §5.1 row 13).
fn over_size_limit(item: &LocalItem) -> Option<usize> {
    let limit = upload_limit(&item.content_type);
    too_large_to_sync(&item.content_type, item.content.len()).then_some(limit)
}

/// Would cloud sync refuse this plaintext payload for its size?
///
/// This is the same gate [`CloudSync::push`] applies before encryption. History
/// presenters call it so an item that can never upload is marked before the
/// first round, on every platform.
#[must_use]
pub fn too_large_to_sync(content_type: &str, byte_len: usize) -> bool {
    byte_len > upload_limit(content_type)
}

fn upload_limit(content_type: &str) -> usize {
    if content_type == copypaste_ipc::content_type::TEXT {
        MAX_TEXT_BYTES
    } else {
        MAX_BINARY_BYTES
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::super::fakes::{
        config, driver, item, key, session, tombstone, FakeAuth, FakeRest, FakeSource,
    };
    use super::*;
    use crate::crypto::decrypt_row;
    use crate::sync::SensitiveGuard;

    #[tokio::test]
    async fn push_seals_every_row_so_the_backend_never_sees_plaintext() {
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "hunter2 in the clear")]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        let stats = sync.push(&source).await.unwrap();
        assert_eq!(stats.uploaded, 1);

        let rows = sync.rest.rows.lock().unwrap();
        let row = &rows["a"];
        assert!(!row.ciphertext.contains("hunter2"));
        assert!(!row.nonce.is_empty());
        assert!(!row.deleted, "`deleted` must always be explicit (T-5)");

        // And it is the *cloud* key that opens it, bound to this item id.
        assert_eq!(
            decrypt_row(&row.ciphertext, &row.nonce, &key(), "a")
                .unwrap()
                .as_slice(),
            b"hunter2 in the clear"
        );
        assert!(decrypt_row(&row.ciphertext, &row.nonce, &key(), "b").is_err());
    }

    #[tokio::test]
    async fn the_nonce_and_ciphertext_are_not_transposed() {
        // `encrypt_row` returns `(nonce, ciphertext)`; a `CloudItem` names them
        // in the other order. Both are base64 `String`, so swapping them is a
        // type-correct, silent bug that only shows up as "every synced item is
        // undecryptable" on the *other* device. Assert the columns directly
        // rather than trusting the call site.
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "round trip")]);
        let sync = driver(FakeRest::default(), FakeAuth::default());
        sync.push(&source).await.unwrap();

        let rows = sync.rest.rows.lock().unwrap();
        let row = &rows["a"];

        // The nonce column holds exactly 24 bytes; the ciphertext column holds
        // the plaintext plus a 16-byte tag. Neither length is a coincidence, so
        // a transposition fails here even for a 24-byte plaintext.
        use base64::engine::general_purpose::STANDARD as B64;
        use base64::Engine as _;
        assert_eq!(B64.decode(&row.nonce).unwrap().len(), 24);
        assert_eq!(
            B64.decode(&row.ciphertext).unwrap().len(),
            "round trip".len() + 16
        );
        assert_eq!(
            decrypt_row(&row.ciphertext, &row.nonce, &key(), "a")
                .unwrap()
                .as_slice(),
            b"round trip"
        );
    }

    #[tokio::test]
    async fn every_row_that_leaves_the_device_is_signed() {
        // The upload half of manifest 05 §5.3. Asserted over the rows the
        // backend ends up holding rather than at the call site, so a second
        // construction path added later fails here.
        let mut dead = tombstone("gone", 3_000);
        dead.content = zeroize::Zeroizing::new(b"still in memory".to_vec());
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "live"), dead]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        let stats = sync.push(&source).await.unwrap();
        assert_eq!((stats.uploaded, stats.tombstoned), (1, 1));

        for row in sync.rest.sorted_rows() {
            row.verify(&key())
                .unwrap_or_else(|_| panic!("{} was uploaded unsigned", row.item_id));
        }
    }

    #[tokio::test]
    async fn a_tombstone_keeps_the_stores_created_at() {
        let stamped = 1_700_000_000_000i64;
        let source = FakeSource::with_outgoing(vec![tombstone("a", stamped)]);
        let sync = driver(FakeRest::default(), FakeAuth::default());
        sync.push(&source).await.unwrap();

        let rows = sync.rest.rows.lock().unwrap();
        let row = &rows["a"];
        assert!(row.deleted);
        assert_eq!(
            row.created_at, stamped,
            "push must not restamp a tombstone's created_at"
        );
        row.verify(&key()).expect("signed at the store stamp");
    }

    #[tokio::test]
    async fn push_is_idempotent() {
        let source =
            FakeSource::with_outgoing(vec![item("a", 1_000, "one"), item("b", 2_000, "two")]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        sync.push(&source).await.unwrap();
        let after_first: Vec<_> = sync
            .rest
            .sorted_rows()
            .iter()
            .map(|r| r.item_id.clone())
            .collect();

        sync.push(&source).await.unwrap();
        let after_second: Vec<_> = sync
            .rest
            .sorted_rows()
            .iter()
            .map(|r| r.item_id.clone())
            .collect();

        // Same set of rows: the upsert conflict target is `item_id`, so a
        // replay overwrites rather than duplicating.
        assert_eq!(after_first, after_second);
        assert_eq!(sync.rest.rows.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_sensitive_item_is_never_uploaded() {
        // CopyPaste-20yw / AT-56. The store is *supposed* to filter these; this
        // asserts the second layer, because this is data leaving the machine.
        let source = FakeSource::with_outgoing(vec![
            item("safe", 1_000, "a normal snippet"),
            item("secret", 2_000, "AKIAIOSFODNN7EXAMPLE"),
        ]);
        let sync = CloudSync::new(
            FakeRest::default(),
            FakeAuth::default(),
            key(),
            config(),
            session("token-1"),
            SensitiveGuard::new(|item| item.item_id == "secret"),
        );

        let stats = sync.push(&source).await.unwrap();

        assert_eq!(stats.uploaded, 1);
        assert_eq!(stats.skipped_sensitive, 1);

        let rows = sync.rest.rows.lock().unwrap();
        assert!(rows.contains_key("safe"));
        assert!(
            !rows.contains_key("secret"),
            "a sensitive item reached the backend"
        );
    }

    #[tokio::test]
    async fn a_sensitive_tombstone_is_uploaded() {
        let mut dead = tombstone("secret", 3_000);
        dead.content = zeroize::Zeroizing::new(b"AKIAIOSFODNN7EXAMPLE".to_vec());
        let source = FakeSource::with_outgoing(vec![dead]);
        let sync = CloudSync::new(
            FakeRest::default(),
            FakeAuth::default(),
            key(),
            config(),
            session("token-1"),
            SensitiveGuard::new(|item| item.item_id == "secret"),
        );

        let stats = sync.push(&source).await.unwrap();
        assert_eq!(stats.tombstoned, 1);
        assert_eq!(stats.skipped_sensitive, 0);

        let rows = sync.rest.rows.lock().unwrap();
        let row = rows
            .get("secret")
            .expect("sensitive tombstone was withheld");
        assert!(row.deleted);
        assert!(row.ciphertext.is_empty());
    }

    #[tokio::test]
    async fn a_sensitive_item_is_withheld_even_when_it_is_the_only_one() {
        // The batching must not turn "nothing to upload" into an empty request
        // that still counts as an upload.
        let source = FakeSource::with_outgoing(vec![item("secret", 1_000, "x")]);
        let sync = CloudSync::new(
            FakeRest::default(),
            FakeAuth::default(),
            key(),
            config(),
            session("token-1"),
            SensitiveGuard::new(|_| true),
        );

        let stats = sync.push(&source).await.unwrap();
        assert_eq!(stats.uploaded, 0);
        assert_eq!(stats.skipped_sensitive, 1);
        assert_eq!(sync.rest.upserts.load(Ordering::SeqCst), 0);
        assert!(sync.rest.rows.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_delete_travels_as_a_tombstone_and_carries_no_ciphertext() {
        // T-4: even if the local version still has content in memory, the
        // tombstone must not carry it.
        let mut dead = tombstone("a", 3_000);
        dead.content = zeroize::Zeroizing::new(b"content that must not be uploaded".to_vec());

        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "live"), dead]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        let stats = sync.push(&source).await.unwrap();
        assert_eq!(stats.uploaded, 1);
        assert_eq!(stats.tombstoned, 1);

        let rows = sync.rest.rows.lock().unwrap();
        let row = &rows["a"];
        assert!(row.deleted, "the tombstone did not propagate");
        assert!(row.ciphertext.is_empty(), "a tombstone carried ciphertext");
    }

    /// Manifest 05 §5.1 row 13: the caps are the client's, split by type, and
    /// an item over one of them is withheld rather than lost or uploaded.
    #[tokio::test]
    async fn an_item_over_the_per_type_cap_is_withheld_and_the_rest_still_upload() {
        let big_text = || {
            let mut item = item("huge-text", 1_000, "");
            item.content = zeroize::Zeroizing::new(vec![b'x'; MAX_TEXT_BYTES + 1]);
            item
        };
        // The same bytes as an image are under the binary cap and go up.
        let big_image = || {
            let mut item = item("big-image", 2_000, "");
            item.content = zeroize::Zeroizing::new(vec![0u8; MAX_TEXT_BYTES + 1]);
            item.content_type = "image".into();
            item
        };
        let huge_image = || {
            let mut item = item("huge-image", 3_000, "");
            item.content = zeroize::Zeroizing::new(vec![0u8; MAX_BINARY_BYTES + 1]);
            item.content_type = "image".into();
            item
        };

        let source = FakeSource::with_outgoing(vec![
            big_text(),
            big_image(),
            huge_image(),
            item("ok", 4_000, "small"),
        ]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        let stats = sync.push(&source).await.unwrap();

        assert_eq!(stats.skipped_too_large, 2);
        assert_eq!(stats.uploaded, 2);
        let rows = sync.rest.rows.lock().unwrap();
        assert!(rows.contains_key("ok"));
        assert!(
            rows.contains_key("big-image"),
            "8 MiB is under the binary cap"
        );
        assert!(!rows.contains_key("huge-text"));
        assert!(!rows.contains_key("huge-image"));
    }

    #[test]
    fn the_caps_are_the_manifests_and_the_boundary_is_inclusive() {
        let mut at_limit = item("a", 1, "");
        at_limit.content = zeroize::Zeroizing::new(vec![b'x'; MAX_TEXT_BYTES]);
        assert_eq!(over_size_limit(&at_limit), None, "the cap itself must fit");
        assert!(!too_large_to_sync("text", MAX_TEXT_BYTES));

        at_limit.content.push(b'x');
        assert_eq!(over_size_limit(&at_limit), Some(MAX_TEXT_BYTES));
        assert!(too_large_to_sync("text", MAX_TEXT_BYTES + 1));
        assert!(!too_large_to_sync("text/rtf", MAX_TEXT_BYTES + 1));
        assert!(too_large_to_sync("image/png", MAX_BINARY_BYTES + 1));

        assert_eq!(MAX_TEXT_BYTES, 8 * 1024 * 1024);
        assert_eq!(MAX_BINARY_BYTES, 10 * 1024 * 1024);
    }

    #[tokio::test]
    async fn push_does_not_move_the_download_watermark() {
        let source = FakeSource::with_outgoing(vec![item("a", 5_000, "x")]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        sync.push(&source).await.unwrap();
        assert_eq!(source.watermark().unwrap(), 0);
    }

    /// The upload floor is what push offers from, so a download watermark that
    /// has run ahead of local time cannot strand local items.
    #[tokio::test]
    async fn push_offers_from_the_upload_floor_not_the_watermark() {
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "written here")]);
        // Another device's rows dragged the download cursor past ours.
        source.set_watermark(9_000).unwrap();

        let sync = driver(FakeRest::default(), FakeAuth::default());
        let stats = sync.push(&source).await.unwrap();
        assert_eq!(
            stats.uploaded, 1,
            "a local item was stranded below the cursor"
        );
        assert!(sync.rest.rows.lock().unwrap().contains_key("a"));

        // Once the round is over and the owner advances the floor past it, the
        // same item stops being re-offered.
        source.set_upload_floor(2_000);
        assert_eq!(sync.push(&source).await.unwrap().uploaded, 0);
    }
}
