//! The one v2 schema, created in one transaction on a newly reserved file.

use rusqlite::Connection;

use super::model::StoreError;

pub(super) const SCHEMA: &str = concat!(
    include_str!("migrations/000_initial.sql"),
    include_str!("migrations/001_source_icons.sql"),
    include_str!("migrations/003_file_import.sql"),
);

pub(super) fn create(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    tx.execute_batch(SCHEMA)?;
    super::migrations::record_current(&tx)?;
    tx.pragma_update(None, "user_version", super::migrations::LATEST_VERSION)?;
    tx.commit()?;
    super::schema_verify::verify_schema(conn)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{KEY, OTHER_KEY};
    use super::super::{Store, StoreError};

    #[test]
    fn fresh_open_creates_the_canonical_schema_and_reopens_it() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");

        let store = Store::open(&path, &KEY).unwrap();
        super::super::schema_verify::verify_schema(&store.conn().unwrap()).unwrap();
        drop(store);
        Store::open(&path, &KEY).unwrap();
    }

    #[test]
    fn an_incompatible_database_is_refused_without_repairing_it() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        store
            .conn()
            .unwrap()
            .execute_batch("DROP INDEX idx_items_sync_cursor")
            .unwrap();
        drop(store);

        let error = Store::open(&path, &KEY).unwrap_err();
        assert!(matches!(error, StoreError::InvalidSchema));
        assert!(!error.to_string().contains(&*path.to_string_lossy()));

        let conn = rusqlite::Connection::open(&path).unwrap();
        super::super::connection::apply_key(&conn, &KEY).unwrap();
        super::super::connection::validate_key(&conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'idx_items_sync_cursor'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "open must not repair an incompatible schema");
    }

    #[test]
    fn wrong_key_still_fails_closed() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        drop(Store::open(&path, &KEY).unwrap());

        assert!(matches!(
            Store::open(&path, &OTHER_KEY),
            Err(StoreError::InvalidKey)
        ));
    }

    #[test]
    fn fresh_schema_records_the_registered_version() {
        let store = Store::open_in_memory(&KEY).unwrap();
        let version: u32 = store
            .conn()
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, super::super::migrations::LATEST_VERSION);
    }
}
