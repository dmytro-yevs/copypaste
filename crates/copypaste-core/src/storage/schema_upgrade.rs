//! Upgrading known v2 storage schemas to the current canonical form.

use rusqlite::{Connection, OptionalExtension};

use super::connection::write_tx;
use super::model::StoreError;

const LEGACY_TABLES: &[&str] = &["__rusqlite_migrations"];

pub(super) fn upgrade_if_legacy_v2(conn: &mut Connection) -> Result<(), StoreError> {
    if super::schema_verify::verify_schema(conn).is_ok() {
        return Ok(());
    }
    if !looks_like_legacy_v2(conn)? {
        return Err(StoreError::InvalidSchema);
    }

    let tx = write_tx(conn)?;
    rebuild_clipboard_items(&tx)?;
    rebuild_sync_table(&tx, "sync_device_state", &["key", "value"])?;
    rebuild_sync_table(&tx, "sync_device_name", &["device_id", "name"])?;
    rebuild_indexes(&tx)?;
    rebuild_live_count(&tx)?;
    drop_legacy_tables(&tx)?;
    tx.commit()?;
    Ok(())
}

fn looks_like_legacy_v2(conn: &Connection) -> Result<bool, StoreError> {
    if !has_table(conn, "clipboard_items")?
        || !has_table(conn, "clipboard_fts")?
        || !has_index(conn, "idx_items_history")?
        || !has_index(conn, "idx_items_hash")?
        || !has_index(conn, "idx_items_dedup")?
    {
        return Ok(false);
    }

    for name in user_tables(conn)? {
        if super::schema_verify::is_current_table(&name) || LEGACY_TABLES.contains(&name.as_str()) {
            continue;
        }
        return Ok(false);
    }

    Ok(has_table(conn, "__rusqlite_migrations")?
        || !has_column(conn, "clipboard_items", "content_bytes")?
        || !has_index(conn, "idx_items_sensitive_wipe")?
        || !has_column(conn, "clipboard_items", "origin_device_id")?
        || !has_column(conn, "clipboard_items", "pin_updated_at")?
        || !has_column(conn, "clipboard_items", "fts_rowid")?
        || !has_table(conn, "clipboard_live_count")?
        || !has_table(conn, "sync_device_state")?
        || !has_table(conn, "sync_device_name")?)
}

fn rebuild_clipboard_items(conn: &Connection) -> Result<(), StoreError> {
    let old_name = "clipboard_items_legacy_v2";
    conn.execute_batch(&format!(
        "ALTER TABLE clipboard_items RENAME TO {old_name};"
    ))?;
    conn.execute_batch(&expected_sql("table", "clipboard_items")?)?;
    let select = clipboard_items_select(conn, old_name)?;
    conn.execute_batch(&format!(
        "INSERT INTO clipboard_items (
             id, content_ciphertext, nonce, content_type, content_hash, is_sensitive,
             pinned, pin_order, pin_updated_at, created_at, deleted, origin_device_id,
             app_bundle_id, app_name, payload_metadata, fts_rowid, content_bytes
         ) SELECT {select} FROM {old_name};"
    ))?;
    conn.execute_batch(&format!("DROP TABLE {old_name};"))?;
    Ok(())
}

fn rebuild_indexes(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "DROP INDEX IF EXISTS idx_items_history;
         DROP INDEX IF EXISTS idx_items_hash;
         DROP INDEX IF EXISTS idx_items_dedup;
         DROP INDEX IF EXISTS idx_items_evictable;
         DROP INDEX IF EXISTS idx_items_sensitive_wipe;
         DROP INDEX IF EXISTS idx_items_unpinned_bytes;
         DROP INDEX IF EXISTS idx_items_syncable;
         DROP INDEX IF EXISTS idx_items_sync_cursor;",
    )?;
    for name in [
        "idx_items_history",
        "idx_items_hash",
        "idx_items_dedup",
        "idx_items_evictable",
        "idx_items_sensitive_wipe",
        "idx_items_unpinned_bytes",
        "idx_items_syncable",
        "idx_items_sync_cursor",
    ] {
        conn.execute_batch(&expected_sql("index", name)?)?;
    }
    Ok(())
}

