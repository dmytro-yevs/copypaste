//! Clip expiry: the deadline a clip gets, who purges it and how, and the
//! cloud withholding of short-lived clips.
//!
//! A local deadline only ever moves earlier: it is fixed at the first local
//! observation, and every later one can only lower it. Each deadline also
//! records a reading of a clock that counts sleep, so moving the wall clock
//! back cannot extend a clip's life. Pinned and kept clips never expire. The
//! device that set an expiry purges with a synced tombstone; every other
//! device deletes its copy locally and remembers the id so a re-arrival is
//! dropped. A clip that expires within an hour of its creation is never
//! uploaded to cloud, nor is its tombstone or its note.

use std::collections::HashSet;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};

use super::connection::write_tx;
use super::local_state::{self, ClipLocalState};
use super::organization::{self, ClipNoteRecord, Record, RecordKey};
use super::search::delete_fts_row_in_tx;
use super::{IncomingItem, Store, StoreError};

/// Clips that expire within this long of their creation are kept out of cloud.
pub const CLOUD_WITHHOLD_MS: i64 = 60 * 60 * 1000;
/// `clip_local_state.eval_flags` bit: this device set the expiry and purges
/// with a synced tombstone. Above every rule evaluation flag.
pub const EXPIRY_OWNER: i64 = 1 << 24;
/// `clip_local_state.eval_flags` bit: clear the system clipboard at expiry if
/// it still holds the clip.
pub const CLEAR_CLIPBOARD_AT_EXPIRY: i64 = 1 << 25;
const PURGE_BATCH: i64 = 256;

/// The two clocks an expiry is checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clocks {
    /// Wall-clock Unix milliseconds.
    pub wall_ms: i64,
    /// Milliseconds since boot, counting sleep; `None` where unavailable.
    pub boot_ms: Option<i64>,
}

impl Clocks {
    #[must_use]
    pub fn now() -> Self {
        Self::at(crate::now_ms())
    }

    /// `wall_ms` with the real boot clock.
    #[must_use]
    pub fn at(wall_ms: i64) -> Self {
        Self {
            wall_ms,
            boot_ms: boot_clock_ms(),
        }
    }
}

/// A synced deadline and the duration it was set with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncedExpiry {
    pub expires_at: Option<i64>,
    pub duration_ms: Option<i64>,
}

/// What [`Store::keep_clip`] changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeepOutcome {
    /// A keep note was written and the local expiry cleared.
    pub kept: bool,
    /// Ids that were withheld from cloud and must be offered to it again.
    pub reoffer: Vec<String>,
}

/// What one [`Store::expire_due`] removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExpirySweep {
    /// Clips this device set the expiry of: tombstoned, the delete syncs.
    pub tombstoned: Vec<String>,
    /// Clips deleted here only, without a tombstone.
    pub purged_locally: usize,
    /// Expired clips whose clipboard copy is cleared if it is still current.
    pub clear_clipboard: Vec<String>,
}

/// Milliseconds on a clock that keeps counting while the system sleeps.
#[must_use]
pub fn boot_clock_ms() -> Option<i64> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let elapsed =
        Duration::try_from(rustix::time::clock_gettime(rustix::time::ClockId::Boottime)).ok();
    // Apple's CLOCK_MONOTONIC continues to advance while the system sleeps.
    #[cfg(target_vendor = "apple")]
    let elapsed = Duration::try_from(rustix::time::clock_gettime(
        rustix::time::ClockId::Monotonic,
    ))
    .ok();
    #[cfg(target_os = "windows")]
    let elapsed = Some(Duration::from_millis(winsafe::GetTickCount64()));
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "windows"
    )))]
    let elapsed: Option<Duration> = None;
    elapsed.and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
}

/// The deadline a receiver gives a clip from a duration it derived itself:
/// counted from the earlier of the producer's stamp and the local receipt, so
/// a slow local clock never keeps the clip longer than `duration_ms`.
#[must_use]
pub fn received_deadline(created_at: i64, duration_ms: i64, now_ms: i64) -> i64 {
    created_at.min(now_ms).saturating_add(duration_ms)
}

/// A synced deadline in local terms: no later than `duration_ms` after now.
#[must_use]
pub fn clamp_synced(expires_at: i64, duration_ms: Option<i64>, now_ms: i64) -> i64 {
    duration_ms.map_or(expires_at, |duration| {
        expires_at.min(now_ms.saturating_add(duration))
    })
}

