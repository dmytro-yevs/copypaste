//! The history as a sync session sees it: every version, tombstones included,
//! and the last-write-wins write that stores the one that won.
//!
//! Two rules the transports depend on and cannot check for themselves:
//!
//! * **[`Store::summaries`] and [`Store::versions`] never return a sensitive
//!   item.** That is what makes "a sensitive item never leaves the device" a
//!   property of the protocol rather than a promise: a session serves only ids
//!   it advertised, so an item outside the summary list cannot be requested out
//!   of this device.
//! * **A tombstone is a version.** It keeps its item's `content_hash` and its
//!   `created_at`, which is what lets a delete tie the row it deletes on merge
//!   keys 1 and 2 and win on key 3.

use std::collections::HashSet;

use rusqlite::{params, OptionalExtension};

use super::connection::write_tx;
use super::model::{
    is_constraint_violation, item_columns, row_to_item, ItemColumns, StoreError, StoredItem,
};
use super::search::{delete_fts_row_in_tx, insert_fts_in_tx};
use super::store::Store;

/// The version identity of one row — the four merge keys and the id.
///
/// Lean on purpose: a session advertises up to
/// `copypaste_p2p::protocol::MAX_SUMMARIES_PER_MESSAGE` of these, and reading
/// every ciphertext to compare four scalars would hold the whole history in
/// memory to answer "what has changed".
#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    pub id: String,
    pub created_at: i64,
    pub content_hash: String,
    pub deleted: bool,
    pub pinned: bool,
    pub pin_order: Option<f64>,
    pub pin_updated_at: i64,
    /// Empty means "captured on this device" — see [`origin_or`].
    pub origin_device_id: String,
    /// Read by the merge for the tombstone case: an incoming delete must not
    /// clear the flag, or the item is re-indexed and the secret becomes
    /// searchable.
    pub is_sensitive: bool,
}

/// One version arriving from another device, already sealed under the local key.
pub struct IncomingItem<'a> {
    pub id: &'a str,
    /// `None` on a tombstone: the soft delete wipes the payload.
    pub content_ciphertext: Option<&'a [u8]>,
    pub nonce: Option<&'a [u8]>,
    pub content_type: &'a str,
    pub content_hash: &'a str,
    pub created_at: i64,
    pub deleted: bool,
    pub is_sensitive: bool,
    pub origin_device_id: &'a str,
    /// P2P pin state. `None` is the explicit unpinned order.
    pub pinned: bool,
    pub pin_order: Option<f64>,
    pub pin_updated_at: i64,
    /// Plaintext for the search index. Ignored when the item is sensitive or a
    /// tombstone — the write-time layer of "sensitive items are never indexed".
    pub search_text: Option<&'a str>,
    pub payload_metadata: Option<&'a str>,
}

/// The origin of a row, resolving the empty sentinel to `here`.
///
/// A locally captured item stores no origin: the alternative is writing this
/// device's own id onto every capture, which costs a column of duplicated UUIDs
/// and an extra argument on the ingest path for a value every reader already
/// knows. Every reader that compares or displays an origin must go through
/// this, or two rows that are in fact from the same device compare unequal on
/// merge key 4.
#[must_use]
pub fn origin_or<'a>(stored: &'a str, here: &'a str) -> &'a str {
    if stored.is_empty() {
        here
    } else {
        stored
    }
}

/// The projection behind [`Version`], in the order [`row_to_version`] reads and
/// in the order `idx_items_syncable` carries, so the covering read and the
/// index stay recognisably one thing.
macro_rules! version_columns {
    () => {
        "created_at, id, content_hash, deleted, origin_device_id, \
         pinned, pin_order, pin_updated_at, is_sensitive"
    };
}

