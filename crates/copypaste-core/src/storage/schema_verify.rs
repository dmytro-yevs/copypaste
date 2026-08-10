//! Proving that a database file is exactly the schema this build writes.

use rusqlite::Connection;

use super::model::StoreError;

const CURRENT_TABLES: &[&str] = &[
    "clipboard_items",
    "clipboard_fts",
    "clipboard_live_count",
    "sync_device_state",
    "sync_device_name",
];

#[derive(Debug, PartialEq)]
struct SchemaObject {
    kind: String,
    name: String,
    table: String,
    sql: String,
}

pub(super) fn verify_schema(conn: &Connection) -> Result<(), StoreError> {
    let expected = Connection::open_in_memory()?;
    expected.execute_batch(super::schema::SCHEMA)?;

    if schema_objects(conn)? != schema_objects(&expected)? {
        return Err(StoreError::InvalidSchema);
    }
    Ok(())
}

pub(super) fn is_current_table(name: &str) -> bool {
    CURRENT_TABLES.contains(&name)
}

fn schema_objects(conn: &Connection) -> rusqlite::Result<Vec<SchemaObject>> {
    let mut statement = conn.prepare(
        "SELECT type, name, tbl_name, sql FROM sqlite_schema \
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' \
         ORDER BY type, name",
    )?;
    let objects = statement
        .query_map([], |row| {
            Ok(SchemaObject {
                kind: row.get(0)?,
                name: row.get(1)?,
                table: row.get(2)?,
                sql: row.get(3)?,
            })
        })?
        .collect();
    objects
}

#[cfg(test)]
mod tests {
    use super::super::test_support::KEY;
    use super::super::{Store, StoreError};
    use super::verify_schema;

    #[test]
    fn the_current_schema_passes_validation() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();

        verify_schema(&store.conn().unwrap()).unwrap();
    }

    #[test]
    fn validation_rejects_equivalent_but_noncanonical_ddl() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        let conn = store.conn().unwrap();
        conn.execute_batch(
            "DROP TABLE sync_device_name;
             CREATE TABLE sync_device_name (
                 device_id TEXT NOT NULL PRIMARY KEY,
                 name TEXT NOT NULL
             );",
        )
        .unwrap();

        assert!(matches!(
            verify_schema(&conn),
            Err(StoreError::InvalidSchema)
        ));
    }

    #[test]
    fn validation_rejects_a_missing_trigger() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        let conn = store.conn().unwrap();
        conn.execute_batch("DROP TRIGGER clipboard_live_count_insert")
            .unwrap();

        assert!(matches!(
            verify_schema(&conn),
            Err(StoreError::InvalidSchema)
        ));
    }
}
