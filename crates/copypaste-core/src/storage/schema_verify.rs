//! Proving that a database file *is* the schema this build writes.
//!
//! Separate from [`super::schema`] because the question is the opposite one: a
//! candidate — a backup a user is about to restore over their history — is
//! inspected as it stands and refused if it does not match, rather than being
//! upgraded into matching.

use rusqlite::{Connection, OptionalExtension};

use super::model::StoreError;
use super::schema::SCHEMA_VERSION;

struct Column {
    name: &'static str,
    declared_type: &'static str,
    not_null: bool,
    primary_key: bool,
}

struct Table {
    name: &'static str,
    columns: &'static [Column],
    virtual_fts5: bool,
}

struct ActualColumn {
    name: String,
    declared_type: String,
    not_null: bool,
    primary_key: bool,
}

const CLIPBOARD_ITEMS_COLUMNS: &[Column] = &[
    Column {
        name: "id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: true,
    },
    Column {
        name: "content_ciphertext",
        declared_type: "BLOB",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "nonce",
        declared_type: "BLOB",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "content_type",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "content_hash",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "is_sensitive",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "pinned",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "pin_order",
        declared_type: "REAL",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "pin_updated_at",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "created_at",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "deleted",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "origin_device_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "app_bundle_id",
        declared_type: "TEXT",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "app_name",
        declared_type: "TEXT",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "payload_metadata",
        declared_type: "TEXT",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "fts_rowid",
        declared_type: "INTEGER",
        not_null: false,
        primary_key: false,
    },
];

const CLIPBOARD_FTS_COLUMNS: &[Column] = &[
    Column {
        name: "id",
        declared_type: "",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "content_text",
        declared_type: "",
        not_null: false,
        primary_key: false,
    },
];

const SYNC_DEVICE_STATE_COLUMNS: &[Column] = &[
    Column {
        name: "key",
        declared_type: "TEXT",
        not_null: true,
        primary_key: true,
    },
    Column {
        name: "value",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
];

const SYNC_DEVICE_NAME_COLUMNS: &[Column] = &[
    Column {
        name: "device_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: true,
    },
    Column {
        name: "name",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
];

const TABLES: &[Table] = &[
    Table {
        name: "clipboard_items",
        columns: CLIPBOARD_ITEMS_COLUMNS,
        virtual_fts5: false,
    },
    Table {
        name: "clipboard_fts",
        columns: CLIPBOARD_FTS_COLUMNS,
        virtual_fts5: true,
    },
    Table {
        name: "sync_device_state",
        columns: SYNC_DEVICE_STATE_COLUMNS,
        virtual_fts5: false,
    },
    Table {
        name: "sync_device_name",
        columns: SYNC_DEVICE_NAME_COLUMNS,
        virtual_fts5: false,
    },
];

pub(super) fn verify_schema(conn: &Connection) -> Result<(), StoreError> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version != SCHEMA_VERSION {
        return Err(StoreError::InvalidSchema);
    }

    for table in TABLES {
        verify_table(conn, table)?;
    }
    Ok(())
}

fn verify_table(conn: &Connection, expected: &Table) -> Result<(), StoreError> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [expected.name],
            |row| row.get(0),
        )
        .optional()?;
    let Some(sql) = sql else {
        return Err(StoreError::InvalidSchema);
    };
    if expected.virtual_fts5 {
        let definition = sql.trim_start().to_ascii_lowercase();
        if !definition.starts_with("create virtual table") || !definition.contains("using fts5") {
            return Err(StoreError::InvalidSchema);
        }
    }

    let pragma = format!("PRAGMA table_info({})", quote_identifier(expected.name));
    let mut statement = conn.prepare(&pragma)?;
    let actual = statement
        .query_map([], |row| {
            Ok(ActualColumn {
                name: row.get(1)?,
                declared_type: row.get(2)?,
                not_null: row.get::<_, i64>(3)? != 0,
                primary_key: row.get::<_, i64>(5)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if actual.len() != expected.columns.len()
        || expected.columns.iter().any(|expected_column| {
            actual
                .iter()
                .find(|actual| actual.name == expected_column.name)
                .is_none_or(|actual| {
                    actual.declared_type != expected_column.declared_type
                        || actual.not_null != expected_column.not_null
                        || actual.primary_key != expected_column.primary_key
                })
        })
    {
        return Err(StoreError::InvalidSchema);
    }
    Ok(())
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::super::test_support::KEY;
    use super::super::{Store, StoreError};
    use super::verify_schema;

    #[test]
    fn the_current_schema_passes_validation() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        let conn = store.conn().unwrap();

        verify_schema(&conn).unwrap();
    }

    #[test]
    fn schema_validation_rejects_a_column_with_the_wrong_type() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        let conn = store.conn().unwrap();
        conn.execute_batch(
            "DROP TABLE sync_device_name;
             CREATE TABLE sync_device_name (
                 device_id BLOB PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL
             );",
        )
        .unwrap();

        assert!(matches!(
            verify_schema(&conn),
            Err(StoreError::InvalidSchema)
        ));
    }
}
