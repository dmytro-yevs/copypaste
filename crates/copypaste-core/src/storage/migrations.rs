//! Ordered, transactional schema changes. Unknown schemas fail without writes.

use rusqlite::Connection;

use super::{schema_verify, source_icons, StoreError};

pub(super) const LATEST_VERSION: u32 = MIGRATIONS[MIGRATIONS.len() - 1].version;
const INITIAL_SCHEMA: &str = include_str!("migrations/000_initial.sql");

struct Migration {
    version: u32,
    sql: &'static str,
    transform: fn(&Connection) -> Result<(), StoreError>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("migrations/001_source_icons.sql"),
        transform: share_source_icons,
    },
    Migration {
        version: 2,
        sql: include_str!("migrations/002_semantic_search.sql"),
        transform: |_| Ok(()),
    },
    Migration {
        version: 3,
        sql: include_str!("migrations/003_file_import.sql"),
        transform: |_| Ok(()),
    },
];

pub(super) fn upgrade(conn: &mut Connection) -> Result<(), StoreError> {
    run_registered(conn, MIGRATIONS)
}

fn run_registered(conn: &mut Connection, migrations: &[Migration]) -> Result<(), StoreError> {
    let version: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let latest = migrations.last().map_or(0, |step| step.version);
    if version > latest {
        return Err(StoreError::InvalidSchema);
    }
    let mut applied = if has_ledger(conn)? {
        let mut statement =
            conn.prepare("SELECT version FROM copypaste_schema_migrations ORDER BY version")?;
        let recorded = statement
            .query_map([], |row| row.get::<_, u32>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if recorded.last().copied().unwrap_or(0) != version {
            return Err(StoreError::InvalidSchema);
        }
        recorded
    } else {
        migrations
            .iter()
            .filter(|step| step.version <= version)
            .map(|step| step.version)
            .collect()
    };
    schema_verify::verify_schema_against(conn, &schema_for(&applied, migrations)?)?;
    let missing = migrations
        .iter()
        .filter(|step| !applied.contains(&step.version))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    super::dbfile::verify_integrity(conn)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    for migration in missing {
        tx.execute_batch(migration.sql)?;
        (migration.transform)(&tx)?;
        applied.push(migration.version);
    }
    record_versions(&tx, &applied)?;
    tx.pragma_update(None, "user_version", latest)?;
    schema_verify::verify_schema_against(&tx, &schema_for(&applied, migrations)?)?;
    tx.commit()?;
    Ok(())
}

fn has_ledger(conn: &Connection) -> Result<bool, StoreError> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'copypaste_schema_migrations')", [], |row| row.get(0))?)
}

fn record_versions(conn: &Connection, versions: &[u32]) -> Result<(), StoreError> {
    if has_ledger(conn)? {
        for version in versions {
            conn.execute(
                "INSERT OR IGNORE INTO copypaste_schema_migrations (version) VALUES (?1)",
                [version],
            )?;
        }
    }
    Ok(())
}

pub(super) fn record_current(conn: &Connection) -> Result<(), StoreError> {
    record_versions(
        conn,
        &MIGRATIONS
            .iter()
            .map(|step| step.version)
            .collect::<Vec<_>>(),
    )
}

fn schema_for(versions: &[u32], migrations: &[Migration]) -> Result<String, StoreError> {
    if versions
        .iter()
        .any(|version| !migrations.iter().any(|step| step.version == *version))
    {
        return Err(StoreError::InvalidSchema);
    }
    let mut schema = INITIAL_SCHEMA.to_string();
    for migration in migrations
        .iter()
        .filter(|step| versions.contains(&step.version))
    {
        schema.push_str(migration.sql);
    }
    Ok(schema)
}

#[cfg(test)]
fn schema_at(version: u32) -> String {
    schema_for(
        &MIGRATIONS
            .iter()
            .filter(|step| step.version <= version)
            .map(|step| step.version)
            .collect::<Vec<_>>(),
        MIGRATIONS,
    )
    .unwrap()
}