fn rebuild_live_count(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS clipboard_live_count_insert;
         DROP TRIGGER IF EXISTS clipboard_live_count_delete;
         DROP TRIGGER IF EXISTS clipboard_live_count_update;
         DROP TABLE IF EXISTS clipboard_live_count;",
    )?;
    conn.execute_batch(&expected_sql("table", "clipboard_live_count")?)?;
    conn.execute(
        "INSERT INTO clipboard_live_count (only_row, live)
         VALUES (0, (SELECT COUNT(*) FROM clipboard_items WHERE deleted = 0))",
        [],
    )?;
    for trigger in [
        "clipboard_live_count_insert",
        "clipboard_live_count_delete",
        "clipboard_live_count_update",
    ] {
        conn.execute_batch(&expected_sql("trigger", trigger)?)?;
    }
    Ok(())
}

fn drop_legacy_tables(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch("DROP TABLE IF EXISTS __rusqlite_migrations;")?;
    Ok(())
}

fn rebuild_sync_table(conn: &Connection, table: &str, columns: &[&str]) -> Result<(), StoreError> {
    let expected = expected_sql("table", table)?;
    match schema_sql(conn, "table", table)? {
        Some(existing) if existing == expected => return Ok(()),
        Some(_) => {}
        None => {
            conn.execute_batch(&expected)?;
            return Ok(());
        }
    }

    let old_name = format!("{table}_legacy_v2");
    conn.execute_batch(&format!("ALTER TABLE {table} RENAME TO {old_name};"))?;
    conn.execute_batch(&expected)?;
    let list = columns.join(", ");
    conn.execute_batch(&format!(
        "INSERT INTO {table} ({list}) SELECT {list} FROM {old_name};
         DROP TABLE {old_name};"
    ))?;
    Ok(())
}

fn has_table(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    has_schema_object(conn, "table", name)
}

fn has_index(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    has_schema_object(conn, "index", name)
}

fn has_schema_object(conn: &Connection, kind: &str, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2 LIMIT 1",
        [kind, name],
        |_| Ok(()),
    )
    .optional()
    .map(|found| found.is_some())
}

