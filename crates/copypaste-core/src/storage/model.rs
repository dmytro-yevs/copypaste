//! The row types, the one column projection every read shares, and the error
//! set. One contract: the projection lists the columns, [`row_to_item`] maps
//! exactly those, and [`StoredItem`] is their shape. Splitting them is how the
//! three drift apart.

use rusqlite::{ErrorCode, Row};

/// The projection every read uses. Bound by *name*, never by position — v1
/// maintained three parallel positional column lists and an off-by-one panic in
/// the row mapper was the result (`CopyPaste-crh3.85`).
macro_rules! item_columns {
    () => {
        "id, content_ciphertext, nonce, content_type, content_hash, created_at, \
         pinned, pin_order, pin_updated_at, is_sensitive, deleted, origin_device_id, \
         app_bundle_id, app_name, payload_metadata"
    };
}

/// The same projection through the `ci` alias used by the FTS JOIN, aliased back
/// to the bare names so one row mapper serves both.
macro_rules! item_columns_ci {
    () => {
        "ci.id AS id, ci.content_ciphertext AS content_ciphertext, ci.nonce AS nonce, \
         ci.content_type AS content_type, ci.content_hash AS content_hash, \
         ci.created_at AS created_at, ci.pinned AS pinned, ci.pin_order AS pin_order, \
         ci.pin_updated_at AS pin_updated_at, \
         ci.is_sensitive AS is_sensitive, ci.deleted AS deleted, \
         ci.origin_device_id AS origin_device_id, ci.app_bundle_id AS app_bundle_id, \
         ci.app_name AS app_name, ci.payload_metadata AS payload_metadata"
    };
}

pub(super) use {item_columns, item_columns_ci};

/// An item on its way into the store.
pub struct NewItem {
    /// Primary key, chosen by the caller — not generated here. The item AEAD
    /// binds this id as associated data, so the id must exist before the seal
    /// and therefore before this insert. A store-minted id would produce rows
    /// whose ciphertext authenticates against an id they do not have, failing
    /// closed on every later read: a silent, total loss of the content.
    pub id: String,
    pub content_ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub content_type: String,
    pub content_hash: String,
    pub is_sensitive: bool,
    /// Plaintext for the search index. MUST be `None` when `is_sensitive` is
    /// true. [`super::Store::insert`] enforces this rather than trusting it: a
    /// non-`None` value on a sensitive item is dropped (and logged), never
    /// indexed. The insert itself still succeeds — refusing it would lose the
    /// user's clipboard content, and data loss is the worst outcome.
    pub search_text: Option<String>,
    /// Milliseconds since the Unix epoch.
    pub created_at: i64,
    /// Frontmost application at local capture time, if the platform supplied it.
    pub app_bundle_id: Option<String>,
    /// Human-readable label resolved by the platform alongside the app id.
    pub app_name: Option<String>,
    /// JSON-encoded [`crate::FileMetadata`], only for file payloads.
    pub payload_metadata: Option<String>,
}

/// What [`super::Store::insert_or_bump`] did with a capture.
#[derive(Debug, Clone, PartialEq)]
pub enum Ingest {
    /// The content was new to this device's history.
    Inserted(StoredItem),
    /// The content was already here, and this row's `created_at` was moved to
    /// the new capture time. The row is the *existing* one, under its original
    /// id — never the rejected candidate (manifest 01 I-28: broadcasting the
    /// candidate's id makes every subscriber look up a row that does not exist).
    Bumped(StoredItem),
}

impl Ingest {
    #[must_use]
    pub fn item(&self) -> &StoredItem {
        match self {
            Ingest::Inserted(item) | Ingest::Bumped(item) => item,
        }
    }

    #[must_use]
    pub fn into_item(self) -> StoredItem {
        match self {
            Ingest::Inserted(item) | Ingest::Bumped(item) => item,
        }
    }

    #[must_use]
    pub fn is_bump(&self) -> bool {
        matches!(self, Ingest::Bumped(_))
    }
}

/// A row as it comes back out. The plaintext never leaves the store: callers
/// decrypt `content_ciphertext` themselves.
///
/// This is also what a sync session compares and serves. The four merge keys
/// (`created_at`, `content_hash`, `deleted`, `origin_device_id`) are all here
/// so that one row type answers both questions; carrying them in a second view
/// on a second connection is what `crate::sync` was extracted from.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredItem {
    pub id: String,
    /// Empty on a tombstone: the soft delete wiped the payload.
    pub content_ciphertext: Vec<u8>,
    /// Empty on a tombstone, for the same reason.
    pub nonce: Vec<u8>,
    pub content_type: String,
    /// SHA-256 hex of the pre-encryption bytes, kept on tombstones on purpose —
    /// a delete has to tie the version it deletes on merge key 2 so that key 3
    /// decides it.
    pub content_hash: String,
    pub created_at: i64,
    pub pinned: bool,
    pub pin_order: Option<f64>,
    pub pin_updated_at: i64,
    pub is_sensitive: bool,
    /// A tombstone. `Store::get` and `Store::list` never return one; the sync
    /// reads in [`super::versions`] do, because a delete is a version.
    pub deleted: bool,
    /// Empty means "captured on this device" — see [`super::origin_or`].
    pub origin_device_id: String,
    /// Frontmost application at local capture time. Remote items may not have it.
    pub app_bundle_id: Option<String>,
    /// Name resolved by the capturing platform. It is advisory and never used
    /// as an identifier or as a filesystem path.
    pub app_name: Option<String>,
    pub payload_metadata: Option<String>,
}

