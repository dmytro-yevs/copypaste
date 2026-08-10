//! The key/value table both sync transports keep their cursors and credentials
//! in, and where the daemon keeps its settings.
//!
//! It is inside the SQLCipher database rather than a file beside it because a
//! refresh token or a sync key in plaintext next to an encrypted database would
//! be the weakest link in the design.

use rusqlite::{params, OptionalExtension};

use super::connection::write_tx;
use super::model::StoreError;
use super::store::Store;

impl Store {
    /// Read one value, or `None` when it has never been set.
    pub fn state(&self, key: &str) -> Result<Option<String>, StoreError> {
        let conn = self.conn()?;
        let value = conn
            .query_row(
                "SELECT value FROM sync_device_state WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(value.filter(|v| !v.is_empty()))
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<(), StoreError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO sync_device_state (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Write several values in one transaction.
    ///
    /// A set rather than one key for the same reason [`Store::clear_state`] is:
    /// the cloud download cursor is a *keyset* over `(created_at, item_id)`,
    /// and a crash between two separate writes would leave a millisecond from
    /// one round beside an item id from another. The pull query trusts the
    /// pair, so a mismatched half does not cost re-pagination — it silently
    /// skips the rows the stale id sorts above.
    ///
    /// An empty value deletes the key, because [`Store::state`] already reads
    /// an empty string back as absent and storing one would be a third state.
    pub fn set_state_all(&self, entries: &[(&str, &str)]) -> Result<(), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        for (key, value) in entries {
            if value.is_empty() {
                tx.execute("DELETE FROM sync_device_state WHERE key = ?1", [key])?;
            } else {
                tx.execute(
                    "INSERT INTO sync_device_state (key, value) VALUES (?1, ?2) \
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![key, value],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Forget a set of keys in one transaction.
    ///
    /// A set rather than one key because signing out has to clear the account,
    /// both tokens and the key together: a partial clear leaves a refresh token
    /// on disk for an account the daemon no longer thinks it holds.
    pub fn clear_state(&self, keys: &[&str]) -> Result<(), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        for key in keys {
            tx.execute("DELETE FROM sync_device_state WHERE key = ?1", [key])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Read a cursor stored as a decimal string, defaulting to zero.
    ///
    /// A value that will not parse is treated as absent rather than as an
    /// error: the cost of re-reading from the start is bandwidth, and the cost
    /// of refusing to sync is that nothing syncs at all.
    pub fn state_ms(&self, key: &str) -> Result<i64, StoreError> {
        Ok(self
            .state(key)?
            .and_then(|raw| raw.parse::<i64>().ok())
            .unwrap_or(0)
            .max(0))
    }

    pub fn set_state_ms(&self, key: &str, ms: i64) -> Result<(), StoreError> {
        self.set_state(key, &ms.max(0).to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::test_support::store;

    #[test]
    fn a_value_round_trips_and_overwrites() {
        let s = store();
        assert_eq!(s.state("example_state").unwrap(), None);

        s.set_state("example_state", "first").unwrap();
        assert_eq!(s.state("example_state").unwrap().as_deref(), Some("first"));

        s.set_state("example_state", "second").unwrap();
        assert_eq!(s.state("example_state").unwrap().as_deref(), Some("second"));
    }

    #[test]
    fn clearing_removes_every_named_key_and_spares_the_identity() {
        let s = store();
        let me = s.device_identity("laptop").unwrap();
        s.set_state("a", "1").unwrap();
        s.set_state("b", "2").unwrap();
        s.clear_state(&["a", "b", "never-set"]).unwrap();
        assert_eq!(s.state("a").unwrap(), None);
        assert_eq!(s.state("b").unwrap(), None);
        // The device identity lives in the same table and must survive.
        assert_eq!(s.device_identity("ignored").unwrap(), me);
    }

    #[test]
    fn several_values_are_written_together_and_an_empty_one_clears() {
        let s = store();
        s.set_state_all(&[("cursor", "42"), ("cursor_item", "item-a")])
            .unwrap();
        assert_eq!(s.state("cursor").unwrap().as_deref(), Some("42"));
        assert_eq!(s.state("cursor_item").unwrap().as_deref(), Some("item-a"));

        // The half that no longer applies is removed rather than left behind:
        // a stale item id beside a fresh millisecond skips rows.
        s.set_state_all(&[("cursor", "43"), ("cursor_item", "")])
            .unwrap();
        assert_eq!(s.state("cursor").unwrap().as_deref(), Some("43"));
        assert_eq!(s.state("cursor_item").unwrap(), None);
    }

    #[test]
    fn a_cursor_defaults_to_zero_and_never_reads_negative() {
        let s = store();
        assert_eq!(s.state_ms("cursor").unwrap(), 0);

        s.set_state_ms("cursor", 1_700_000_000_000).unwrap();
        assert_eq!(s.state_ms("cursor").unwrap(), 1_700_000_000_000);

        // CopyPaste-psx7 in miniature: a negative cursor must not survive a
        // round trip, in either direction.
        s.set_state_ms("cursor", -5).unwrap();
        assert_eq!(s.state_ms("cursor").unwrap(), 0);
        s.set_state("cursor", "not a number").unwrap();
        assert_eq!(s.state_ms("cursor").unwrap(), 0);
    }
}