/// The synced expiry after `incoming` is applied over `local`: a keep clears
/// it, otherwise the earlier deadline wins. `local` is empty when kept.
#[must_use]
pub fn merge_synced(
    local: SyncedExpiry,
    incoming: SyncedExpiry,
    incoming_keep: bool,
) -> SyncedExpiry {
    if incoming_keep {
        return SyncedExpiry::default();
    }
    match (local.expires_at, incoming.expires_at) {
        (Some(local_at), Some(incoming_at)) if local_at < incoming_at => local,
        (Some(_), None) => local,
        _ => incoming,
    }
}

/// The boot-clock reading at which a wall-clock deadline falls due.
#[must_use]
pub fn monotonic_deadline(deadline: i64, clocks: Clocks) -> Option<i64> {
    clocks
        .boot_ms
        .map(|boot| boot.saturating_add(deadline.saturating_sub(clocks.wall_ms).max(0)))
}

/// The local deadline a received version gets from its clip's synced note,
/// clamped to its duration from now. Pinned and kept clips get none.
pub(crate) fn receipt_deadline(
    tx: &Connection,
    incoming: &IncomingItem<'_>,
    now_ms: i64,
) -> Result<Option<i64>, StoreError> {
    if incoming.deleted || incoming.pinned {
        return Ok(None);
    }
    Ok(synced(tx, incoming.id)?
        .map(|(expires_at, duration)| clamp_synced(expires_at, duration, now_ms)))
}

/// Whether a received version is already past its synced deadline. Such a
/// version is discarded without persisting its plaintext, and its id is
/// recorded so later re-arrivals are dropped too.
pub(crate) fn arrived_expired(
    tx: &Connection,
    incoming: &IncomingItem<'_>,
    now_ms: i64,
) -> Result<bool, StoreError> {
    if incoming.deleted || incoming.pinned {
        return Ok(false);
    }
    let expired = synced(tx, incoming.id)?.is_some_and(|(expires_at, _)| expires_at <= now_ms);
    if expired {
        record_expired(tx, incoming.id, now_ms)?;
    }
    Ok(expired)
}

/// Applies the synced deadline to a clip version that was just stored.
pub(crate) fn after_received(
    tx: &Connection,
    incoming: &IncomingItem<'_>,
) -> Result<(), StoreError> {
    let now = crate::now_ms();
    if let Some(deadline) = receipt_deadline(tx, incoming, now)? {
        observe_deadline(tx, incoming.id, deadline, None, 0, Clocks::at(now))?;
    }
    Ok(())
}

/// Follows a clip note that was just projected: a keep clears the local
/// expiry, a synced deadline lowers it.
pub(crate) fn note_applied(tx: &Connection, note: &ClipNoteRecord) -> Result<(), StoreError> {
    if note.keep {
        return clear_expiry(tx, &note.clip_id);
    }
    observe_synced(tx, &note.clip_id, Clocks::now())
}