/// Storage failures.
///
/// No variant carries a filesystem path (CLAUDE.md rule 4). `rusqlite` and
/// `r2d2` errors do not embed one either, so these are safe to show a user.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database file operation failed")]
    File(#[from] std::io::Error),

    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("connection pool error: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("schema migration failed: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    /// The supplied key does not open this database, or the file is not a
    /// database at all. Fail closed: never fall back to an unkeyed read.
    #[error("database could not be opened with the supplied key")]
    InvalidKey,

    /// `PRAGMA integrity_check` returned something other than `ok`. The key was
    /// right and the pages are not: a candidate file in this state must not
    /// replace a working one.
    #[error("the database failed its integrity check")]
    IntegrityCheckFailed,

    #[error("the database schema does not match this version")]
    InvalidSchema,

    #[error("item not found")]
    NotFound,

    /// A pagination cursor this build did not write. Refused rather than
    /// treated as "start from the top", which would silently make a load-more
    /// repeat the whole history.
    #[error("that page marker is not valid")]
    InvalidCursor,

    #[error("a device name must contain visible text")]
    InvalidDeviceName,
}

pub(super) const ITEM_COLUMN_COUNT: usize = 15;

pub(super) struct ItemColumns([usize; ITEM_COLUMN_COUNT]);

impl ItemColumns {
    pub(super) fn resolve(stmt: &rusqlite::Statement<'_>) -> rusqlite::Result<Self> {
        let mut at = [0usize; ITEM_COLUMN_COUNT];
        for (slot, name) in (0..ITEM_COLUMN_COUNT).zip(item_columns!().split(',')) {
            at[slot] = stmt.column_index(name.trim())?;
        }
        Ok(Self(at))
    }

    pub(super) fn pin_order_index(&self) -> usize {
        self.0[7]
    }
}

pub(super) fn row_to_item(row: &Row<'_>, columns: &ItemColumns) -> rusqlite::Result<StoredItem> {
    let at = &columns.0;
    Ok(StoredItem {
        id: row.get(at[0])?,
        // NULL only on a tombstone, which no read path returns; map defensively
        // rather than failing the whole query.
        content_ciphertext: row.get::<_, Option<Vec<u8>>>(at[1])?.unwrap_or_default(),
        nonce: row.get::<_, Option<Vec<u8>>>(at[2])?.unwrap_or_default(),
        content_type: row.get(at[3])?,
        content_hash: row.get(at[4])?,
        created_at: row.get(at[5])?,
        pinned: row.get(at[6])?,
        pin_order: row.get(at[7])?,
        pin_updated_at: row.get(at[8])?,
        is_sensitive: row.get(at[9])?,
        deleted: row.get(at[10])?,
        origin_device_id: row.get(at[11])?,
        app_bundle_id: row.get(at[12])?,
        app_name: row.get(at[13])?,
        payload_metadata: row.get(at[14])?,
    })
}

// Both classifications match on the SQLite result code, never on a message
// string: v1's migration runner string-matched `"duplicate column name"` and
// broke when the wording changed.

pub(super) fn is_constraint_violation(err: &rusqlite::Error) -> bool {
    matches!(err, rusqlite::Error::SqliteFailure(e, _)
        if e.code == ErrorCode::ConstraintViolation)
}

pub(super) fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(err, rusqlite::Error::SqliteFailure(e, _)
        if e.code == ErrorCode::NotADatabase || e.code == ErrorCode::DatabaseCorrupt)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{item, store, T0};
    use super::*;

    #[test]
    fn the_resolved_indices_match_the_projection_and_follow_the_names() {
        assert_eq!(
            item_columns!().split(',').count(),
            ITEM_COLUMN_COUNT,
            "the projection and the resolved width must agree"
        );
        assert_eq!(
            item_columns_ci!().split(',').count(),
            ITEM_COLUMN_COUNT,
            "the aliased projection must list the same columns"
        );

        let s = store();
        let stored = s.insert(item("round trip", T0)).unwrap();
        let conn = s.conn().unwrap();

        for sql in [
            concat!("SELECT ", item_columns!(), " FROM clipboard_items"),
            concat!("SELECT ", item_columns_ci!(), " FROM clipboard_items ci"),
            "SELECT payload_metadata, app_name, app_bundle_id, origin_device_id, deleted, \
             is_sensitive, pin_updated_at, pin_order, pinned, created_at, content_hash, \
             content_type, nonce, content_ciphertext, id FROM clipboard_items",
        ] {
            let mut stmt = conn.prepare(sql).unwrap();
            let columns = ItemColumns::resolve(&stmt).unwrap();
            let read = stmt
                .query_row([], |row| row_to_item(row, &columns))
                .unwrap();
            assert_eq!(read, stored, "projection: {sql}");
        }
    }
}
