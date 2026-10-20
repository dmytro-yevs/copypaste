//! Durable cache identities, changed by triggers in the writer's transaction.
use super::{Store, StoreError};
use rusqlite::Connection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRevisions {
    pub generation: String,
    pub semantic: i64,
    pub facets: i64,
}

pub(super) fn read(conn: &Connection) -> Result<ResourceRevisions, StoreError> {
    Ok(conn.query_row(
        "SELECT generation, semantic, facets FROM resource_revisions WHERE singleton = 0",
        [],
        |row| {
            Ok(ResourceRevisions {
                generation: row.get(0)?,
                semantic: row.get(1)?,
                facets: row.get(2)?,
            })
        },
    )?)
}

impl Store {
    pub fn resource_revisions(&self) -> Result<ResourceRevisions, StoreError> {
        let conn = self.conn()?;
        read(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{item, store, T0};

    #[test]
    fn revisions_are_transactional_and_facets_ignore_pins() {
        let store = store();
        let item = store.insert(item("test", T0)).unwrap();
        let before = store.resource_revisions().unwrap();
        let mut conn = store.conn().unwrap();
        {
            let tx = conn.transaction().unwrap();
            tx.execute(
                "UPDATE clipboard_items SET app_name = 'new' WHERE id = ?1",
                [&item.id],
            )
            .unwrap();
            tx.rollback().unwrap();
        }
        assert_eq!(store.resource_revisions().unwrap(), before);
        conn.execute(
            "UPDATE clipboard_items SET pinned = 1 WHERE id = ?1",
            [&item.id],
        )
        .unwrap();
        let pinned = store.resource_revisions().unwrap();
        assert_eq!(pinned, before);
        assert_eq!(pinned.facets, before.facets);
        conn.execute(
            "UPDATE clipboard_items SET app_name = 'new' WHERE id = ?1",
            [&item.id],
        )
        .unwrap();
        assert!(store.resource_revisions().unwrap().facets > pinned.facets);
    }
}
