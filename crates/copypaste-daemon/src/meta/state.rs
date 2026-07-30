//! The key/value table both transports keep their cursors and credentials in.
//!
//! It is inside the SQLCipher database rather than a file beside it because a
//! refresh token or a sync key in plaintext next to an encrypted database would
//! be the weakest link in the design.

use rusqlite::{params, OptionalExtension};

use super::{Meta, MetaError};

impl Meta {
    /// Read one value, or `None` when it has never been set.
    pub fn state(&self, key: &str) -> Result<Option<String>, MetaError> {
        let conn = self.lock()?;
        let value = conn
            .query_row(
                "SELECT value FROM sync_device_state WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(value.filter(|v| !v.is_empty()))
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<(), MetaError> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO sync_device_state (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Forget a set of keys in one transaction.
    ///
    /// A set rather than one key because signing out has to clear the account,
    /// both tokens and the key together: a partial clear leaves a refresh token
    /// on disk for an account the daemon no longer thinks it holds.
    pub fn clear_state(&self, keys: &[&str]) -> Result<(), MetaError> {
        let mut conn = self.lock()?;
        // IMMEDIATE, for the reason `copypaste_core::storage::connection::write_tx`
        // records: a deferred transaction that upgrades to a write mid-way gets
        // SQLITE_BUSY_SNAPSHOT the instant the other connection to this file is
        // writing, and `busy_timeout` does not retry that. A cloud round writing
        // through this same handle while a capture lands is the ordinary case,
        // not a rare one — it surfaced as "the item could not be stored" in
        // `demo-cloud.sh` two runs in five once the idle poll ceiling dropped
        // to 10 s.
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
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
    pub fn state_ms(&self, key: &str) -> Result<i64, MetaError> {
        Ok(self
            .state(key)?
            .and_then(|raw| raw.parse::<i64>().ok())
            .unwrap_or(0)
            .max(0))
    }

    pub fn set_state_ms(&self, key: &str, ms: i64) -> Result<(), MetaError> {
        self.set_state(key, &ms.max(0).to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::meta::testutil::fixture;

    #[test]
    fn a_value_round_trips_and_overwrites() {
        let f = fixture();
        assert_eq!(f.meta.state("cloud_email").unwrap(), None);

        f.meta.set_state("cloud_email", "a@example.com").unwrap();
        assert_eq!(
            f.meta.state("cloud_email").unwrap().as_deref(),
            Some("a@example.com")
        );

        f.meta.set_state("cloud_email", "b@example.com").unwrap();
        assert_eq!(
            f.meta.state("cloud_email").unwrap().as_deref(),
            Some("b@example.com")
        );
    }

    #[test]
    fn clearing_removes_every_named_key() {
        let f = fixture();
        f.meta.set_state("a", "1").unwrap();
        f.meta.set_state("b", "2").unwrap();
        f.meta.clear_state(&["a", "b", "never-set"]).unwrap();
        assert_eq!(f.meta.state("a").unwrap(), None);
        assert_eq!(f.meta.state("b").unwrap(), None);
        // The device identity lives in the same table and must survive.
        assert!(!f.meta.device_id().is_empty());
    }

    #[test]
    fn a_cursor_defaults_to_zero_and_never_reads_negative() {
        let f = fixture();
        assert_eq!(f.meta.state_ms("cursor").unwrap(), 0);

        f.meta.set_state_ms("cursor", 1_700_000_000_000).unwrap();
        assert_eq!(f.meta.state_ms("cursor").unwrap(), 1_700_000_000_000);

        // CopyPaste-psx7 in miniature: a negative cursor must not survive a
        // round trip, in either direction.
        f.meta.set_state_ms("cursor", -5).unwrap();
        assert_eq!(f.meta.state_ms("cursor").unwrap(), 0);
        f.meta.set_state("cursor", "not a number").unwrap();
        assert_eq!(f.meta.state_ms("cursor").unwrap(), 0);
    }
}
