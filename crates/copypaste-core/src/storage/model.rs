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
         pinned, is_sensitive, deleted, origin_device_id, app_bundle_id, payload_metadata"
    };
}

/// The same projection through the `ci` alias used by the FTS JOIN, aliased back
/// to the bare names so one row mapper serves both.
macro_rules! item_columns_ci {
    () => {
        "ci.id AS id, ci.content_ciphertext AS content_ciphertext, ci.nonce AS nonce, \
         ci.content_type AS content_type, ci.content_hash AS content_hash, \
         ci.created_at AS created_at, ci.pinned AS pinned, \
         ci.is_sensitive AS is_sensitive, ci.deleted AS deleted, \
         ci.origin_device_id AS origin_device_id, ci.app_bundle_id AS app_bundle_id, \
         ci.payload_metadata AS payload_metadata"
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
    /// JSON-encoded [`crate::FileMetadata`], only for file payloads.
    pub payload_metadata: Option<String>,
}

/// What [`super::Store::insert_or_bump`] did with a capture.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub is_sensitive: bool,
    /// A tombstone. `Store::get` and `Store::list` never return one; the sync
    /// reads in [`super::versions`] do, because a delete is a version.
    pub deleted: bool,
    /// Empty means "captured on this device" — see [`super::origin_or`].
    pub origin_device_id: String,
    /// Frontmost application at local capture time. Remote items may not have it.
    pub app_bundle_id: Option<String>,
    pub payload_metadata: Option<String>,
}

/// Storage failures.
///
/// No variant carries a filesystem path (CLAUDE.md rule 4). `rusqlite` and
/// `r2d2` errors do not embed one either, so these are safe to show a user.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
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

    /// A database written by v0.4.x. Its own code because the recovery is a
    /// human decision — keep the old history and downgrade, or start fresh —
    /// and never a retry. v2 shares no formats with it (CLAUDE.md rule 3), and
    /// nothing here opens, migrates or removes it.
    #[error(
        "this is a CopyPaste 0.4 history; this version cannot read it, and has left it as it was"
    )]
    LegacyDatabase,

    /// `PRAGMA integrity_check` returned something other than `ok`. The key was
    /// right and the pages are not: a candidate file in this state must not
    /// replace a working one.
    #[error("the database failed its integrity check")]
    IntegrityCheckFailed,

    #[error("item not found")]
    NotFound,

    /// A pagination cursor this build did not write. Refused rather than
    /// treated as "start from the top", which would silently make a load-more
    /// repeat the whole history.
    #[error("that page marker is not valid")]
    InvalidCursor,
}

pub(super) fn row_to_item(row: &Row<'_>) -> rusqlite::Result<StoredItem> {
    Ok(StoredItem {
        id: row.get("id")?,
        // NULL only on a tombstone, which no read path returns; map defensively
        // rather than failing the whole query.
        content_ciphertext: row
            .get::<_, Option<Vec<u8>>>("content_ciphertext")?
            .unwrap_or_default(),
        nonce: row.get::<_, Option<Vec<u8>>>("nonce")?.unwrap_or_default(),
        content_type: row.get("content_type")?,
        content_hash: row.get("content_hash")?,
        created_at: row.get("created_at")?,
        pinned: row.get("pinned")?,
        is_sensitive: row.get("is_sensitive")?,
        deleted: row.get("deleted")?,
        origin_device_id: row.get("origin_device_id")?,
        app_bundle_id: row.get("app_bundle_id")?,
        payload_metadata: row.get("payload_metadata")?,
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