/// Lowers a live, unpinned, unkept clip's local deadline to `deadline` and
/// adds `flags`. Answers the resulting deadline, or `None` when exempt.
pub(crate) fn observe_deadline(
    tx: &Connection,
    clip_id: &str,
    deadline: i64,
    duration_ms: Option<i64>,
    flags: i64,
    clocks: Clocks,
) -> Result<Option<i64>, StoreError> {
    let clip: Option<(i64, bool)> = tx
        .query_row(
            "SELECT created_at, pinned FROM clipboard_items WHERE id = ?1 AND deleted = 0",
            [clip_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((created_at, pinned)) = clip else {
        return Ok(None);
    };
    if pinned || kept(tx, clip_id)? {
        return Ok(None);
    }
    let mut state = local_state::read(tx, clip_id)?.unwrap_or_else(|| ClipLocalState {
        clip_id: clip_id.to_owned(),
        ..Default::default()
    });
    if state.expires_at.is_none_or(|current| deadline < current) {
        state.expires_at = Some(deadline);
        state.expiry_duration_ms = duration_ms.or(state.expiry_duration_ms);
    }
    let at = state.expires_at.unwrap_or(deadline);
    state.expiry_mono_ms = state
        .expiry_mono_ms
        .into_iter()
        .chain(monotonic_deadline(at, clocks))
        .min();
    state.eval_flags |= flags;
    local_state::write(tx, &state)?;
    if at.saturating_sub(created_at) <= CLOUD_WITHHOLD_MS {
        withhold(tx, clip_id, clocks.wall_ms)?;
    }
    Ok(Some(at))
}

/// Sets an expiry as the device that purges with a tombstone, and syncs the
/// deadline in the clip's note. `None` when the clip is gone, pinned or kept.
pub(crate) fn set_owned_expiry(
    tx: &Connection,
    key: &crate::ItemKey,
    clip_id: &str,
    duration_ms: i64,
    clear_clipboard: bool,
    clocks: Clocks,
) -> Result<Option<i64>, StoreError> {
    let flags = EXPIRY_OWNER
        | if clear_clipboard {
            CLEAR_CLIPBOARD_AT_EXPIRY
        } else {
            0
        };
    let deadline = clocks.wall_ms.saturating_add(duration_ms.max(0));
    let Some(at) = observe_deadline(tx, clip_id, deadline, Some(duration_ms), flags, clocks)?
    else {
        return Ok(None);
    };
    let mut note = editable_note(tx, clip_id)?;
    let merged = merge_synced(
        SyncedExpiry {
            expires_at: note.expires_at,
            duration_ms: note.expiry_duration_ms,
        },
        SyncedExpiry {
            expires_at: Some(at),
            duration_ms: Some(duration_ms),
        },
        false,
    );
    note.expires_at = merged.expires_at;
    note.expiry_duration_ms = merged.duration_ms;
    organization::write(tx, key, Record::ClipNote(note))?;
    Ok(Some(at))
}

/// Keeps a clip: a synced keep note, no local expiry, no cloud withholding.
/// With `only_if_expiring` a clip with no expiry is left untouched. `None`
/// when the clip is not live.
pub(crate) fn keep_in_tx(
    tx: &Connection,
    key: &crate::ItemKey,
    clip_id: &str,
    only_if_expiring: bool,
) -> Result<Option<KeepOutcome>, StoreError> {
    let live: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM clipboard_items WHERE id = ?1 AND deleted = 0)",
        [clip_id],
        |row| row.get(0),
    )?;
    if !live {
        return Ok(None);
    }
    let mut note = editable_note(tx, clip_id)?;
    let local_expiry = local_state::read(tx, clip_id)?.and_then(|state| state.expires_at);
    let withheld = withheld_ids(tx, &[clip_id.to_owned()])?.contains(clip_id);
    if only_if_expiring && local_expiry.is_none() && note.expires_at.is_none() && !withheld {
        return Ok(Some(KeepOutcome::default()));
    }
    if !note.keep {
        note.keep = true;
        note.expires_at = None;
        note.expiry_duration_ms = None;
        organization::write(tx, key, Record::ClipNote(note))?;
    }
    clear_expiry(tx, clip_id)?;
    Ok(Some(KeepOutcome {
        kept: true,
        reoffer: if withheld {
            vec![clip_id.to_owned()]
        } else {
            Vec::new()
        },
    }))
}

/// Clears a clip's local expiry and its cloud withholding.
fn clear_expiry(tx: &Connection, clip_id: &str) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE clip_local_state SET expires_at = NULL, expiry_duration_ms = NULL, \
         expiry_mono_ms = NULL, eval_flags = eval_flags & ~?2 WHERE clip_id = ?1",
        params![clip_id, EXPIRY_OWNER | CLEAR_CLIPBOARD_AT_EXPIRY],
    )?;
    unwithhold(tx, clip_id)
}

fn unwithhold(tx: &Connection, clip_id: &str) -> Result<(), StoreError> {
    let mut delete = tx.prepare_cached("DELETE FROM cloud_withheld WHERE clip_id = ?1")?;
    delete.execute([clip_id])?;
    delete.execute([organization::clip_note_record_id(clip_id)])?;
    Ok(())
}

/// The clip's note as the base of a local edit, or a fresh one.
fn editable_note(tx: &Connection, clip_id: &str) -> Result<ClipNoteRecord, StoreError> {
    let stamp: Option<i64> = tx
        .query_row(
            "SELECT created_at FROM organization_records WHERE id = ?1 AND deleted = 0",
            [organization::clip_note_record_id(clip_id)],
            |row| row.get(0),
        )
        .optional()?;
    let note = match organization::read(tx, RecordKey::ClipNote(clip_id))? {
        Some(Record::ClipNote(note)) => Some(note),
        _ => None,
    };
    Ok(ClipNoteRecord {
        base_stamp: stamp.unwrap_or(0),
        ..note.unwrap_or(ClipNoteRecord {
            clip_id: clip_id.to_owned(),
            hidden_from_ai: false,
            hidden_changed_at: 0,
            base_stamp: 0,
            keep: false,
            expires_at: None,
            expiry_duration_ms: None,
            provenance: None,
        })
    })
}

