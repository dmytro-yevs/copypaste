//! Conflict copies of snippet bodies synced from devices that still keep the
//! app's own snippet library.
//!
//! An apply that holds no key projects a conflict copy without its records;
//! the next keyed pass writes them so the copy reaches every other device.

use rusqlite::Connection;

use super::connection::write_tx;
use super::organization::{self, Record, RecordKey};
use super::{Store, StoreError};
use crate::snippets::conflict;
use crate::ItemKey;

fn pending(tx: &Connection, sql: &str) -> Result<Vec<String>, StoreError> {
    Ok(tx
        .prepare(sql)?
        .query_map([conflict::COPY_RANGE.0, conflict::COPY_RANGE.1], |r| {
            r.get(0)
        })?
        .collect::<rusqlite::Result<_>>()?)
}

fn publish_copies(tx: &Connection, key: &ItemKey) -> Result<usize, StoreError> {
    let snippets = pending(
        tx,
        "SELECT p.id FROM snippets p WHERE p.id>=?1 AND p.id<?2 \
         AND NOT EXISTS(SELECT 1 FROM organization_records r WHERE r.id=p.record_id)",
    )?;
    let bodies = pending(
        tx,
        "SELECT p.id FROM snippet_bodies p WHERE p.id>=?1 AND p.id<?2 \
         AND NOT EXISTS(SELECT 1 FROM organization_records r WHERE r.id=p.record_id)",
    )?;
    let published = snippets.len() + bodies.len();
    for id in snippets {
        if let Some(record @ Record::Snippet(_)) = organization::read(tx, RecordKey::Snippet(&id))?
        {
            organization::write(tx, key, record)?;
        }
    }
    for id in bodies {
        if let Some(body @ Record::SnippetBody(_)) =
            organization::read(tx, RecordKey::SnippetBody(&id))?
        {
            organization::write(tx, key, body)?;
        }
    }
    Ok(published)
}

impl Store {
    /// Writes the records of conflict copies an apply projected without a
    /// key. Cheap when there are none: one range seek.
    pub fn publish_snippet_conflicts(&self, key: &ItemKey) -> Result<usize, StoreError> {
        let pending: bool = self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM snippets p WHERE p.id>=?1 AND p.id<?2 \
             AND NOT EXISTS(SELECT 1 FROM organization_records r WHERE r.id=p.record_id))",
            [conflict::COPY_RANGE.0, conflict::COPY_RANGE.1],
            |r| r.get(0),
        )?;
        if !pending {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let published = publish_copies(&tx, key)?;
        tx.commit()?;
        Ok(published)
    }
}