fn share_source_icons(conn: &Connection) -> Result<(), StoreError> {
    let mut after = String::new();
    loop {
        let rows = {
            let mut stmt = conn.prepare("SELECT id, app_bundle_id, content_type, payload_metadata, created_at FROM clipboard_items \
                WHERE id > ?1 AND payload_metadata IS NOT NULL ORDER BY id LIMIT 100")?;
            let values = stmt
                .query_map([&after], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            values
        };
        if rows.is_empty() {
            break;
        }
        for (id, app_id, content_type, metadata, captured_at) in rows {
            let (metadata, icon_id) = source_icons::normalise(
                conn,
                app_id.as_deref(),
                &content_type,
                captured_at,
                metadata.as_deref(),
            )?;
            conn.execute("UPDATE clipboard_items SET payload_metadata = ?2, source_icon_id = ?3, \
                content_bytes = LENGTH(COALESCE(content_ciphertext, X'')) + LENGTH(COALESCE(?2, '')) WHERE id = ?1",
                rusqlite::params![id, metadata, icon_id])?;
            after = id;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::source_icons::tests::icon;
    use super::*;

    #[test]
    fn migration_preserves_history_and_deduplicates_existing_icons() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(INITIAL_SCHEMA).unwrap();
        let metadata = crate::PayloadMetadata {
            file: None,
            source_app_icon: Some(icon()),
        }
        .to_json("text")
        .unwrap();
        for id in ["first", "second"] {
            conn.execute("INSERT INTO clipboard_items (id, content_ciphertext, nonce, content_type, content_hash, created_at, pinned, app_bundle_id, payload_metadata) \
                VALUES (?1, X'010203', X'04', 'text', ?1, 123, 1, 'com.example.editor', ?2)", rusqlite::params![id, metadata]).unwrap();
        }
        upgrade(&mut conn).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            LATEST_VERSION
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM source_app_icons", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM clipboard_items WHERE payload_metadata IS NULL AND \
            source_icon_id = 'app:com.example.editor' AND content_ciphertext = X'010203' AND nonce = X'04' AND created_at = 123 AND pinned = 1", [], |r| r.get::<_, u32>(0)).unwrap(), 2);
        upgrade(&mut conn).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM source_app_icons", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn failed_data_transformation_rolls_back_ddl_and_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(INITIAL_SCHEMA).unwrap();
        let metadata = crate::PayloadMetadata {
            file: None,
            source_app_icon: Some(icon()),
        }
        .to_json("text")
        .unwrap();
        conn.execute("INSERT INTO clipboard_items (id, content_type, content_hash, created_at, payload_metadata) VALUES ('one', 'text', '', 0, ?1)", [&metadata]).unwrap();
        fn abort_after_transform(conn: &Connection) -> Result<(), StoreError> {
            share_source_icons(conn)?;
            Err(StoreError::InvalidSchema)
        }
        let failing = [Migration {
            version: 1,
            sql: MIGRATIONS[0].sql,
            transform: abort_after_transform,
        }];
        assert!(run_registered(&mut conn, &failing).is_err());
        schema_verify::verify_schema_against(&conn, INITIAL_SCHEMA).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT payload_metadata FROM clipboard_items WHERE id = 'one'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            metadata
        );
    }

    #[test]
    fn version_one_history_survives_upgrade_to_version_three() {
        use super::super::{connection::apply_key, Store};
        assert_eq!(LATEST_VERSION, 3);
        let keyring = crate::Keyring::from_secret(&[37; 32]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let conn = Connection::open(&path).unwrap();
        apply_key(&conn, &keyring.db_key()).unwrap();
        conn.execute_batch(&schema_at(1)).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        let (nonce, ciphertext) =
            crate::encrypt(b"retained history", &keyring.item_key(), "existing").unwrap();
        conn.execute("INSERT INTO clipboard_items (id, content_ciphertext, nonce, content_type, content_hash, created_at) VALUES ('existing', ?1, ?2, 'text', 'existing-hash', 1)", rusqlite::params![ciphertext, nonce]).unwrap();
        drop(conn);
        let store = Store::open(&path, &keyring.db_key()).unwrap();
        let row = store.get("existing").unwrap().unwrap();
        assert_eq!(
            &*crate::decrypt(
                &row.content_ciphertext,
                &row.nonce,
                &keyring.item_key(),
                &row.id
            )
            .unwrap(),
            b"retained history"
        );
        assert_eq!(
            store
                .conn()
                .unwrap()
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            3
        );
        schema_verify::verify_schema(&store.conn().unwrap()).unwrap();
        drop(store);
        assert_eq!(
            Store::open(&path, &keyring.db_key())
                .unwrap()
                .count()
                .unwrap(),
            1
        );
    }

    #[test]
    fn a_later_registered_version_two_is_applied_after_version_three() {
        let first = [
            Migration {
                version: 1,
                sql: MIGRATIONS[0].sql,
                transform: share_source_icons,
            },
            Migration {
                version: 3,
                sql: MIGRATIONS
                    .iter()
                    .find(|step| step.version == 3)
                    .unwrap()
                    .sql,
                transform: |_| Ok(()),
            },
        ];
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&schema_for(&[1], &first).unwrap())
            .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO clipboard_items (id, content_type, content_hash, created_at) VALUES ('keep', 'text', 'keep-hash', 1)", []).unwrap();
        run_registered(&mut conn, &first).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            3
        );
        let later = [
            Migration {
                version: 1,
                sql: first[0].sql,
                transform: share_source_icons,
            },
            Migration {
                version: 2,
                sql: "CREATE TABLE late_search_state (id INTEGER PRIMARY KEY NOT NULL);",
                transform: |_| Ok(()),
            },
            Migration {
                version: 3,
                sql: first[1].sql,
                transform: |_| Ok(()),
            },
        ];
        run_registered(&mut conn, &later).unwrap();
        conn.execute("INSERT INTO late_search_state (id) VALUES (7)", [])
            .unwrap();
        run_registered(&mut conn, &later).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM late_search_state WHERE id = 7",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id = 'keep'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM copypaste_schema_migrations",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            3
        );
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            3
        );
        conn.execute_batch("DROP INDEX idx_items_history").unwrap();
        assert!(matches!(
            run_registered(&mut conn, &later),
            Err(StoreError::InvalidSchema)
        ));
    }

    #[test]
    fn opening_an_existing_keyed_database_migrates_and_reopens() {
        use super::super::{connection::apply_key, test_support::KEY, Store};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let conn = Connection::open(&path).unwrap();
        apply_key(&conn, &KEY).unwrap();
        conn.execute_batch(INITIAL_SCHEMA).unwrap();
        let metadata = crate::PayloadMetadata {
            file: None,
            source_app_icon: Some(icon()),
        }
        .to_json("text")
        .unwrap();
        conn.execute("INSERT INTO clipboard_items (id, content_type, content_hash, created_at, app_bundle_id, payload_metadata) VALUES ('one', 'text', '', 42, 'com.example.editor', ?1)", [&metadata]).unwrap();
        drop(conn);
        let store = Store::open(&path, &KEY).unwrap();
        let item = store.get("one").unwrap().unwrap();
        assert_eq!(item.created_at, 42);
        assert_eq!(
            item.source_icon_id.as_deref(),
            Some("app:com.example.editor")
        );
        assert_eq!(
            store
                .source_app_icon_by_id(item.source_icon_id.as_ref().unwrap())
                .unwrap(),
            Some(icon())
        );
        drop(store);
        Store::open(&path, &KEY).unwrap();
    }

    #[test]
    fn future_versions_and_tampered_baselines_are_rejected_without_changes() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(INITIAL_SCHEMA).unwrap();
        conn.pragma_update(None, "user_version", LATEST_VERSION + 1)
            .unwrap();
        assert!(matches!(upgrade(&mut conn), Err(StoreError::InvalidSchema)));
        schema_verify::verify_schema_against(&conn, INITIAL_SCHEMA).unwrap();
        conn.pragma_update(None, "user_version", 0).unwrap();
        conn.execute_batch("DROP INDEX idx_items_history").unwrap();
        assert!(matches!(upgrade(&mut conn), Err(StoreError::InvalidSchema)));
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'source_app_icons'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
    }
}
