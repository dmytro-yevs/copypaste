//! The upload path: what leaves the device, sealed, and what never does.
//!
//! Two rules are enforced here and nowhere else in this module: the sensitive
//! gate runs before anything is sealed or counted, and a tombstone is sent as a
//! tombstone rather than as a row that happens to have a flag set.

use super::driver::CloudSync;
use super::outcome::{SyncError, SyncStats};
use super::source::CloudSource;
use super::transport::{AuthApi, RestApi};
use crate::crypto::encrypt_row;
use crate::rest::CloudItem;

/// Rows per upsert request. Bounds request size without needing to measure it.
const UPLOAD_BATCH: usize = 50;

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
        let device_id = source.device_id();
        let mut stats = SyncStats::default();

        let mut live: Vec<CloudItem> = Vec::new();
        let mut dead: Vec<String> = Vec::new();

        for item in source.local_changes_since(since)? {
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

            if item.deleted {
                // A tombstone carries no ciphertext, even if the caller handed
                // us a version that still has content in memory (T-4).
                dead.push(item.item_id);
                continue;
            }

            let (nonce, ciphertext) = encrypt_row(&item.content, &self.key, &item.item_id)
                .map_err(|_| SyncError::Encrypt)?;

            live.push(CloudItem {
                item_id: item.item_id,
                ciphertext,
                nonce,
                content_type: item.content_type,
                created_at: item.created_at,
                // Always explicit, never left to the column default (T-5).
                deleted: false,
                origin_device_id: if item.origin_device_id.is_empty() {
                    device_id.clone()
                } else {
                    // Preserve the original origin across hops; restamping it
                    // breaks the ordering's final tie-break.
                    item.origin_device_id
                },
            });
        }

        for batch in live.chunks(UPLOAD_BATCH) {
            self.execute(|token| async move { self.rest.upsert(&token, batch).await })
                .await?;
            stats.uploaded += batch.len();
        }
        for batch in dead.chunks(UPLOAD_BATCH) {
            self.execute(|token| async move { self.rest.tombstone(&token, batch).await })
                .await?;
            stats.tombstoned += batch.len();
        }

        Ok(stats)
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
            decrypt_row(&row.ciphertext, &row.nonce, &key(), "a").unwrap(),
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
            decrypt_row(&row.ciphertext, &row.nonce, &key(), "a").unwrap(),
            b"round trip"
        );
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
        dead.content = b"content that must not be uploaded".to_vec();

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
