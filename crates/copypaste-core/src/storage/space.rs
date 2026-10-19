//! Return unused encrypted pages without rebuilding a live store or polling.

use std::sync::atomic::Ordering;
use std::time::Duration;

use rusqlite::Connection;

use super::{connection::run_pragma, Store};

const LEGACY_RECLAIM_BYTES: u64 = 8 * 1024 * 1024;
const PAGES_PER_BATCH: u64 = 256;

/// Every published Store keeps a shared lease. Legacy conversion needs the
/// exclusive lease, including against other handles in this process. The OS
/// releases it after a crash; the empty coordination file is never removed.
pub(super) fn lock_store(path: &std::path::Path) -> std::io::Result<(std::fs::File, bool)> {
    let mut lock_path = path.as_os_str().to_owned();
    lock_path.push(".space-lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    // Fresh-file publication already has a concurrent-winner protocol. Both
    // creators can share this lease because new files need no VACUUM.
    if !path.try_exists()? {
        file.lock_shared()?;
        return Ok((file, false));
    }
    match file.try_lock() {
        Ok(()) => Ok((file, true)),
        Err(std::fs::TryLockError::WouldBlock) => {
            file.lock_shared()?;
            Ok((file, false))
        }
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

/// Existing databases need a one-time rebuild to acquire pointer-map pages.
/// Run only before the runtime publishes this Store, never after a mutation:
/// VACUUM may renumber implicit rowids used by pending clear-history actions.
/// Avoid rebuilding a large, mostly occupied file merely to change its mode.
pub(super) fn prepare(conn: &Connection) -> rusqlite::Result<()> {
    let mode: u32 = conn.pragma_query_value(None, "auto_vacuum", |r| r.get(0))?;
    if mode == 0 {
        let free: u64 = conn.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
        let pages: u64 = conn.pragma_query_value(None, "page_count", |r| r.get(0))?;
        // SQLCipher exposes this pragma as TEXT; plain SQLite returns INTEGER.
        let page_size: u64 = conn.pragma_query_value(None, "page_size", |r| {
            match r.get::<_, rusqlite::types::Value>(0)? {
                rusqlite::types::Value::Integer(value) if value > 0 => Ok(value as u64),
                rusqlite::types::Value::Text(value) => {
                    value.parse().map_err(|_| rusqlite::Error::InvalidQuery)
                }
                _ => Err(rusqlite::Error::InvalidQuery),
            }
        })?;
        if free.saturating_mul(page_size) < LEGACY_RECLAIM_BYTES || free < pages / 4 {
            return Ok(());
        }
        conn.pragma_update(None, "auto_vacuum", 2)?;
        run_pragma(conn, "VACUUM")?;
    }
    reclaim(conn)
}

fn reclaim(conn: &Connection) -> rusqlite::Result<()> {
    // Maintenance yields to captures, sync and readers. A later deletion or
    // store open retries deferred work; no timer or permanent worker exists.
    conn.busy_timeout(Duration::ZERO)?;
    let result = (|| {
        let mode: u32 = conn.pragma_query_value(None, "auto_vacuum", |r| r.get(0))?;
        if mode != 2 {
            return Ok(());
        }
        loop {
            let before: u64 = conn.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
            if before == 0 {
                break;
            }
            run_pragma(
                conn,
                &format!("PRAGMA incremental_vacuum({PAGES_PER_BATCH})"),
            )?;
            let after: u64 = conn.pragma_query_value(None, "freelist_count", |r| r.get(0))?;
            if after >= before {
                break;
            }
            std::thread::yield_now();
        }
        // A busy reader leaves the WAL intact. Never wait for it or delete
        // journal files directly. SQLite reports that case in the result row.
        run_pragma(conn, "PRAGMA wal_checkpoint(TRUNCATE)")
    })();
    let restored = conn.busy_timeout(Duration::from_secs(5));
    result.and(restored)
}

impl Store {
    pub(super) fn request_space_reclaim(&self) {
        if !self.incremental_reclaim {
            return;
        }
        self.reclaim_requested.store(true, Ordering::Release);
        if self
            .reclaiming
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let store = self.clone();
        let result = std::thread::Builder::new()
            .name("history-space".into())
            .spawn(move || {
                store.reclaim_requested.store(false, Ordering::Release);
                let result = store
                    .conn()
                    .and_then(|conn| reclaim(&conn).map_err(Into::into));
                if let Err(error) = &result {
                    tracing::debug!(%error, "deferred history space maintenance");
                }
                store.reclaiming.store(false, Ordering::Release);
                // A deletion arriving after the last freelist read must not
                // disappear merely because it was coalesced with this worker.
                if result.is_ok() && store.reclaim_requested.swap(false, Ordering::AcqRel) {
                    store.request_space_reclaim();
                }
            });
        if result.is_err() {
            self.reclaiming.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{item, KEY, T0};

    fn bytes(path: &std::path::Path) -> u64 {
        std::fs::metadata(path).unwrap().len()
            + std::fs::metadata(format!("{}-wal", path.display())).map_or(0, |m| m.len())
    }

    fn store_mode(path: &std::path::Path) -> u32 {
        let conn = crate::storage::dbfile::open_validated(path, &KEY).unwrap();
        conn.pragma_query_value(None, "auto_vacuum", |r| r.get(0))
            .unwrap()
    }
    fn store_free(path: &std::path::Path) -> u64 {
        let conn = crate::storage::dbfile::open_validated(path, &KEY).unwrap();
        conn.pragma_query_value(None, "freelist_count", |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn incremental_reclaim_shrinks_disk_and_preserves_pins_search_and_clear_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let store = Store::open(&path, &KEY).unwrap();
        // Drive maintenance synchronously so assertions observe its completion.
        store.reclaiming.store(true, Ordering::Release);
        let pinned = store.insert(item("retained pin", T0)).unwrap();
        store.set_pinned(&pinned.id, true).unwrap();
        let mut large = item("discarded large clip", T0 + 1);
        large.content_ciphertext = vec![42; 12 * 1024 * 1024];
        store.insert(large).unwrap();
        let through = store.max_rowid().unwrap();
        let later = store.insert(item("later capture", T0 + 2)).unwrap();
        let conn = store.conn().unwrap();
        run_pragma(&conn, "PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        let before = bytes(&path);
        assert_eq!(store.delete_all_through(through).unwrap(), 1);
        reclaim(&conn).unwrap();
        assert!(
            bytes(&path) + 8 * 1024 * 1024 < before,
            "before={before}, after={}, mode={}, free={}",
            bytes(&path),
            store_mode(&path),
            store_free(&path)
        );
        assert!(store.get(&pinned.id).unwrap().unwrap().pinned);
        assert!(store.get(&later.id).unwrap().is_some());
        assert_eq!(store.max_rowid().unwrap(), through + 1);
        assert_eq!(store.search("retained", 10).unwrap().len(), 1);
        assert!(store.search("discarded", 10).unwrap().is_empty());
        crate::storage::dbfile::verify_integrity(&conn).unwrap();
        drop(conn);
        drop(store);
        let reopened = Store::open(&path, &KEY).unwrap();
        assert_eq!(reopened.count().unwrap(), 2);
        assert_eq!(reopened.search("retained", 10).unwrap().len(), 1);
    }

    #[test]
    fn legacy_conversion_is_deferred_until_open_and_preserves_versions_and_search() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.db");
        let store = Store::open(&path, &KEY).unwrap();
        store.reclaiming.store(true, Ordering::Release);
        {
            let conn = store.conn().unwrap();
            conn.pragma_update(None, "auto_vacuum", 0).unwrap();
            run_pragma(&conn, "VACUUM").unwrap();
        }
        let pin = store.insert(item("legacy pin", T0)).unwrap();
        store.set_pinned(&pin.id, true).unwrap();
        let mut large = item("legacy removed", T0 + 1);
        large.content_ciphertext = vec![42; 12 * 1024 * 1024];
        let removed = store.insert(large).unwrap();
        store.delete(&removed.id).unwrap();
        let conn = store.conn().unwrap();
        run_pragma(&conn, "PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "auto_vacuum", |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
        let before = bytes(&path);
        drop(conn);
        drop(store);
        let reopened = Store::open(&path, &KEY).unwrap();
        assert!(
            bytes(&path) + 8 * 1024 * 1024 < before,
            "before={before}, after={}, mode={}, free={}",
            bytes(&path),
            store_mode(&path),
            store_free(&path)
        );
        assert!(reopened.get(&pin.id).unwrap().unwrap().pinned);
        assert!(reopened.version(&removed.id).unwrap().unwrap().deleted);
        assert_eq!(reopened.search("legacy", 10).unwrap().len(), 1);
        assert!(reopened.incremental_reclaim);
    }

    #[test]
    fn another_handle_cannot_vacuum_while_a_clear_boundary_is_live() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.db");
        let first = Store::open(&path, &KEY).unwrap();
        first.reclaiming.store(true, Ordering::Release);
        let mut large = item("removed", T0);
        large.content_ciphertext = vec![42; 12 * 1024 * 1024];
        let removed = first.insert(large).unwrap();
        let keep = first.insert(item("keep", T0 + 1)).unwrap();
        let through = first.max_rowid().unwrap();
        {
            let conn = first.conn().unwrap();
            conn.pragma_update(None, "auto_vacuum", 0).unwrap();
            run_pragma(&conn, "VACUUM").unwrap();
        }
        first.delete(&removed.id).unwrap();
        let later = first.insert(item("after boundary", T0 + 2)).unwrap();
        let second = Store::open(&path, &KEY).unwrap();
        assert!(
            !second.incremental_reclaim,
            "legacy conversion must wait for every live Store"
        );
        assert_eq!(second.max_rowid().unwrap(), through + 1);
        assert_eq!(first.delete_all_through(through).unwrap(), 1);
        assert!(first.get(&keep.id).unwrap().is_none());
        assert!(first.get(&later.id).unwrap().is_some());
    }
}
