//! Recognising a v0.4.x history file, so a v2 build that meets one says so
//! instead of reporting corruption.
//!
//! This is the whole of CLAUDE.md rule 3's obligation. The distinct v2 filename
//! discharges the first half — a v1 file is never opened, migrated or
//! overwritten — and this discharges the second: "a v2 build that stumbles onto
//! it must say so plainly rather than failing with a decryption error that reads
//! like corruption".
//!
//! # Why identification is positive, and where its edge is
//!
//! "The key did not work, therefore v1" would label a genuinely damaged v2 file
//! as an old history and send the user looking for a downgrade that will not
//! help. Nothing here consults the key at all. Two independent identifications,
//! tried in that order:
//!
//! 1. **The schema, read directly.** v0.4.x could hold a *plaintext* SQLite file
//!    (it encrypted one in place on first open, manifest 03 §3.4-§3.5), so when
//!    a file opens unkeyed its own tables answer the question: `user_version` in
//!    `1..=15` and a `clipboard_items` carrying columns v2 does not have. This
//!    is conclusive whatever the file is called. A file that opens unkeyed and
//!    is *not* that schema is conclusively **not** v1 — which is what stops a
//!    stray database inheriting the verdict from its name.
//! 2. **The name, for the encrypted case.** SQLCipher pages are
//!    indistinguishable from random bytes; an encrypted v1 file has no header,
//!    no magic and no version to read. Its name is the only signal that
//!    survives, and it is a designed one: v2 writes `copypaste-v2.db`
//!    *precisely* so that `clipboard.db` can only have come from v0.4.x. Paired
//!    with a page-shape check so a text file someone parked under that name is
//!    not announced as a history.
//!
//! The edge, stated rather than hidden: an encrypted v1 file that has been
//! renamed is not identified, and falls back to [`StoreError::InvalidKey`]. That
//! is the safe direction — a wrong-key error on a file we cannot name is honest.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

/// The filename v0.4.x wrote its history to.
///
/// v2 writes `copypaste-v2.db` (`copypaste_ipc::database_path`), so the two
/// names never collide and this one is evidence rather than a coincidence.
pub const V1_DATABASE_FILENAME: &str = "clipboard.db";

/// v0.4.x's migration ladder ran `user_version` 1 through 15 (manifest 03 §4).
const V1_SCHEMA_MIN: i64 = 1;
const V1_SCHEMA_MAX: i64 = 15;

/// Columns present in every v0.4.x `clipboard_items` and in no v2 one: the
/// cross-device identity, the Lamport stamp and the v1 spelling of `created_at`
/// (manifest 03 §3.1). All three must be there — one alone could be a
/// coincidence in some other application's database.
const V1_ONLY_COLUMNS: [&str; 3] = ["item_id", "lamport_ts", "wall_time"];

/// SQLCipher 4's default page size, which v0.4.x did not override (manifest 03
/// I5: no cipher parameter other than the key was ever set). SQLite writes whole
/// pages, so a real database file is a whole number of them.
const SQLCIPHER_PAGE_BYTES: u64 = 4096;

/// Whether the file at `path` is a database written by v0.4.x.
///
/// Never opens it with a key, never writes to it, and never reports a verdict
/// derived from a failed decryption.
#[must_use]
pub fn is_v1_database(path: &Path) -> bool {
    match probe_unkeyed(path) {
        Unkeyed::V1Schema => true,
        Unkeyed::OtherDatabase => false,
        Unkeyed::Unreadable => encrypted_under_the_v1_name(path),
    }
}

/// Whether a v0.4.x history is sitting in `dir`.
///
/// The directory is the caller's to supply: v0.4.x resolved its own data
/// directory (`~/Library/Application Support/CopyPaste` on macOS) and v2 does
/// not resolve to the same place, so "beside the v2 database" is not the same
/// question as "where v0.4.x put it". Duplicating either resolver here would be
/// the second copy of a path rule that already has one owner.
#[must_use]
pub fn v1_database_in(dir: &Path) -> bool {
    is_v1_database(&dir.join(V1_DATABASE_FILENAME))
}

enum Unkeyed {
    /// Opened without a key and carries v0.4.x's schema.
    V1Schema,
    /// Opened without a key and does not. Conclusive: this is some other
    /// database, whatever it is named.
    OtherDatabase,
    /// Encrypted, absent, or not a database — nothing was learned.
    Unreadable,
}

fn probe_unkeyed(path: &Path) -> Unkeyed {
    // Read-only, so a probe of a file we have decided not to touch cannot
    // create it, journal it, or replay a WAL into it.
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let Ok(conn) = Connection::open_with_flags(path, flags) else {
        return Unkeyed::Unreadable;
    };
    let Ok(version) = conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)) else {
        return Unkeyed::Unreadable;
    };
    if !(V1_SCHEMA_MIN..=V1_SCHEMA_MAX).contains(&version) {
        return Unkeyed::OtherDatabase;
    }
    let all_present = V1_ONLY_COLUMNS
        .iter()
        .all(|column| has_column(&conn, column).unwrap_or(false));
    if all_present {
        Unkeyed::V1Schema
    } else {
        Unkeyed::OtherDatabase
    }
}

fn has_column(conn: &Connection, column: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('clipboard_items') WHERE name = ?1",
        [column],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n > 0)
}

