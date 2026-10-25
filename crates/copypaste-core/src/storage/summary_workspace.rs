//! Protected, session-scoped P2P summary storage.

use std::path::{Path, PathBuf};

use copypaste_p2p::protocol::ItemSummary;
use copypaste_p2p::sync::{MemorySummaryWorkspace, SummaryWorkspace, SyncError};
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use zeroize::Zeroizing;

use super::Store;

// Account conservatively for maps, remote rows and the later wanted map. A
// page itself is bounded by the existing wire contract and is transient.
const MEMORY_BUDGET: usize = 4 * 1024 * 1024;
const ERROR: &str = "the sync session workspace is unavailable";

fn failed(error: impl std::error::Error + 'static) -> SyncError {
    let error = &error as &(dyn std::error::Error + 'static);
    let code = if let Some(error) = error.downcast_ref::<rusqlite::Error>() {
        crate::sync::diagnostics::sqlite(error)
    } else if let Some(error) = error.downcast_ref::<std::io::Error>() {
        crate::sync::diagnostics::io(error)
    } else if let Some(error) = error.downcast_ref::<crate::StoreError>() {
        crate::sync::diagnostics::store(error)
    } else if error.is::<rand::Error>() {
        copypaste_ipc::sync_issue::SyncIssueCode::EncryptionUnavailable
    } else if error.is::<serde_json::Error>() {
        copypaste_ipc::sync_issue::SyncIssueCode::LocalPayloadInvalid
    } else {
        copypaste_ipc::sync_issue::SyncIssueCode::HistoryUnavailable
    };
    SyncError::Diagnostic(code)
}

impl Store {
    pub(crate) fn summary_workspace(&self) -> Box<dyn SummaryWorkspace> {
        let parent = self
            .path
            .as_deref()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        Box::new(SpillingWorkspace {
            parent,
            retained: 0,
            inner: Workspace::Memory(MemorySummaryWorkspace::default()),
        })
    }
}

enum Workspace {
    Memory(MemorySummaryWorkspace),
    Disk(DiskWorkspace),
    // A failed spill is terminal for the session.
    Failed,
}

struct SpillingWorkspace {
    parent: Option<PathBuf>,
    retained: usize,
    inner: Workspace,
}

impl SpillingWorkspace {
    fn page_cost(items: &[ItemSummary]) -> usize {
        items.iter().fold(0usize, |n, item| {
            let size = std::mem::size_of::<ItemSummary>()
                .saturating_add(item.item_id.len())
                .saturating_add(item.content_hash.len())
                .saturating_add(item.origin_device_id.len())
                .saturating_add(128);
            n.saturating_add(size.saturating_mul(3))
        })
    }
    fn backend(&mut self) -> Result<&mut dyn SummaryWorkspace, SyncError> {
        match &mut self.inner {
            Workspace::Memory(m) => Ok(m),
            Workspace::Disk(d) => Ok(d),
            Workspace::Failed => Err(SyncError::Source(ERROR.into())),
        }
    }

    fn reserve_page(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        let cost = Self::page_cost(items);
        self.retained = self.retained.saturating_add(cost);
        if self.retained <= MEMORY_BUDGET || !matches!(self.inner, Workspace::Memory(_)) {
            return Ok(());
        }
        let old = std::mem::replace(&mut self.inner, Workspace::Failed);
        let Workspace::Memory(memory) = old else {
            unreachable!()
        };
        let mut disk = DiskWorkspace::new(self.parent.as_deref())?;
        memory.spill_into(&mut disk)?;
        self.inner = Workspace::Disk(disk);
        Ok(())
    }
}

impl SummaryWorkspace for SpillingWorkspace {
    fn remote_wins(&mut self, item: &ItemSummary) -> Result<bool, SyncError> {
        self.backend()?.remote_wins(item)
    }
    fn consider_wanted(&mut self, item: &ItemSummary) -> Result<bool, SyncError> {
        self.backend()?.consider_wanted(item)
    }
    fn may_block(&self, incoming: &[ItemSummary]) -> bool {
        !matches!(self.inner, Workspace::Memory(_))
            || self.retained.saturating_add(Self::page_cost(incoming)) > MEMORY_BUDGET
    }
    fn add_advertised(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        self.reserve_page(items)?;
        self.backend()?.add_advertised(items)
    }
    fn add_comparison(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        self.reserve_page(items)?;
        self.backend()?.add_comparison(items)
    }
    fn add_remote(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        self.reserve_page(items)?;
        self.backend()?.add_remote(items)
    }
    fn remote_page(
        &mut self,
        after: u64,
        limit: usize,
    ) -> Result<Vec<(u64, ItemSummary)>, SyncError> {
        self.backend()?.remote_page(after, limit)
    }
    fn advertised(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        self.backend()?.advertised(id)
    }
    fn comparison(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        self.backend()?.comparison(id)
    }
    fn wanted(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        self.backend()?.wanted(id)
    }
    fn set_wanted(&mut self, item: &ItemSummary) -> Result<(), SyncError> {
        self.backend()?.set_wanted(item)
    }
    fn defer(&mut self, id: &str) -> Result<(), SyncError> {
        self.backend()?.defer(id)
    }
    fn is_deferred(&mut self, id: &str) -> Result<bool, SyncError> {
        self.backend()?.is_deferred(id)
    }
    fn finish_plan(&mut self, limit: usize) -> Result<(Vec<ItemSummary>, usize), SyncError> {
        self.backend()?.finish_plan(limit)
    }
}

struct DiskWorkspace {
    // Connection drops before the directory, including on Windows.
    conn: Connection,
    _directory: super::summary_workspace_cleanup::WorkspaceDirectory,
    planning: bool,
}

impl DiskWorkspace {
    fn new(parent: Option<&Path>) -> Result<Self, SyncError> {
        let directory = super::summary_workspace_cleanup::create(parent).map_err(failed)?;
        let conn = Connection::open(directory.path().join("workspace.db")).map_err(failed)?;
        let mut key = Zeroizing::new([0u8; 32]);
        OsRng.try_fill_bytes(&mut *key).map_err(failed)?;
        super::connection::apply_key(&conn, &key).map_err(failed)?;
        let cipher: String = conn
            .query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .map_err(failed)?;
        if cipher.is_empty() {
            return Err(SyncError::Source(ERROR.into()));
        }
        conn.execute_batch(
            "PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA temp_store=MEMORY; PRAGMA cache_size=-512;
             CREATE TABLE advertised (id TEXT PRIMARY KEY, body BLOB NOT NULL) WITHOUT ROWID;
             CREATE TABLE comparison (id TEXT PRIMARY KEY, body BLOB NOT NULL) WITHOUT ROWID;
             CREATE TABLE remote (seq INTEGER PRIMARY KEY, body BLOB NOT NULL);
             CREATE TABLE wanted (id TEXT PRIMARY KEY, stamp INTEGER NOT NULL, body BLOB NOT NULL) WITHOUT ROWID;
             CREATE INDEX wanted_age ON wanted(stamp DESC, id ASC);
             CREATE TABLE deferred (id TEXT PRIMARY KEY) WITHOUT ROWID;"
        ).map_err(failed)?;
        Ok(Self {
            conn,
            _directory: directory,
            planning: false,
        })
    }

    fn begin_plan(&mut self) -> Result<(), SyncError> {
        if !self.planning {
            self.conn.execute_batch("BEGIN").map_err(failed)?;
            self.planning = true;
        }
        Ok(())
    }

    fn get(&self, sql: &str, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        let body: Option<Vec<u8>> = self
            .conn
            .query_row(sql, [id], |r| r.get(0))
            .optional()
            .map_err(failed)?;
        body.map(|b| serde_json::from_slice(&b).map_err(failed))
            .transpose()
    }
}

impl SummaryWorkspace for DiskWorkspace {
    fn add_advertised(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        let tx = self.conn.transaction().map_err(failed)?;
        {
            let mut stmt = tx
                .prepare_cached("INSERT OR REPLACE INTO advertised VALUES(?1, ?2)")
                .map_err(failed)?;
            for item in items {
                let body = serde_json::to_vec(item).map_err(failed)?;
                stmt.execute(params![item.item_id, body]).map_err(failed)?;
            }
        }
        tx.commit().map_err(failed)
    }
    fn add_comparison(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        let tx = self.conn.transaction().map_err(failed)?;
        {
            let mut stmt = tx
                .prepare_cached("INSERT OR REPLACE INTO comparison VALUES(?1, ?2)")
                .map_err(failed)?;
            for item in items {
                let body = serde_json::to_vec(item).map_err(failed)?;
                stmt.execute(params![item.item_id, body]).map_err(failed)?;
            }
        }
        tx.commit().map_err(failed)
    }
    fn add_remote(&mut self, items: &[ItemSummary]) -> Result<(), SyncError> {
        let tx = self.conn.transaction().map_err(failed)?;
        {
            let mut stmt = tx
                .prepare_cached("INSERT INTO remote(body) VALUES(?1)")
                .map_err(failed)?;
            for item in items {
                let body = serde_json::to_vec(item).map_err(failed)?;
                stmt.execute([body]).map_err(failed)?;
            }
        }
        tx.commit().map_err(failed)
    }
    fn remote_page(
        &mut self,
        after: u64,
        limit: usize,
    ) -> Result<Vec<(u64, ItemSummary)>, SyncError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT seq, body FROM remote WHERE seq > ?1 ORDER BY seq LIMIT ?2")
            .map_err(failed)?;
        let rows = stmt
            .query_map(params![after as i64, limit as i64], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(failed)?;
        rows.map(|row| {
            let (seq, body) = row.map_err(failed)?;
            Ok((seq as u64, serde_json::from_slice(&body).map_err(failed)?))
        })
        .collect()
    }
    fn advertised(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        self.get("SELECT body FROM advertised WHERE id=?1", id)
    }
    fn comparison(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        self.get("SELECT body FROM comparison WHERE id=?1", id)
    }
    fn wanted(&mut self, id: &str) -> Result<Option<ItemSummary>, SyncError> {
        self.get("SELECT body FROM wanted WHERE id=?1", id)
    }
    fn set_wanted(&mut self, item: &ItemSummary) -> Result<(), SyncError> {
        self.begin_plan()?;
        let body = serde_json::to_vec(item).map_err(failed)?;
        self.conn
            .execute(
                "INSERT OR REPLACE INTO wanted VALUES(?1,?2,?3)",
                params![item.item_id, item.created_at, body],
            )
            .map_err(failed)?;
        Ok(())
    }
    fn defer(&mut self, id: &str) -> Result<(), SyncError> {
        self.begin_plan()?;
        self.conn
            .execute("INSERT OR IGNORE INTO deferred VALUES(?1)", [id])
            .map_err(failed)?;
        Ok(())
    }
    fn is_deferred(&mut self, id: &str) -> Result<bool, SyncError> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM deferred WHERE id=?1)",
                [id],
                |r| r.get(0),
            )
            .map_err(failed)
    }
    fn finish_plan(&mut self, limit: usize) -> Result<(Vec<ItemSummary>, usize), SyncError> {
        self.begin_plan()?;
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM wanted", [], |r| r.get(0))
            .map_err(failed)?;
        self.conn.execute("INSERT OR IGNORE INTO deferred SELECT id FROM wanted WHERE id NOT IN (SELECT id FROM wanted ORDER BY stamp DESC, id ASC LIMIT ?1)", [limit as i64]).map_err(failed)?;
        let selected = {
            let mut stmt = self
                .conn
                .prepare_cached("SELECT body FROM wanted ORDER BY stamp DESC, id ASC LIMIT ?1")
                .map_err(failed)?;
            let rows = stmt
                .query_map([limit as i64], |r| r.get::<_, Vec<u8>>(0))
                .map_err(failed)?;
            rows.map(|row| serde_json::from_slice(&row.map_err(failed)?).map_err(failed))
                .collect::<Result<Vec<_>, _>>()?
        };
        self.conn.execute_batch("COMMIT").map_err(failed)?;
        self.planning = false;
        Ok((selected, (total as usize).saturating_sub(limit)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: String, stamp: i64) -> ItemSummary {
        ItemSummary {
            item_id: id,
            created_at: stamp,
            deleted: false,
            content_hash: "a".repeat(64),
            origin_device_id: "private-origin-sentinel".into(),
            pinned: false,
            pin_order: None,
            pin_updated_at: stamp,
        }
    }

    #[test]
    fn spill_preserves_snapshot_order_and_cleans_encrypted_storage() {
        let root = tempfile::tempdir().unwrap();
        let mut workspace = SpillingWorkspace {
            parent: Some(root.path().to_owned()),
            retained: 0,
            inner: Workspace::Memory(MemorySummaryWorkspace::default()),
        };
        let items: Vec<_> = (0..3000).map(|i| item(format!("id-{i}"), i)).collect();
        workspace.add_advertised(&items).unwrap();
        workspace.add_comparison(&items).unwrap();
        workspace.add_remote(&items).unwrap();
        assert!(matches!(workspace.inner, Workspace::Disk(_)));
        let workspace_root = root.path().join(".copypaste-sync-workspaces");
        let path = std::fs::read_dir(&workspace_root)
            .unwrap()
            .find(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("session-")
            })
            .unwrap()
            .unwrap()
            .path();
        let disk = std::fs::read(path.join("workspace.db")).unwrap();
        assert!(!disk.is_empty());
        assert!(!disk
            .windows(b"private-origin-sentinel".len())
            .any(|b| b == b"private-origin-sentinel"));
        let wrong_key = Connection::open(path.join("workspace.db")).unwrap();
        super::super::connection::apply_key(&wrong_key, &[0x55; 32]).unwrap();
        assert!(wrong_key
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row
                .get::<_, i64>(0))
            .is_err());
        drop(wrong_key);
        assert_eq!(
            workspace.advertised("id-2999").unwrap(),
            Some(items[2999].clone())
        );
        assert_eq!(
            workspace.comparison("id-2999").unwrap(),
            Some(items[2999].clone())
        );
        let page = workspace.remote_page(999, 2).unwrap();
        assert_eq!(page[0].1, items[999]);
        assert_eq!(page[1].1, items[1000]);
        workspace.set_wanted(&items[10]).unwrap();
        workspace.set_wanted(&items[11]).unwrap();
        let (selected, dropped) = workspace.finish_plan(1).unwrap();
        assert_eq!(selected, vec![items[11].clone()]);
        assert_eq!(dropped, 1);
        assert!(workspace.is_deferred("id-10").unwrap());
        drop(workspace);
        assert!(
            !std::fs::read_dir(workspace_root).unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("session-"))
        );
    }
}
