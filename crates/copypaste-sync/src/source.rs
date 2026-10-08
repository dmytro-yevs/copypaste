//! The local seam: what the driver is allowed to see of the daemon's history.
//!
//! Nothing here touches the network, and nothing here touches SQLite. It is the
//! contract between the two.

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::outcome::SyncError;

// The local side

/// One version of one item, in the plain.
///
/// This is the shape either side of the encryption boundary: what the daemon
/// hands up to be sealed, and what comes back down after opening. `content` is
/// plaintext here and nowhere else — the moment it leaves this process it is
/// ciphertext.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalItem {
    /// Cross-device logical identity. Bound into the AEAD's associated data,
    /// and the conflict target of the upsert. Never a local row primary key
    /// (manifest 05 R-ID-1).
    pub item_id: String,
    /// Plaintext. Empty for a tombstone — see [`LocalItem::deleted`].
    #[serde(with = "payload")]
    pub content: Zeroizing<Vec<u8>>,
    pub content_type: String,
    /// Opaque payload metadata needed to restore a binary value faithfully.
    /// For file payloads this is the validated JSON form of `FileMetadata`.
    /// It is signed before leaving the device and never interpreted here.
    pub payload_metadata: Option<String>,
    /// Version stamp, Unix milliseconds.
    pub created_at: i64,
    /// A tombstone. Carried on the wire and persisted on the receiver
    /// (manifest 05 T-2); a tombstone never carries ciphertext (T-4).
    pub deleted: bool,
    /// The device that produced *this version*. Preserved across hops — never
    /// restamped with the forwarding device, or the ordering's final tie-break
    /// stops being stable.
    pub origin_device_id: String,
    pub source_app_bundle_id: Option<String>,
    pub source_app_name: Option<String>,
}

/// What [`CloudSource::apply_remote`] did with the version it was given.
///
/// `Declined` hands the item back rather than reporting `false`, because the
/// caller needs it again for [`CloudSource::requeue_local_winner`] and that is
/// the branch it usually does not take: returning it is what lets the applied
/// path — every row of a catch-up drain — move the decrypted plaintext instead
/// of copying it speculatively.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Applied {
    /// The remote version won and the local history changed.
    Merged,
    /// The local copy won, or the two were equal on every ordering key. The
    /// local history is unchanged (INV-I1).
    Declined(LocalItem),
}

/// The daemon's history, as this driver needs to see it.
///
/// Implementations live over the store. The driver never touches SQLite, and
/// the store never touches the network.
pub trait CloudSource: Send + Sync {
    /// This device's stable id. Stamped onto rows this device uploads.
    fn device_id(&self) -> String;

    /// Local versions with `created_at >= since_ms`, where `since_ms` is
    /// [`CloudSource::upload_floor`] — not the download watermark.
    ///
    /// Inclusive on purpose: re-offering the boundary row costs one idempotent
    /// upsert, and excluding it loses every row that shares the boundary
    /// millisecond.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn local_changes_since(&self, since_ms: i64) -> Result<Vec<LocalItem>, SyncError>;

    /// Local versions after an optional upload keyset cursor.
    ///
    /// Sources that have not persisted the tie-break retain the inclusive
    /// timestamp behaviour. Production sources override this with a database
    /// keyset query so a full timestamp bucket cannot stall behind a page cap.
    fn local_changes_after(
        &self,
        since_ms: i64,
        after_item_id: Option<&str>,
    ) -> Result<Vec<LocalItem>, SyncError> {
        let mut items = self.local_changes_since(since_ms)?;
        if let Some(item_id) = after_item_id {
            items.retain(|item| (item.created_at, item.item_id.as_str()) > (since_ms, item_id));
        }
        Ok(items)
    }

    /// Merge one remote version into the local history, reporting what happened
    /// and handing the item back when it was declined.
    ///
    /// Recheck the current local version atomically, preserving its local row id.
    /// Use the shared P2P comparator: timestamp, content hash, deleted, origin.
    /// A timestamp/hash tie chooses the tombstone before comparing origin so
    /// delayed creates cannot resurrect deletions. Maintain FTS in the same
    /// transaction. Providers authenticate rows and enforce clock skew before
    /// applying them; absence from a transit backend never means deletion.
    fn apply_remote(&self, item: LocalItem) -> Result<Applied, SyncError>;

    fn apply_remote_batch(&self, items: Vec<LocalItem>) -> Result<Vec<Applied>, SyncError> {
        items
            .into_iter()
            .map(|item| self.apply_remote(item))
            .collect()
    }

    /// The reconciliation cursor, in Unix milliseconds. Zero on a first run.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn watermark(&self) -> Result<i64, SyncError>;

    /// The persisted item-id half of the download keyset. Timestamp-only
    /// sources replay the inclusive boundary; current hosts persist both halves.
    fn watermark_item_id(&self) -> Result<Option<String>, SyncError> {
        Ok(None)
    }

    /// Upload progress is independent from the download watermark. Advance it
    /// after a complete round to its start time; late peer writes lower it.
    fn upload_floor(&self) -> Result<i64, SyncError> {
        self.watermark()
    }

    /// The tie-break half of [`upload_floor`](Self::upload_floor).
    fn upload_floor_item_id(&self) -> Result<Option<String>, SyncError> {
        Ok(None)
    }

    /// Schedule a local LWW winner for upload after an incoming cloud row was
    /// declined. The default is for sources that cannot preserve a local
    /// version independently of their watermark.
    fn requeue_local_winner(&self, _incoming: &LocalItem) -> Result<bool, SyncError> {
        Ok(false)
    }

    /// Persist the cursor.
    ///
    /// Must be stored independently of the item rows so that pruning history to
    /// a storage cap cannot move it backwards (INV-N5), and should be written
    /// in the same transaction as the rows it covers so that a crash cannot
    /// lose it. Losing it costs re-pagination, not data.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn set_watermark(&self, ms: i64) -> Result<(), SyncError>;

    /// Persist both halves of the cursor.
    ///
    /// `item_id` is the last row covered by `ms`. The default drops it, which
    /// is the millisecond-only cursor described on
    /// [`CloudSource::watermark_item_id`]; a source that overrides one of the
    /// two should override both, or the halves disagree.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn set_watermark_keyset(&self, ms: i64, item_id: &str) -> Result<(), SyncError> {
        let _ = item_id;
        self.set_watermark(ms)
    }
}

mod payload {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};
    use zeroize::Zeroizing;
    pub fn serialize<S: Serializer>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(value))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Zeroizing<Vec<u8>>, D::Error> {
        let encoded = Zeroizing::new(String::deserialize(deserializer)?);
        STANDARD
            .decode(encoded.as_bytes())
            .map(Zeroizing::new)
            .map_err(serde::de::Error::custom)
    }
}