fn row_to_version(row: &rusqlite::Row<'_>) -> rusqlite::Result<Version> {
    Ok(Version {
        created_at: row.get(0)?,
        id: row.get(1)?,
        content_hash: row.get(2)?,
        deleted: row.get::<_, i64>(3)? != 0,
        origin_device_id: row.get(4)?,
        pinned: row.get(5)?,
        pin_order: row.get(6)?,
        pin_updated_at: row.get(7)?,
        is_sensitive: row.get(8)?,
    })
}

impl Store {
    /// Apply a P2P pin-state version without touching the content version.
    ///
    /// Cloud rows deliberately have no pin metadata. Keeping this write
    /// separate prevents a local pin from re-uploading older content.
    pub fn apply_pin_state(
        &self,
        id: &str,
        pinned: bool,
        pin_order: Option<f64>,
        pin_updated_at: i64,
    ) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let written = tx.execute(
            "UPDATE clipboard_items \
                SET pinned = ?2, pin_order = ?3, pin_updated_at = ?4 \
              WHERE id = ?1 AND deleted = 0",
            params![id, pinned, pin_order, pin_updated_at],
        )?;
        tx.commit()?;
        Ok(written != 0)
    }

    /// Everything eligible to sync, newest first, tombstones included.
    ///
    /// Live sensitive items are excluded here and nowhere else matters more.
    /// Their payload-less tombstones remain eligible so auto-wipe converges on
    /// peers without disclosing the secret.
    pub fn summaries(&self, limit: i64) -> Result<Vec<Version>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            version_columns!(),
            " FROM clipboard_items \
              WHERE deleted = 1 OR is_sensitive = 0 \
              ORDER BY created_at DESC, id DESC LIMIT ?1"
        ))?;
        let rows = stmt.query_map([limit], row_to_version)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The same view, from a cursor: everything whose version *or pin state*
    /// moved at or after `since_ms`, oldest change first.
    ///
    /// Ordered and bounded on `MAX(created_at, pin_updated_at)`, not on
    /// `created_at`. Pinning does not restamp the content version — that is the
    /// whole point of `pin_updated_at` — so a `created_at` cursor would stop
    /// carrying pin and unpin for any item older than it, permanently.
    ///
    /// `after_id` breaks the tie the way [`Store::versions_after`] does: a
    /// stamp-only floor cannot drain a page of rows that share its boundary.
    pub fn summaries_since(
        &self,
        since_ms: i64,
        after_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Version>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            version_columns!(),
            " FROM clipboard_items \
               WHERE (deleted = 1 OR is_sensitive = 0) \
                 AND (MAX(created_at, pin_updated_at) > ?1 \
                      OR (MAX(created_at, pin_updated_at) = ?1 \
                          AND (?2 IS NULL OR id > ?2))) \
               ORDER BY MAX(created_at, pin_updated_at) ASC, id ASC LIMIT ?3"
        ))?;
        let rows = stmt.query_map(params![since_ms, after_id, limit], row_to_version)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// One row's version identity, without its payload.
    ///
    /// [`Store::version`] for a caller that only compares metadata: the merge
    /// reads eight scalars and the full projection makes it read and copy the
    /// ciphertext BLOB to do it. Sensitive rows are included for the same
    /// reason they are in [`Store::version`] — the merge has to compare against
    /// one — and `is_sensitive` is part of the answer, not filtered out of it.
    pub fn version_summary(&self, id: &str) -> Result<Option<Version>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            version_columns!(),
            " FROM clipboard_items WHERE id = ?1"
        ))?;
        Ok(stmt.query_row([id], row_to_version).optional()?)
    }

    /// One row as the merge sees it — live, tombstoned, or sensitive.
    ///
    /// Sensitive rows are *included*, unlike everywhere else: an incoming
    /// version has to be compared against one, or a peer's copy of something
    /// this device flagged would be stored a second time under the same id.
    pub fn version(&self, id: &str) -> Result<Option<StoredItem>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            " FROM clipboard_items WHERE id = ?1"
        ))?;
        let columns = ItemColumns::resolve(&stmt)?;
        Ok(stmt
            .query_row([id], |row| row_to_item(row, &columns))
            .optional()?)
    }

    /// The rows behind a request, still encrypted. Live sensitive items and
    /// unknown ids are omitted; sensitive tombstones are included because they
    /// carry no payload and are the delete protocol's only data.
    pub fn versions(&self, ids: &[String]) -> Result<Vec<StoredItem>, StoreError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // Duplicates in the request would otherwise duplicate the reply.
        let unique: Vec<&String> = {
            let mut seen = HashSet::with_capacity(ids.len());
            ids.iter().filter(|id| seen.insert(id.as_str())).collect()
        };
        let conn = self.conn()?;
        // One statement per batch size rather than a bound-parameter loop: the
        // batch is at most `MAX_ITEMS_PER_MESSAGE`.
        let placeholders = std::iter::repeat_n("?", unique.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT {} FROM clipboard_items \
              WHERE (deleted = 1 OR is_sensitive = 0) AND id IN ({placeholders})",
            item_columns!()
        );
        let mut stmt = conn.prepare(&sql)?;
        let columns = ItemColumns::resolve(&stmt)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(unique), |row| {
            row_to_item(row, &columns)
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Every version stamped at or after `since_ms`, oldest first, sensitive
    /// items excluded.
    ///
    /// `>=` rather than `>` because the cursor is inclusive — re-sending the
    /// boundary row costs one idempotent upsert, and excluding it loses every
    /// row that shares the boundary millisecond.
    ///
    /// Note that a tombstone keeps the *item's* `created_at`
    /// ([`Store::delete`] does not restamp), so deleting an item older than the
    /// cursor produces a version this query cannot see. Callers pull their
    /// cursor back rather than this query widening.
    pub fn versions_since(&self, since_ms: i64, limit: i64) -> Result<Vec<StoredItem>, StoreError> {
        self.versions_after(since_ms, None, limit)
    }

    /// Every version after the inclusive `created_at` boundary or, when an id
    /// is present, strictly after the compound `(created_at, id)` keyset.
    ///
    /// The upload cursor needs the same tie-break as the download cursor. A
    /// millisecond-only floor cannot drain a full page of rows that share its
    /// boundary stamp: the next scan reads that first page again forever.
    pub fn versions_after(
        &self,
        since_ms: i64,
        after_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<StoredItem>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            " FROM clipboard_items \
               WHERE (deleted = 1 OR is_sensitive = 0) \
                 AND (created_at > ?1 \
                      OR (created_at = ?1 AND (?2 IS NULL OR id > ?2))) \
               ORDER BY created_at ASC, id ASC LIMIT ?3"
        ))?;
        let columns = ItemColumns::resolve(&stmt)?;
        let rows = stmt.query_map(params![since_ms, after_id, limit], |row| {
            row_to_item(row, &columns)
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The stamp of the oldest version this device would ever offer, if any.
    ///
    /// Used to reset a sync cursor to "everything": a bulk delete has to
    /// re-offer versions the cursor has already passed, and this is the bound
    /// on how far back that goes.
    pub fn oldest_version_ms(&self) -> Result<Option<i64>, StoreError> {
        let conn = self.conn()?;
        let oldest: Option<i64> = conn.query_row(
            "SELECT MIN(created_at) FROM clipboard_items WHERE deleted = 1 OR is_sensitive = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(oldest)
    }

    /// Store the version a session decided should win.
    ///
    /// The one write the insert-only API cannot express, and the only path that
    /// may overwrite an existing row. `Ok(false)` means the store refused it
    /// because the dedup index already holds a different id from the same
    /// origin with this content in the same bucket. Different origins remain
    /// separate items: collapsing them would leave replicas with incompatible
    /// item ids and make later deletes diverge.
    ///
    /// P2P carries pin state as ordinary version metadata. The incoming value
    /// replaces the local one wholesale, including an explicit `NULL` order on
    /// unpin, because OR-merging pin state leaves replicas permanently apart.
    pub fn upsert(&self, incoming: &IncomingItem<'_>) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        // Before the write below clears `fts_rowid`, and rolled back with it if
        // the dedup index refuses the version.
        delete_fts_row_in_tx(&tx, incoming.id)?;
        let (pinned, pin_order, pin_updated_at) = if incoming.deleted {
            (false, None, 0)
        } else {
            (incoming.pinned, incoming.pin_order, incoming.pin_updated_at)
        };

        let indexable = incoming
            .search_text
            .filter(|t| !incoming.deleted && !t.trim().is_empty());
        let fts_rowid = match indexable {
            Some(text) => insert_fts_in_tx(
                &tx,
                incoming.id,
                text,
                incoming.is_sensitive,
                incoming.content_type,
            )?,
            None => None,
        };

        let written = tx.execute(
            "INSERT INTO clipboard_items \
                 (id, content_ciphertext, nonce, content_type, content_hash, \
                  is_sensitive, pinned, pin_order, pin_updated_at, created_at, deleted, origin_device_id, payload_metadata, \
                  fts_rowid, content_bytes) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, \
                     LENGTH(COALESCE(?2, X''))) \
             ON CONFLICT(id) DO UPDATE SET \
                 content_ciphertext = excluded.content_ciphertext, \
                 content_bytes      = excluded.content_bytes, \
                 nonce              = excluded.nonce, \
                 content_type       = excluded.content_type, \
                 content_hash       = excluded.content_hash, \
                 is_sensitive       = excluded.is_sensitive, \
                 created_at         = excluded.created_at, \
                 deleted            = excluded.deleted, \
                 origin_device_id   = excluded.origin_device_id, \
                 app_bundle_id      = NULL, \
                 payload_metadata   = excluded.payload_metadata, \
                 pinned             = excluded.pinned, \
                 pin_order          = excluded.pin_order, \
                 pin_updated_at     = excluded.pin_updated_at, \
                 fts_rowid          = excluded.fts_rowid",
            params![
                incoming.id,
                incoming.content_ciphertext,
                incoming.nonce,
                incoming.content_type,
                incoming.content_hash,
                incoming.is_sensitive,
                pinned,
                pin_order,
                pin_updated_at,
                incoming.created_at,
                incoming.deleted,
                incoming.origin_device_id,
                incoming.payload_metadata,
                fts_rowid,
            ],
        );

        match written {
            Ok(_) => {}
            Err(e) if is_constraint_violation(&e) => {
                tracing::warn!(
                    "an incoming item collides with the dedup index under another id; skipping it"
                );
                return Ok(false);
            }
            Err(e) => return Err(e.into()),
        }

        tx.commit()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{fts_row_count, item, plan_of, sensitive_item, store, T0};

    fn incoming<'a>(id: &'a str, hash: &'a str, created_at: i64) -> IncomingItem<'a> {
        IncomingItem {
            id,
            content_ciphertext: Some(&[9, 9, 9]),
            nonce: Some(&[8, 8, 8]),
            content_type: "text",
            content_hash: hash,
            created_at,
            deleted: false,
            is_sensitive: false,
            origin_device_id: "device-a",
            pinned: false,
            pin_order: None,
            pin_updated_at: 0,
            search_text: Some("remote text"),
            payload_metadata: None,
        }
    }

    #[test]
    fn summaries_exclude_sensitive_items_and_include_tombstones() {
        let s = store();
        let plain = s.insert(item("plain", T0)).unwrap();
        let secret = s
            .insert(sensitive_item("AKIA-shaped", T0 + 60_000))
            .unwrap();
        let gone = s.insert(item("gone", T0 + 120_000)).unwrap();
        assert!(s.delete(&gone.id).unwrap());

        let summaries = s.summaries(100).unwrap();
        let ids: Vec<&str> = summaries.iter().map(|v| v.id.as_str()).collect();
        assert!(ids.contains(&plain.id.as_str()), "{ids:?}");
        assert!(
            ids.contains(&gone.id.as_str()),
            "a tombstone is a version, not an absence"
        );
        assert!(
            !ids.contains(&secret.id.as_str()),
            "a sensitive item must never be advertised"
        );

        let tombstone = summaries.iter().find(|v| v.id == gone.id).unwrap();
        assert!(tombstone.deleted);
        assert_eq!(
            tombstone.content_hash, gone.content_hash,
            "a tombstone keeps the hash of what it deleted"
        );
    }

    #[test]
    fn versions_omit_sensitive_and_unknown_ids_and_never_duplicate() {
        let s = store();
        let plain = s.insert(item("plain", T0)).unwrap();
        let secret = s.insert(sensitive_item("secret", T0 + 60_000)).unwrap();

        let rows = s
            .versions(&[
                plain.id.clone(),
                plain.id.clone(),
                secret.id,
                "never-existed".to_string(),
            ])
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, plain.id);
        assert_eq!(rows[0].content_ciphertext, b"ct:plain");
        assert!(s.versions(&[]).unwrap().is_empty());
    }

    #[test]
    fn a_sensitive_tombstone_is_advertised_and_served_without_a_payload() {
        let s = store();
        let secret = s.insert(sensitive_item("secret", T0)).unwrap();
        assert!(s.delete(&secret.id).unwrap());

        let summaries = s.summaries(100).unwrap();
        assert!(summaries
            .iter()
            .any(|item| item.id == secret.id && item.deleted));

        let rows = s.versions(std::slice::from_ref(&secret.id)).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].deleted);
        assert!(rows[0].content_ciphertext.is_empty());
        assert!(rows[0].nonce.is_empty());
        assert!(rows[0].content_hash.is_empty());
    }

    /// `version` is the only read that returns a tombstone or a flagged row,
    /// because the merge has to compare against both.
    #[test]
    fn version_returns_tombstones_and_sensitive_rows() {
        let s = store();
        let secret = s.insert(sensitive_item("secret", T0)).unwrap();
        let gone = s.insert(item("gone", T0 + 60_000)).unwrap();
        s.delete(&gone.id).unwrap();

        assert!(s.version(&secret.id).unwrap().unwrap().is_sensitive);
        let tombstone = s.version(&gone.id).unwrap().unwrap();
        assert!(tombstone.deleted);
        assert!(tombstone.content_ciphertext.is_empty());
        assert!(s.version("never-existed").unwrap().is_none());
    }

    #[test]
    fn an_item_with_no_recorded_origin_reads_as_this_device() {
        let s = store();
        let stored = s.insert(item("mine", T0)).unwrap();
        assert_eq!(stored.origin_device_id, "");
        assert_eq!(
            origin_or(&stored.origin_device_id, "device-here"),
            "device-here"
        );
        assert_eq!(origin_or("device-b", "device-here"), "device-b");
    }

    #[test]
    fn an_upsert_stores_an_unknown_item_with_its_remote_identity() {
        let s = store();
        assert!(s
            .upsert(&incoming("from-peer", "hash-remote", 5_000))
            .unwrap());

        let stored = s.get("from-peer").unwrap().expect("stored");
        assert_eq!(stored.created_at, 5_000);
        assert_eq!(stored.content_hash, "hash-remote");
        assert_eq!(stored.origin_device_id, "device-a");
        assert!(!s.search("remote", 10).unwrap().is_empty());
    }

    #[test]
    fn an_upsert_of_a_sensitive_version_keeps_it_out_of_the_index() {
        let s = store();
        s.upsert(&IncomingItem {
            is_sensitive: true,
            // Even when a caller supplies it, as the store's own layer does.
            search_text: Some("AKIAIOSFODNN7EXAMPLE"),
            ..incoming("flagged", "hash-flagged", 5_000)
        })
        .unwrap();

        assert!(s.search("AKIAIOSFODNN7EXAMPLE", 10).unwrap().is_empty());
        assert!(s.version("flagged").unwrap().unwrap().is_sensitive);
    }

    #[test]
    fn an_upsert_of_a_tombstone_wipes_the_row_and_unindexes_it() {
        let s = store();
        let doomed = s.insert(item("doomed", T0)).unwrap();
        s.set_pinned(&doomed.id, true).unwrap();

        s.upsert(&IncomingItem {
            content_ciphertext: None,
            nonce: None,
            deleted: true,
            search_text: None,
            ..incoming(&doomed.id, &doomed.content_hash, T0 + 1)
        })
        .unwrap();

        assert!(s.get(&doomed.id).unwrap().is_none(), "still live");
        assert!(s.search("doomed", 10).unwrap().is_empty());
        let row = s.version(&doomed.id).unwrap().unwrap();
        assert!(row.deleted);
        assert!(!row.pinned, "a tombstone has nothing to be pinned to");
    }

    #[test]
    fn an_upsert_counts_tombstoning_and_resurrection() {
        let s = store();
        let row = s.insert(item("contested", T0)).unwrap();

        let tombstone = IncomingItem {
            content_ciphertext: None,
            nonce: None,
            deleted: true,
            search_text: None,
            ..incoming(&row.id, &row.content_hash, T0 + 1)
        };
        s.upsert(&tombstone).unwrap();
        assert_eq!(s.count().unwrap(), 0);
        s.upsert(&tombstone).unwrap();
        assert_eq!(s.count().unwrap(), 0, "reapplying must not decrement twice");

        s.upsert(&incoming(&row.id, "newer-hash", T0 + 900_000))
            .unwrap();
        assert_eq!(s.count().unwrap(), 1);
    }

    #[test]
    fn an_incoming_version_replaces_pin_state() {
        let s = store();
        let kept = s.insert(item("kept", T0)).unwrap();
        s.set_pinned(&kept.id, true).unwrap();

        s.upsert(&incoming(&kept.id, "hash-newer", T0 + 900_000))
            .unwrap();
        let row = s.get(&kept.id).unwrap().unwrap();
        assert!(!row.pinned);
        assert_eq!(row.pin_order, None);
    }

    #[test]
    fn tombstones_clear_pin_metadata_even_if_the_caller_supplies_it() {
        let s = store();
        let doomed = s.insert(item("doomed", T0)).unwrap();
        s.upsert(&IncomingItem {
            deleted: true,
            pinned: true,
            pin_order: Some(99.0),
            pin_updated_at: T0 + 1,
            ..incoming(&doomed.id, "gone", T0 + 1)
        })
        .unwrap();
        let row = s.version(&doomed.id).unwrap().unwrap();
        assert!(row.deleted);
        assert!(!row.pinned);
        assert_eq!(row.pin_order, None);
        assert_eq!(row.pin_updated_at, 0);
    }

    #[test]
    fn a_collision_with_the_dedup_index_is_refused_not_an_error() {
        let s = store();
        let mine = s.insert(item("mine", T0)).unwrap();

        // Same content hash, same bucket, different id: the dedup index owns
        // that pair.
        let applied = s
            .upsert(&IncomingItem {
                origin_device_id: "",
                search_text: Some("leaking remote text"),
                ..incoming("theirs", &mine.content_hash, T0 + 500)
            })
            .unwrap();
        assert!(!applied, "the collision must be reported, not stored");
        assert!(s.get("theirs").unwrap().is_none());
        assert!(s.get(&mine.id).unwrap().is_some(), "no local data lost");
        assert!(s.search("leaking", 10).unwrap().is_empty());
        assert_eq!(fts_row_count(&s, "theirs"), 0);
        assert_eq!(fts_row_count(&s, &mine.id), 1);
    }

    #[test]
    fn versions_since_is_inclusive_and_oldest_first() {
        let s = store();
        let a = s.insert(item("a", T0)).unwrap();
        let b = s.insert(item("b", T0 + 60_000)).unwrap();
        s.insert(sensitive_item("secret", T0 + 120_000)).unwrap();

        let rows = s.versions_since(T0, 100).unwrap();
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, [a.id.as_str(), b.id.as_str()]);
        assert_eq!(s.versions_since(T0 + 60_000, 100).unwrap().len(), 1);
        assert_eq!(s.oldest_version_ms().unwrap(), Some(T0));
    }

    #[test]
    fn versions_after_drains_a_same_millisecond_boundary_by_id() {
        let s = store();
        let a = s.insert(item("a", T0)).unwrap();
        let b = s.insert(item("b", T0)).unwrap();
        let c = s.insert(item("c", T0)).unwrap();
        let mut ids = [a.id, b.id, c.id];
        ids.sort();

        let first = s.versions_after(T0, None, 2).unwrap();
        assert_eq!(first.len(), 2);
        let second = s.versions_after(T0, Some(&first[1].id), 2).unwrap();
        assert_eq!(
            second.iter().map(|row| &row.id).collect::<Vec<_>>(),
            [&ids[2]]
        );
    }

    #[test]
    fn an_empty_history_has_no_oldest_version() {
        assert_eq!(store().oldest_version_ms().unwrap(), None);
    }

    /// F-STOR-2. Without an index the sync read scans `clipboard_items` — whose
    /// rows are dominated by the ciphertext this projection discards — and then
    /// materialises a temp B-tree over every qualifying row. Measured 9.06 ms →
    /// 0.86 ms at 5 000 rows.
    ///
    /// The partial predicate has to be written exactly as `idx_items_syncable`
    /// declares it or SQLite silently declines the index (`CopyPaste-crh3.3`),
    /// and here that predicate is also what keeps a live sensitive item out of
    /// an advertisement — so this test guards a disclosure, not a stall.
    #[test]
    fn the_sync_read_is_served_by_its_covering_index_without_a_sort() {
        let s = store();
        s.insert(item("something", T0)).unwrap();
        let plan = plan_of(
            &s,
            concat!(
                "SELECT ",
                version_columns!(),
                " FROM clipboard_items \
                  WHERE deleted = 1 OR is_sensitive = 0 \
                  ORDER BY created_at DESC, id DESC LIMIT ?1"
            ),
        );
        assert!(
            plan.iter().any(|d| d.contains("idx_items_syncable")),
            "the sync read must use its index, got {plan:?}"
        );
        assert!(
            !plan.iter().any(|d| d.contains("TEMP B-TREE")),
            "the index must supply the order, got {plan:?}"
        );
    }

    #[test]
    fn the_sync_cursor_seeks_rather_than_scanning() {
        let s = store();
        s.insert(item("something", T0)).unwrap();
        let plan = plan_of(
            &s,
            concat!(
                "SELECT ",
                version_columns!(),
                " FROM clipboard_items \
                   WHERE (deleted = 1 OR is_sensitive = 0) \
                     AND (MAX(created_at, pin_updated_at) > ?1 \
                          OR (MAX(created_at, pin_updated_at) = ?1 \
                              AND (?2 IS NULL OR id > ?2))) \
                   ORDER BY MAX(created_at, pin_updated_at) ASC, id ASC LIMIT ?3"
            ),
        );
        assert!(
            plan.iter().any(|d| d.contains("idx_items_sync_cursor")),
            "the cursor must use its index, got {plan:?}"
        );
        assert!(
            !plan.iter().any(|d| d.contains("TEMP B-TREE")),
            "the index must supply the order, got {plan:?}"
        );
    }

    /// Manifest 05 §3.6. A pin does **not** restamp `created_at` — that is the
    /// entire reason `pin_updated_at` is a separate column — so a cursor over
    /// `created_at` alone would advertise the pin change to nobody, for ever.
    #[test]
    fn the_cursor_carries_a_pin_change_on_an_item_older_than_itself() {
        let s = store();
        let old = s.insert(item("pinned later", T0)).unwrap();
        s.insert(item("newer", T0 + 600_000)).unwrap();

        // Everything up to here is behind the reader.
        let cursor = T0 + 600_000;
        assert!(s.summaries_since(cursor + 1, None, 100).unwrap().is_empty());

        s.apply_pin_state(&old.id, true, Some(1.0), cursor + 5_000)
            .unwrap();
        let moved = s.summaries_since(cursor + 1, None, 100).unwrap();
        assert_eq!(
            moved.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            [old.id.as_str()],
            "a pin on an old item must cross the cursor"
        );
        assert!(moved[0].pinned);
        assert_eq!(moved[0].created_at, T0, "the content version must not move");
    }

    #[test]
    fn the_cursor_is_inclusive_ordered_and_drains_a_shared_boundary_by_id() {
        let s = store();
        let a = s.insert(item("a", T0)).unwrap();
        let b = s.insert(item("b", T0)).unwrap();
        let c = s.insert(item("c", T0 + 60_000)).unwrap();
        let mut first_two = [a.id.clone(), b.id.clone()];
        first_two.sort();

        let all = s.summaries_since(T0, None, 100).unwrap();
        assert_eq!(all.len(), 3, "the bound is inclusive");
        assert_eq!(all[2].id, c.id, "oldest change first");

        let page = s.summaries_since(T0, None, 1).unwrap();
        assert_eq!(page[0].id, first_two[0]);
        let rest = s.summaries_since(T0, Some(&page[0].id), 1).unwrap();
        assert_eq!(rest[0].id, first_two[1], "the boundary must drain by id");
    }

    /// The same exclusion `summaries` enforces: a live sensitive item must not
    /// be advertised by the incremental read either, and its tombstone must.
    #[test]
    fn the_cursor_excludes_live_sensitive_items_and_includes_their_tombstones() {
        let s = store();
        let secret = s.insert(sensitive_item("AKIA-shaped", T0)).unwrap();
        assert!(!s
            .summaries_since(T0, None, 100)
            .unwrap()
            .iter()
            .any(|v| v.id == secret.id));

        assert!(s.delete(&secret.id).unwrap());
        let tombstone = s
            .summaries_since(T0, None, 100)
            .unwrap()
            .into_iter()
            .find(|v| v.id == secret.id)
            .expect("a sensitive tombstone is still a version");
        assert!(tombstone.deleted);
        assert!(tombstone.is_sensitive);
    }

    /// `version_summary` is `version` without the ciphertext, for the merge —
    /// and `is_sensitive` is load-bearing in it: the tombstone branch reads the
    /// local flag, so losing it would let an incoming delete clear the flag and
    /// let the item back into the search index.
    #[test]
    fn a_version_summary_carries_the_sensitive_flag_and_no_payload() {
        let s = store();
        let secret = s.insert(sensitive_item("secret", T0)).unwrap();
        let gone = s.insert(item("gone", T0 + 60_000)).unwrap();
        s.delete(&gone.id).unwrap();

        let flagged = s.version_summary(&secret.id).unwrap().unwrap();
        assert!(flagged.is_sensitive);
        assert!(!flagged.deleted);
        assert_eq!(flagged.created_at, T0);

        let tombstone = s.version_summary(&gone.id).unwrap().unwrap();
        assert!(tombstone.deleted);
        assert!(!tombstone.is_sensitive);

        // Same answer as the full read, minus the payload.
        let full = s.version(&gone.id).unwrap().unwrap();
        assert_eq!(tombstone.content_hash, full.content_hash);
        assert_eq!(tombstone.origin_device_id, full.origin_device_id);
        assert_eq!(tombstone.pin_updated_at, full.pin_updated_at);

        assert!(s.version_summary("never-existed").unwrap().is_none());
    }
}
