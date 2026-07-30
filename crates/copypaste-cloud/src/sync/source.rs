//! The local seam: what the driver is allowed to see of the daemon's history,
//! and the gate every item passes before it may leave the machine.
//!
//! Nothing here touches the network, and nothing here touches SQLite. It is the
//! contract between the two.

use std::sync::Arc;

use super::outcome::SyncError;

// ---------------------------------------------------------------------------
// The local side
// ---------------------------------------------------------------------------

/// One version of one item, in the plain.
///
/// This is the shape either side of the encryption boundary: what the daemon
/// hands up to be sealed, and what comes back down after opening. `content` is
/// plaintext here and nowhere else — the moment it leaves this process it is
/// ciphertext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalItem {
    /// Cross-device logical identity. Bound into the AEAD's associated data,
    /// and the conflict target of the upsert. Never a local row primary key
    /// (manifest 05 R-ID-1).
    pub item_id: String,
    /// Plaintext. Empty for a tombstone — see [`LocalItem::deleted`].
    pub content: Vec<u8>,
    pub content_type: String,
    /// Version stamp, Unix milliseconds.
    pub created_at: i64,
    /// A tombstone. Carried on the wire and persisted on the receiver
    /// (manifest 05 T-2); a tombstone never carries ciphertext (T-4).
    pub deleted: bool,
    /// The device that produced *this version*. Preserved across hops — never
    /// restamped with the forwarding device, or the ordering's final tie-break
    /// stops being stable.
    pub origin_device_id: String,
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
    /// **Sensitive items must not appear here.** The driver checks again
    /// ([`SensitiveGuard`]) because this is data leaving the user's machine and
    /// one enforcement point is one bug away from none — but the filter belongs
    /// here too, where the detector already ran at capture time.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn local_changes_since(&self, since_ms: i64) -> Result<Vec<LocalItem>, SyncError>;

    /// Merge one remote version into the local history, returning whether
    /// anything changed.
    ///
    /// **The implementor owns the ordering decision, and it must be the same
    /// comparator the P2P transport uses** — `copypaste_p2p::sync::merge_decision`,
    /// ordering on `created_at`, then `content_hash`, then `deleted`, then
    /// `origin_device_id` (manifest 05 INV-C2). Four keys, and `deleted` sits
    /// third rather than last for a reason worth reading before reimplementing
    /// this: a tombstone keeps the hash of the item it deletes, so a delete ties
    /// its own live version on the first two keys, and an order that consulted
    /// the origin before `deleted` would decide it by device id — which
    /// resurrects deletes (`CopyPaste-ojhe`, INV-N2). Do not restate the order;
    /// call the function.
    ///
    /// `created_at` is a wall clock, not a logical one:
    /// v2 has no Lamport stamp, and what makes a wall clock safe as the primary
    /// key is the refusal in [`CloudSync::pull`](super::CloudSync::pull) of
    /// versions stamped further ahead than
    /// [`MAX_FUTURE_SKEW_MS`](super::MAX_FUTURE_SKEW_MS). Anything arriving here
    /// has already passed that check and has already been clamped non-negative.
    ///
    /// Three requirements come with owning the decision:
    ///
    /// * **Equal on every ordering key ⇒ keep local, report `false`.** This is
    ///   what makes replay and self-echo free (INV-I1, INV-I2).
    /// * **A tombstone for an unknown `item_id` must still be persisted** as a
    ///   tombstone row. Dropping it lets a later-arriving create resurrect the
    ///   item (T-3, `CopyPaste-bfiu`).
    /// * **Preserve the local row primary key** on a replace, or the full-text
    ///   index and pins that reference it are orphaned (R-ID-2).
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure. A version this device
    /// declines to take is `Ok(false)`, not an error.
    fn apply_remote(&self, item: LocalItem) -> Result<bool, SyncError>;

    /// The reconciliation cursor, in Unix milliseconds. Zero on a first run.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn watermark(&self) -> Result<i64, SyncError>;

    /// The tie-break half of that cursor: the `item_id` of the last row the
    /// millisecond covers, if this source persists one.
    ///
    /// The cursor is a keyset over `(created_at, item_id)`, because a
    /// millisecond is not unique and a bound over a non-unique key cannot be
    /// paged past: once more than one page of rows shares one `created_at`, a
    /// millisecond-only cursor re-fetches the same first page forever and the
    /// rows behind it never download (manifest 05 §5.1 row 6, INV-N1, AT-24).
    ///
    /// `None` — the default — keeps the millisecond-only behaviour *across*
    /// rounds: the next round re-offers the boundary millisecond, which is free
    /// (INV-I1), and [`CloudSync::pull`](super::CloudSync::pull) still carries
    /// the full keyset from page to page *within* a round. That bounds the
    /// stall at one drain's worth of rows rather than removing it. A source
    /// that persists both halves has no stall at all, and it is two columns of
    /// work.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn watermark_item_id(&self) -> Result<Option<String>, SyncError> {
        Ok(None)
    }

    /// The cursor [`CloudSync::push`](super::CloudSync::push) offers from.
    ///
    /// Defaults to [`CloudSource::watermark`], which is what a source with
    /// nothing better to say should use. It is a separate method because the
    /// two cursors measure different things, and using the download watermark
    /// for both loses uploads in two ways:
    ///
    /// * **The watermark can outrun local time.** It advances to the newest
    ///   stamp *another* device wrote, so on a device whose clock runs behind
    ///   its peers' the local items sit below it and are never offered.
    /// * **Signing in has a backlog to send.** The watermark says nothing about
    ///   what has been uploaded, so a source that conflates them uploads only
    ///   what is captured *after* sign-in (manifest 05 §4.9, BUG C2).
    ///
    /// The driver never advances this: it does not know when a round is over
    /// from the source's point of view, and advancing it on a round that then
    /// failed would drop everything in the window. The owner of the source
    /// advances it, after a complete round, to the instant that round *began*.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn upload_floor(&self) -> Result<i64, SyncError> {
        self.watermark()
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

// ---------------------------------------------------------------------------
// The sensitive-content gate
// ---------------------------------------------------------------------------

/// The last check before an item leaves the machine.
///
/// `CloudSource` filters sensitive items already — the detector ran at capture
/// time and the store knows the answer. This is the second layer, and it exists
/// because manifest 05 AT-56 (`CopyPaste-20yw`) records that v1 had exactly one
/// enforcement point and it had a hole: the backlog sweep took a different path
/// to the same table and did not consult it.
///
/// It is required by [`CloudSync::new`](super::CloudSync::new) rather than
/// defaulted, so there is no way to construct a driver that uploads unchecked.
/// It is a callback rather than a detector because the detector already exists —
/// `copypaste-core`'s `sensitive::Detector`, with the full ruleset, the
/// confidence model and the false-positive defences of manifest 07.
/// Re-implementing even a "quick check" here would be a second regex engine,
/// which is one of the duplications `CLAUDE.md` rule 1 was written about. The
/// daemon wires the real detector in `cloud::sensitive_guard`.
#[derive(Clone)]
pub struct SensitiveGuard(Arc<dyn Fn(&LocalItem) -> bool + Send + Sync>);

impl SensitiveGuard {
    /// Wrap a predicate. `true` means "never upload this".
    pub fn new(f: impl Fn(&LocalItem) -> bool + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// Would this item be withheld?
    pub fn is_sensitive(&self, item: &LocalItem) -> bool {
        (self.0)(item)
    }
}

impl std::fmt::Debug for SensitiveGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SensitiveGuard").finish_non_exhaustive()
    }
}