fn observe_synced(tx: &Connection, clip_id: &str, clocks: Clocks) -> Result<(), StoreError> {
    if let Some((expires_at, duration)) = synced(tx, clip_id)? {
        let deadline = clamp_synced(expires_at, duration, clocks.wall_ms);
        observe_deadline(tx, clip_id, deadline, duration, 0, clocks)?;
    }
    Ok(())
}

/// The synced deadline of a clip, from its note, unless the clip is kept or
/// pinned here.
fn synced(tx: &Connection, clip_id: &str) -> Result<Option<(i64, Option<i64>)>, StoreError> {
    Ok(tx
        .prepare_cached(
            "SELECT expires_at, expiry_duration_ms FROM clip_notes \
             WHERE clip_id = ?1 AND keep = 0 AND expires_at IS NOT NULL AND NOT EXISTS \
             (SELECT 1 FROM clipboard_items WHERE id = ?1 AND deleted = 0 AND pinned = 1)",
        )?
        .query_row([clip_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?)
}

fn kept(tx: &Connection, clip_id: &str) -> Result<bool, StoreError> {
    Ok(tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM clip_notes WHERE clip_id = ?1 AND keep = 1)",
        [clip_id],
        |row| row.get(0),
    )?)
}

/// Keeps the clip, and its note, out of every cloud upload.
fn withhold(tx: &Connection, clip_id: &str, now_ms: i64) -> Result<(), StoreError> {
    let mut insert =
        tx.prepare_cached("INSERT OR IGNORE INTO cloud_withheld(clip_id, since) VALUES (?1, ?2)")?;
    insert.execute(params![clip_id, now_ms])?;
    insert.execute(params![organization::clip_note_record_id(clip_id), now_ms])?;
    Ok(())
}

fn record_expired(tx: &Connection, clip_id: &str, now_ms: i64) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO expired_ids(clip_id, expired_at) VALUES (?1, ?2) \
         ON CONFLICT(clip_id) DO UPDATE SET expired_at = excluded.expired_at",
        params![clip_id, now_ms],
    )?;
    Ok(())
}

fn withheld_ids(conn: &Connection, ids: &[String]) -> Result<HashSet<String>, StoreError> {
    if ids.is_empty() {
        return Ok(HashSet::new());
    }
    let ids = serde_json::to_string(ids).map_err(|_| StoreError::InvalidHistoryQuery)?;
    let mut statement = conn.prepare_cached(
        "SELECT clip_id FROM cloud_withheld WHERE clip_id IN (SELECT value FROM json_each(?1))",
    )?;
    let found = statement
        .query_map([ids], |row| row.get(0))?
        .collect::<rusqlite::Result<HashSet<String>>>()?;
    Ok(found)
}

/// Removes one expired clip. The setting device tombstones it; any other
/// device deletes its row outright and records the id.
fn purge(tx: &Connection, clip_id: &str, owner: bool, now_ms: i64) -> Result<(), StoreError> {
    if owner {
        super::items::tombstone_in_tx(tx, clip_id, now_ms)?;
        return Ok(());
    }
    delete_fts_row_in_tx(tx, clip_id)?;
    tx.execute("DELETE FROM clipboard_items WHERE id = ?1", [clip_id])?;
    record_expired(tx, clip_id, now_ms)?;
    unwithhold(tx, clip_id)
}