fn encrypted_under_the_v1_name(path: &Path) -> bool {
    if path.file_name().and_then(|n| n.to_str()) != Some(V1_DATABASE_FILENAME) {
        return false;
    }
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let len = meta.len();
    meta.is_file() && len >= SQLCIPHER_PAGE_BYTES && len % SQLCIPHER_PAGE_BYTES == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{item, KEY, OTHER_KEY, T0};
    use crate::storage::{open_validated, Store, StoreError};

    /// A plaintext file with v0.4.x's schema, which is what a user who never
    /// upgraded past the pre-encryption builds still has (manifest 03 §3.4).
    fn stage_v1_plaintext(path: &Path, user_version: i64) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE clipboard_items (
                 id          TEXT PRIMARY KEY NOT NULL,
                 item_id     TEXT,
                 content     BLOB,
                 lamport_ts  INTEGER NOT NULL DEFAULT 0,
                 wall_time   INTEGER NOT NULL DEFAULT 0
             );",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", user_version)
            .unwrap();
    }

    /// An encrypted v0.4.x file cannot be staged — v2 cannot derive v1's key —
    /// so stand in the one thing that is observable about it: SQLCipher-shaped
    /// bytes under v0.4.x's name.
    fn stage_v1_encrypted(path: &Path) {
        std::fs::write(path, vec![0x5au8; 3 * 4096]).unwrap();
    }

    #[test]
    fn a_plaintext_v1_schema_is_identified_under_any_name() {
        let dir = tempfile::tempdir().unwrap();
        for (name, version) in [("clipboard.db", 1), ("renamed-by-the-user.db", 15)] {
            let path = dir.path().join(name);
            stage_v1_plaintext(&path, version);
            assert!(is_v1_database(&path), "{name} at user_version {version}");
        }
    }

    #[test]
    fn an_encrypted_file_under_the_v1_name_is_identified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(V1_DATABASE_FILENAME);
        stage_v1_encrypted(&path);
        assert!(is_v1_database(&path));
        assert!(v1_database_in(dir.path()));
    }

    /// The distinction the whole module exists for: a v2 database that will not
    /// open is *not* a v0.4.x one, and must not be announced as one.
    #[test]
    fn a_v2_database_is_never_identified_as_v1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        {
            let store = Store::open(&path, &KEY).unwrap();
            store.insert(item("current history", T0)).unwrap();
        }
        assert!(!is_v1_database(&path));

        // Even when the key is wrong, which is the moment the temptation to
        // guess "v1" arises.
        assert!(matches!(
            open_validated(&path, &OTHER_KEY),
            Err(StoreError::InvalidKey)
        ));
    }

    /// A corrupt file must stay corrupt. The name is only evidence for a file
    /// that has a database's shape.
    #[test]
    fn a_corrupt_file_is_not_mislabelled_as_v1() {
        let dir = tempfile::tempdir().unwrap();

        let truncated = dir.path().join(V1_DATABASE_FILENAME);
        std::fs::write(&truncated, vec![0x5au8; 4096 + 17]).unwrap();
        assert!(!is_v1_database(&truncated));

        let empty = dir.path().join("empty").join(V1_DATABASE_FILENAME);
        std::fs::create_dir_all(empty.parent().unwrap()).unwrap();
        std::fs::write(&empty, b"").unwrap();
        assert!(!is_v1_database(&empty));

        let text = dir.path().join("text").join(V1_DATABASE_FILENAME);
        std::fs::create_dir_all(text.parent().unwrap()).unwrap();
        std::fs::write(&text, b"not a database, just a note someone saved").unwrap();
        assert!(!is_v1_database(&text));
    }

    /// Another application's database parked under the name is readable, so its
    /// own schema answers for it — the name does not get to.
    #[test]
    fn an_unrelated_plaintext_database_under_the_name_is_not_v1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(V1_DATABASE_FILENAME);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE notes (body TEXT); INSERT INTO notes VALUES ('hi');")
            .unwrap();
        conn.pragma_update(None, "user_version", 3).unwrap();
        drop(conn);
        assert!(!is_v1_database(&path));
    }

    /// A v0.4.x table shape at a version outside the ladder is not v0.4.x.
    #[test]
    fn a_schema_version_outside_the_v1_ladder_is_not_v1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("odd.db");
        stage_v1_plaintext(&path, 16);
        assert!(!is_v1_database(&path));
    }

    #[test]
    fn a_missing_file_is_not_v1() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_v1_database(&dir.path().join(V1_DATABASE_FILENAME)));
        assert!(!v1_database_in(dir.path()));
        assert!(!v1_database_in(&dir.path().join("no-such-directory")));
    }

    /// Fail closed, and change nothing: the error is distinct, and the file is
    /// byte-for-byte what it was.
    #[test]
    fn opening_a_v1_database_refuses_and_leaves_it_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(V1_DATABASE_FILENAME);
        stage_v1_plaintext(&path, 15);
        let before = std::fs::read(&path).unwrap();

        let err = open_validated(&path, &KEY).expect_err("a v1 database must not open");
        assert!(matches!(err, StoreError::LegacyDatabase), "{err:?}");
        let err = Store::open(&path, &KEY).expect_err("a v1 database must not open");
        assert!(matches!(err, StoreError::LegacyDatabase), "{err:?}");

        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the file was changed"
        );
        assert!(
            !dir.path().join("clipboard.db-wal").exists(),
            "the probe journalled the file"
        );
    }

    /// CLAUDE.md rule 4: the sentence a UI renders may not disclose a path.
    #[test]
    fn the_error_names_no_path_and_reads_as_a_sentence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(V1_DATABASE_FILENAME);
        stage_v1_plaintext(&path, 15);

        let rendered = open_validated(&path, &KEY).unwrap_err().to_string();
        assert!(!rendered.contains(&*path.to_string_lossy()));
        assert!(!rendered.contains(V1_DATABASE_FILENAME));
        assert!(!rendered.contains('/'));
    }
}