fn has_column(conn: &Connection, table: &str, name: &str) -> rusqlite::Result<bool> {
    let pragma = format!("PRAGMA table_info('{table}')");
    let mut stmt = conn.prepare(&pragma)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(1)? == name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn schema_sql(conn: &Connection, kind: &str, name: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = ?1 AND name = ?2 LIMIT 1",
        [kind, name],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

fn expected_sql(kind: &str, name: &str) -> rusqlite::Result<String> {
    let expected = Connection::open_in_memory()?;
    expected.execute_batch(super::schema::SCHEMA)?;
    expected.query_row(
        "SELECT sql FROM sqlite_schema WHERE type = ?1 AND name = ?2 LIMIT 1",
        [kind, name],
        |row| row.get::<_, String>(0),
    )
}

fn clipboard_items_select(conn: &Connection, table: &str) -> rusqlite::Result<String> {
    Ok([
        "id".to_string(),
        "content_ciphertext".to_string(),
        "nonce".to_string(),
        "content_type".to_string(),
        "content_hash".to_string(),
        "is_sensitive".to_string(),
        "pinned".to_string(),
        "pin_order".to_string(),
        column_or_literal(conn, table, "pin_updated_at", "0")?,
        "created_at".to_string(),
        "deleted".to_string(),
        column_or_literal(conn, table, "origin_device_id", "''")?,
        column_or_literal(conn, table, "app_bundle_id", "NULL")?,
        column_or_literal(conn, table, "app_name", "NULL")?,
        column_or_literal(conn, table, "payload_metadata", "NULL")?,
        fts_rowid_expr(conn, table)?,
        "LENGTH(COALESCE(content_ciphertext, X''))".to_string(),
    ]
    .join(", "))
}

fn column_or_literal(
    conn: &Connection,
    table: &str,
    name: &str,
    literal: &str,
) -> rusqlite::Result<String> {
    if has_column(conn, table, name)? {
        Ok(name.to_string())
    } else {
        Ok(literal.to_string())
    }
}

fn fts_rowid_expr(conn: &Connection, table: &str) -> rusqlite::Result<String> {
    if has_column(conn, table, "fts_rowid")? {
        Ok(format!(
            "COALESCE(fts_rowid, (SELECT rowid FROM clipboard_fts WHERE clipboard_fts.id = {table}.id))"
        ))
    } else {
        Ok(format!(
            "(SELECT rowid FROM clipboard_fts WHERE clipboard_fts.id = {table}.id)"
        ))
    }
}

fn user_tables(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_schema
          WHERE type = 'table'
            AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
            AND sql IS NOT NULL",
    )?;
    let tables = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tables
        .into_iter()
        .filter(|name| {
            !name.strip_prefix("clipboard_fts_").is_some_and(|suffix| {
                matches!(suffix, "data" | "idx" | "content" | "docsize" | "config")
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::super::connection::{apply_key, validate_key};
    use super::super::test_support::KEY;
    use super::super::Store;
    use super::*;

    const LEGACY_PRE_SENSITIVE_AND_CONTENT_BYTES: &str = r#"
CREATE TABLE clipboard_items (
    id                 TEXT    PRIMARY KEY NOT NULL,
    content_ciphertext BLOB,
    nonce              BLOB,
    content_type       TEXT    NOT NULL,
    content_hash       TEXT    NOT NULL DEFAULT '',
    is_sensitive       INTEGER NOT NULL DEFAULT 0,
    pinned             INTEGER NOT NULL DEFAULT 0,
    pin_order          REAL,
    pin_updated_at     INTEGER NOT NULL DEFAULT 0,
    created_at         INTEGER NOT NULL,
    deleted            INTEGER NOT NULL DEFAULT 0,
    origin_device_id   TEXT    NOT NULL DEFAULT '',
    app_bundle_id      TEXT,
    app_name           TEXT,
    payload_metadata   TEXT,
    fts_rowid          INTEGER
);
CREATE INDEX idx_items_history
    ON clipboard_items(pinned DESC, pin_order, created_at DESC, id DESC)
    WHERE deleted = 0;
CREATE INDEX idx_items_hash
    ON clipboard_items(content_hash, created_at DESC)
    WHERE deleted = 0;
CREATE UNIQUE INDEX idx_items_dedup
    ON clipboard_items(content_hash, created_at / 60000, origin_device_id)
    WHERE deleted = 0 AND content_hash <> '';
CREATE INDEX idx_items_evictable
    ON clipboard_items(created_at, id)
    WHERE deleted = 0 AND pinned = 0;
CREATE INDEX idx_items_unpinned_bytes
    ON clipboard_items(LENGTH(COALESCE(content_ciphertext, X'')))
    WHERE deleted = 0 AND pinned = 0;
CREATE INDEX idx_items_syncable
    ON clipboard_items(created_at, id, content_hash, deleted, origin_device_id,
                       pinned, pin_order, pin_updated_at, is_sensitive)
    WHERE deleted = 1 OR is_sensitive = 0;
CREATE INDEX idx_items_sync_cursor
    ON clipboard_items(MAX(created_at, pin_updated_at), id)
    WHERE deleted = 1 OR is_sensitive = 0;
CREATE TABLE clipboard_live_count (
    only_row INTEGER PRIMARY KEY NOT NULL CHECK (only_row = 0),
    live     INTEGER NOT NULL
);
INSERT INTO clipboard_live_count (only_row, live) VALUES (0, 0);
CREATE TRIGGER clipboard_live_count_insert AFTER INSERT ON clipboard_items
WHEN NEW.deleted = 0
BEGIN
    UPDATE clipboard_live_count SET live = live + 1;
END;
CREATE TRIGGER clipboard_live_count_delete AFTER DELETE ON clipboard_items
WHEN OLD.deleted = 0
BEGIN
    UPDATE clipboard_live_count SET live = live - 1;
END;
CREATE TRIGGER clipboard_live_count_update AFTER UPDATE OF deleted ON clipboard_items
WHEN OLD.deleted <> NEW.deleted
BEGIN
    UPDATE clipboard_live_count
       SET live = live + CASE WHEN NEW.deleted = 0 THEN 1 ELSE -1 END;
END;
CREATE VIRTUAL TABLE clipboard_fts USING fts5(id UNINDEXED, content_text);
CREATE TABLE sync_device_state (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
CREATE TABLE sync_device_name (
    device_id TEXT PRIMARY KEY NOT NULL,
    name      TEXT NOT NULL
);
CREATE TABLE __rusqlite_migrations (
    version INTEGER PRIMARY KEY NOT NULL,
    applied_on TEXT NOT NULL
);
"#;

    const LEGACY_EARLY_V2: &str = r#"
CREATE TABLE clipboard_items (
    id                 TEXT    PRIMARY KEY NOT NULL,
    content_ciphertext BLOB,
    nonce              BLOB,
    content_type       TEXT    NOT NULL,
    content_hash       TEXT    NOT NULL DEFAULT '',
    is_sensitive       INTEGER NOT NULL DEFAULT 0,
    pinned             INTEGER NOT NULL DEFAULT 0,
    pin_order          REAL,
    created_at         INTEGER NOT NULL,
    deleted            INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_items_history
    ON clipboard_items(pinned DESC, pin_order, created_at DESC, id DESC)
    WHERE deleted = 0;
CREATE INDEX idx_items_hash
    ON clipboard_items(content_hash, created_at DESC)
    WHERE deleted = 0;
CREATE UNIQUE INDEX idx_items_dedup
    ON clipboard_items(content_hash, created_at / 60000)
    WHERE deleted = 0 AND content_hash <> '';
CREATE INDEX idx_items_evictable
    ON clipboard_items(created_at, id)
    WHERE deleted = 0 AND pinned = 0;
CREATE INDEX idx_items_unpinned_bytes
    ON clipboard_items(LENGTH(COALESCE(content_ciphertext, X'')))
    WHERE deleted = 0 AND pinned = 0;
CREATE VIRTUAL TABLE clipboard_fts USING fts5(id UNINDEXED, content_text);
CREATE TABLE __rusqlite_migrations (
    version INTEGER PRIMARY KEY NOT NULL,
    applied_on TEXT NOT NULL
);
"#;

    fn create_legacy_db(path: &std::path::Path, schema: &str) {
        let conn = Connection::open(path).unwrap();
        apply_key(&conn, &KEY).unwrap();
        validate_key(&conn).unwrap();
        conn.execute_batch(schema).unwrap();
    }

    fn schema_dump(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT type, name, tbl_name, sql FROM sqlite_schema
                 WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
                 ORDER BY type, name",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok(format!(
                    "{}|{}|{}|{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        rows
    }

    #[test]
    fn open_upgrades_the_august_9_v2_schema_without_losing_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        create_legacy_db(&path, LEGACY_PRE_SENSITIVE_AND_CONTENT_BYTES);
        let conn = Connection::open(&path).unwrap();
        apply_key(&conn, &KEY).unwrap();
        validate_key(&conn).unwrap();
        conn.execute(
            "INSERT INTO clipboard_items (
                 id, content_ciphertext, nonce, content_type, content_hash, is_sensitive,
                 pinned, pin_order, pin_updated_at, created_at, deleted, origin_device_id,
                 app_bundle_id, app_name, payload_metadata, fts_rowid
             ) VALUES (
                 'item-1', X'010203', X'040506', 'text/plain', 'abc', 0,
                 0, NULL, 0, 1234, 0, '', NULL, NULL, NULL, 1
             )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO clipboard_fts(rowid, id, content_text) VALUES (1, 'item-1', 'hello')",
            [],
        )
        .unwrap();
        drop(conn);

        let mut conn = Connection::open(&path).unwrap();
        apply_key(&conn, &KEY).unwrap();
        validate_key(&conn).unwrap();
        upgrade_if_legacy_v2(&mut conn).unwrap();
        let dump = schema_dump(&conn);
        let expected = {
            let expected = Connection::open_in_memory().unwrap();
            expected
                .execute_batch(super::super::schema::SCHEMA)
                .unwrap();
            schema_dump(&expected)
        };
        assert_eq!(dump, expected, "upgraded schema drifted");
        drop(conn);

        let store = Store::open(&path, &KEY).unwrap();
        super::super::schema_verify::verify_schema(&store.conn().unwrap()).unwrap();
        let upgraded = store.get("item-1").unwrap().unwrap();
        assert_eq!(upgraded.content_ciphertext, vec![1, 2, 3]);
        let bytes: i64 = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT content_bytes FROM clipboard_items WHERE id = 'item-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bytes, 3);
        let legacy_count: i64 = store
            .conn()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name = '__rusqlite_migrations'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy_count, 0);
    }

    #[test]
    fn open_upgrades_early_v2_columns_and_indexes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        create_legacy_db(&path, LEGACY_EARLY_V2);

        let mut conn = Connection::open(&path).unwrap();
        apply_key(&conn, &KEY).unwrap();
        validate_key(&conn).unwrap();
        upgrade_if_legacy_v2(&mut conn).unwrap();
        let dump = schema_dump(&conn);
        let expected = {
            let expected = Connection::open_in_memory().unwrap();
            expected
                .execute_batch(super::super::schema::SCHEMA)
                .unwrap();
            schema_dump(&expected)
        };
        assert_eq!(dump, expected, "upgraded schema drifted");
        drop(conn);

        let store = Store::open(&path, &KEY).unwrap();
        super::super::schema_verify::verify_schema(&store.conn().unwrap()).unwrap();
    }

    #[test]
    fn open_still_refuses_current_schema_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        store
            .conn()
            .unwrap()
            .execute_batch("DROP TRIGGER clipboard_live_count_insert")
            .unwrap();
        drop(store);

        let error = Store::open(&path, &KEY).unwrap_err();
        assert!(matches!(error, StoreError::InvalidSchema));
    }

    #[test]
    #[ignore = "requires a copied live db path and device secret"]
    fn copied_live_db_upgrades_and_reopens() {
        let live = std::env::var("COPYPASTE_LIVE_DB").expect("COPYPASTE_LIVE_DB");
        let secret_hex =
            std::env::var("COPYPASTE_LIVE_SECRET_HEX").expect("COPYPASTE_LIVE_SECRET_HEX");
        let secret_bytes = hex::decode(secret_hex).expect("hex device secret");
        let secret: [u8; 32] = secret_bytes.as_slice().try_into().expect("32-byte secret");
        let key = crate::Keyring::from_secret(&secret).db_key();
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("copypaste-v2.db");
        std::fs::copy(live, &copy).unwrap();

        let conn = Connection::open(&copy).unwrap();
        apply_key(&conn, &key).unwrap();
        validate_key(&conn).unwrap();
        assert!(
            looks_like_legacy_v2(&conn).unwrap(),
            "live db was not a recognized v2 legacy schema"
        );
        drop(conn);

        let store = Store::open(&copy, &key).unwrap();
        super::super::schema_verify::verify_schema(&store.conn().unwrap()).unwrap();
    }
}