impl Store {
    /// [`set_owned_expiry`] in its own transaction.
    pub fn set_clip_expiry(
        &self,
        key: &crate::ItemKey,
        clip_id: &str,
        duration_ms: i64,
        clear_clipboard: bool,
        clocks: Clocks,
    ) -> Result<Option<i64>, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let at = set_owned_expiry(&tx, key, clip_id, duration_ms, clear_clipboard, clocks)?;
        tx.commit()?;
        Ok(at)
    }

    /// [`keep_in_tx`] in its own transaction.
    pub fn keep_clip(
        &self,
        key: &crate::ItemKey,
        clip_id: &str,
        only_if_expiring: bool,
    ) -> Result<Option<KeepOutcome>, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let outcome = keep_in_tx(&tx, key, clip_id, only_if_expiring)?;
        tx.commit()?;
        Ok(outcome)
    }

    /// The ids among `ids` that cloud uploads must skip, tombstones included.
    pub fn cloud_withheld_among(&self, ids: &[String]) -> Result<HashSet<String>, StoreError> {
        withheld_ids(&*self.conn()?, ids)
    }

    /// Purges every clip whose deadline either clock says has passed, with
    /// pin and keep checked again, and drops withholding records whose row is
    /// gone for good.
    pub fn expire_due(&self, clocks: Clocks) -> Result<ExpirySweep, StoreError> {
        let mut sweep = ExpirySweep::default();
        loop {
            let mut conn = self.conn()?;
            let tx = write_tx(&mut conn)?;
            let due: Vec<(String, i64)> = tx
                .prepare_cached(
                    "SELECT ci.id, COALESCE(s.eval_flags, 0) FROM clipboard_items ci \
                     LEFT JOIN clip_local_state s ON s.clip_id = ci.id \
                     WHERE ci.id IN (\
                       SELECT clip_id FROM clip_local_state WHERE expires_at <= ?1 \
                       UNION SELECT clip_id FROM clip_local_state WHERE expiry_mono_ms <= ?2 \
                       UNION SELECT clip_id FROM clip_notes WHERE keep = 0 AND expires_at <= ?1) \
                     AND ci.deleted = 0 AND ci.pinned = 0 \
                     AND NOT EXISTS (SELECT 1 FROM clip_notes n WHERE n.clip_id = ci.id AND n.keep = 1) \
                     LIMIT ?3",
                )?
                .query_map(params![clocks.wall_ms, clocks.boot_ms, PURGE_BATCH], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            for (clip_id, flags) in &due {
                let owner = flags & EXPIRY_OWNER != 0;
                purge(&tx, clip_id, owner, clocks.wall_ms)?;
                if flags & CLEAR_CLIPBOARD_AT_EXPIRY != 0 {
                    sweep.clear_clipboard.push(clip_id.clone());
                }
                if owner {
                    sweep.tombstoned.push(clip_id.clone());
                } else {
                    sweep.purged_locally += 1;
                }
            }
            if (due.len() as i64) < PURGE_BATCH {
                tx.execute(
                    "DELETE FROM cloud_withheld WHERE \
                     NOT EXISTS (SELECT 1 FROM clipboard_items i WHERE i.id = cloud_withheld.clip_id) \
                     AND NOT EXISTS (SELECT 1 FROM organization_records r \
                       WHERE r.id = cloud_withheld.clip_id)",
                    [],
                )?;
                tx.commit()?;
                break;
            }
            tx.commit()?;
        }
        if !sweep.tombstoned.is_empty() || sweep.purged_locally > 0 {
            self.request_space_reclaim();
        }
        Ok(sweep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60 * 1000;

    #[test]
    fn a_synced_deadline_keeps_the_earlier_value_unless_kept() {
        let early = SyncedExpiry {
            expires_at: Some(10),
            duration_ms: Some(5),
        };
        let late = SyncedExpiry {
            expires_at: Some(20),
            duration_ms: Some(15),
        };
        assert_eq!(merge_synced(early, late, false), early);
        assert_eq!(merge_synced(late, early, false), early);
        assert_eq!(merge_synced(early, SyncedExpiry::default(), false), early);
        assert_eq!(merge_synced(SyncedExpiry::default(), late, false), late);
        assert_eq!(merge_synced(early, late, true), SyncedExpiry::default());
    }

    #[test]
    fn the_monotonic_deadline_counts_from_the_boot_clock() {
        let clocks = Clocks {
            wall_ms: 1_000,
            boot_ms: Some(50),
        };
        assert_eq!(monotonic_deadline(1_500, clocks), Some(550));
        assert_eq!(monotonic_deadline(900, clocks), Some(50));
        assert_eq!(
            monotonic_deadline(
                1_500,
                Clocks {
                    boot_ms: None,
                    ..clocks
                }
            ),
            None
        );
        assert!(boot_clock_ms().is_some_and(|ms| ms >= 0));
    }

    #[test]
    fn a_receiver_with_a_slow_clock_keeps_a_clip_no_longer_than_its_duration() {
        // The producer stamps at P with a 15 min expiry; the receiver's clock
        // reads 10 min behind when the clip arrives at once.
        let produced = 1_700_000_000_000;
        let received = produced - 10 * MIN;
        let synced = clamp_synced(produced + 15 * MIN, Some(15 * MIN), received);
        assert_eq!(synced - received, 15 * MIN);
        assert_eq!(
            received_deadline(produced, 15 * MIN, received) - received,
            15 * MIN
        );
        // A fast receiver clock may only shorten it.
        let fast = produced + 3 * MIN;
        assert_eq!(
            clamp_synced(produced + 15 * MIN, Some(15 * MIN), fast),
            produced + 15 * MIN
        );
        assert_eq!(
            received_deadline(produced, 15 * MIN, fast),
            produced + 15 * MIN
        );
    }
}
