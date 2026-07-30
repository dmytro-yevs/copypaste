//! The only two write paths this module has: recording where an item was born,
//! and storing the version a session decided should win.

use rusqlite::params;

use super::error::is_constraint_violation;
use super::{IncomingVersion, Meta, MetaError};

impl Meta {
    /// Record which device first captured an item.
    ///
    /// Called on every local ingest. Idempotent, and deliberately *not* an
    /// update: an origin is the device an item was born on, and restamping it
    /// on a later hop destroys the merge tie-break's determinism across three
    /// devices (`protocol::SyncItem::origin_device_id`).
    pub fn record_origin(&self, item_id: &str, origin_device_id: &str) -> Result<(), MetaError> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO sync_item_origin (item_id, origin_device_id) VALUES (?1, ?2) \
             ON CONFLICT(item_id) DO NOTHING",
            params![item_id, origin_device_id],
        )?;
        Ok(())
    }

    /// Write the version a session decided should win.
    ///
    /// One transaction, and the only write path this module has. Returns
    /// `false` when the store refused the row, which happens for exactly one
    /// reason: the history table's dedup index already holds a *different* id
    /// with this content in the same 60-second bucket. Refusing is the safe
    /// direction — the content is already on this device under another id — and
    /// the caller reports it as skipped rather than failing the session.
    ///
    /// What it preserves deliberately: `pinned` and `pin_order` are local
    /// decisions and never come off the wire, so an incoming version keeps
    /// whatever pin this device has.
    ///
    /// A tombstone clears them, matching `Store::delete` — and that is safe
    /// only because a tombstone for a *pinned* row never reaches this function.
    /// [`crate::merge::apply_remote_version`] refuses it, so that one device
    /// clearing its history cannot destroy every other device's pinned items;
    /// the reasoning is there, with the merge policy it belongs to. What is
    /// left here is hygiene: a row that has genuinely become a tombstone has no
    /// content, so it has nothing to be pinned to.
    pub fn apply(&self, incoming: &IncomingVersion<'_>) -> Result<bool, MetaError> {
        let mut conn = self.lock()?;
        // IMMEDIATE, for the reason `copypaste_core::storage::connection::write_tx`
        // records: a deferred transaction that upgrades to a write mid-way gets
        // SQLITE_BUSY_SNAPSHOT the instant the other connection to this file is
        // writing, and `busy_timeout` does not retry that. A cloud round writing
        // through this same handle while a capture lands is the ordinary case,
        // not a rare one — it surfaced as "the item could not be stored" in
        // `demo-cloud.sh` two runs in five once the idle poll ceiling dropped
        // to 10 s.
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let written = tx.execute(
            "INSERT INTO clipboard_items \
                 (id, content_ciphertext, nonce, content_type, content_hash, \
                  is_sensitive, pinned, pin_order, created_at, deleted) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, ?7, ?8) \
             ON CONFLICT(id) DO UPDATE SET \
                 content_ciphertext = excluded.content_ciphertext, \
                 nonce              = excluded.nonce, \
                 content_type       = excluded.content_type, \
                 content_hash       = excluded.content_hash, \
                 is_sensitive       = excluded.is_sensitive, \
                 created_at         = excluded.created_at, \
                 deleted            = excluded.deleted, \
                 pinned    = CASE WHEN excluded.deleted = 1 THEN 0 ELSE clipboard_items.pinned END, \
                 pin_order = CASE WHEN excluded.deleted = 1 THEN NULL ELSE clipboard_items.pin_order END",
            params![
                incoming.item_id,
                incoming.content_ciphertext,
                incoming.nonce,
                incoming.content_type,
                incoming.content_hash,
                incoming.is_sensitive,
                incoming.created_at,
                incoming.deleted,
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

        // Unconditional, so a version that arrives sensitive, deleted, or
        // simply changed cannot leave a stale index row behind. This is the
        // write-time layer of "sensitive items never reach the search index"
        // for the sync path (`CLAUDE.md` rule 4).
        tx.execute(
            "DELETE FROM clipboard_fts WHERE id = ?1",
            [incoming.item_id],
        )?;
        if !incoming.deleted && !incoming.is_sensitive {
            if let Some(text) = incoming.search_text.filter(|t| !t.trim().is_empty()) {
                tx.execute(
                    "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
                    params![incoming.item_id, text],
                )?;
            }
        }

        tx.execute(
            "INSERT INTO sync_item_origin (item_id, origin_device_id) VALUES (?1, ?2) \
             ON CONFLICT(item_id) DO UPDATE SET origin_device_id = excluded.origin_device_id",
            params![incoming.item_id, incoming.origin_device_id],
        )?;

        tx.commit()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::testutil::{fixture, insert};

    #[test]
    fn a_recorded_origin_is_never_restamped() {
        let f = fixture();
        insert(&f.store, "plain", "hash-plain", 1_000, false);
        f.meta.record_origin("plain", "device-a").unwrap();
        f.meta.record_origin("plain", "device-b").unwrap();
        let local = f.meta.local_version("plain").unwrap().unwrap();
        assert_eq!(local.origin_device_id, "device-a");
    }

    #[test]
    fn applying_an_unknown_item_stores_it_with_its_remote_identity() {
        let f = fixture();
        let applied = f
            .meta
            .apply(&IncomingVersion {
                item_id: "from-peer",
                content_ciphertext: Some(&[9, 9, 9]),
                nonce: Some(&[8, 8, 8]),
                content_type: "text",
                content_hash: "hash-remote",
                created_at: 5_000,
                deleted: false,
                is_sensitive: false,
                origin_device_id: "device-a",
                search_text: Some("remote text"),
            })
            .unwrap();
        assert!(applied);

        let stored = f.store.get("from-peer").unwrap().expect("stored");
        assert_eq!(stored.created_at, 5_000);
        let local = f.meta.local_version("from-peer").unwrap().unwrap();
        assert_eq!(local.origin_device_id, "device-a");
        assert_eq!(local.summary.content_hash, "hash-remote");
        assert!(!f.store.search("remote", 10).unwrap().is_empty());
    }

    #[test]
    fn applying_a_sensitive_version_keeps_it_out_of_the_index() {
        let f = fixture();
        f.meta
            .apply(&IncomingVersion {
                item_id: "flagged",
                content_ciphertext: Some(&[1]),
                nonce: Some(&[2]),
                content_type: "text",
                content_hash: "hash-flagged",
                created_at: 5_000,
                deleted: false,
                is_sensitive: true,
                origin_device_id: "device-a",
                // Even when a caller supplies it, as the store's own layer does.
                search_text: Some("AKIAIOSFODNN7EXAMPLE"),
            })
            .unwrap();

        assert!(f
            .store
            .search("AKIAIOSFODNN7EXAMPLE", 10)
            .unwrap()
            .is_empty());
        assert!(f.store.get("flagged").unwrap().is_some());
    }

    #[test]
    fn applying_a_tombstone_over_a_live_item_wipes_it_and_unindexes_it() {
        let f = fixture();
        insert(&f.store, "doomed", "hash-doomed", 1_000, false);
        f.store.set_pinned("doomed", true).unwrap();

        f.meta
            .apply(&IncomingVersion {
                item_id: "doomed",
                content_ciphertext: None,
                nonce: None,
                content_type: "text",
                content_hash: "hash-doomed",
                created_at: 2_000,
                deleted: true,
                is_sensitive: false,
                origin_device_id: "device-a",
                search_text: None,
            })
            .unwrap();

        assert!(f.store.get("doomed").unwrap().is_none(), "still live");
        assert!(f.store.search("indexed", 10).unwrap().is_empty());
        let local = f.meta.local_version("doomed").unwrap().unwrap();
        assert!(local.summary.deleted);
    }

    #[test]
    fn a_pin_is_local_and_survives_an_incoming_version() {
        let f = fixture();
        insert(&f.store, "kept", "hash-kept", 1_000, false);
        f.store.set_pinned("kept", true).unwrap();

        f.meta
            .apply(&IncomingVersion {
                item_id: "kept",
                content_ciphertext: Some(&[3]),
                nonce: Some(&[4]),
                content_type: "text",
                content_hash: "hash-newer",
                created_at: 9_000,
                deleted: false,
                is_sensitive: false,
                origin_device_id: "device-a",
                search_text: Some("newer text"),
            })
            .unwrap();

        assert!(f.store.get("kept").unwrap().unwrap().pinned);
    }

    #[test]
    fn a_collision_with_the_dedup_index_is_refused_not_an_error() {
        let f = fixture();
        insert(&f.store, "mine", "same-hash", 1_000, false);

        // Same content hash, same 60-second bucket, different id: the store's
        // dedup index owns that pair.
        let applied = f
            .meta
            .apply(&IncomingVersion {
                item_id: "theirs",
                content_ciphertext: Some(&[1]),
                nonce: Some(&[2]),
                content_type: "text",
                content_hash: "same-hash",
                created_at: 1_500,
                deleted: false,
                is_sensitive: false,
                origin_device_id: "device-a",
                search_text: Some("text"),
            })
            .unwrap();
        assert!(!applied, "the collision must be reported, not stored");
        assert!(f.store.get("theirs").unwrap().is_none());
        assert!(f.store.get("mine").unwrap().is_some(), "no local data lost");
    }
}
