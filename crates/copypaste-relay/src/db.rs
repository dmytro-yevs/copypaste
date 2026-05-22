use rusqlite::{Connection, Result};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS devices (
        device_id TEXT PRIMARY KEY NOT NULL,
        public_key TEXT NOT NULL,
        token TEXT NOT NULL,
        registered_at INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS relay_items (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        item_id TEXT NOT NULL,
        recipient_device_id TEXT NOT NULL,
        sender_device_id TEXT NOT NULL,
        ciphertext TEXT NOT NULL,
        nonce TEXT NOT NULL,
        content_type TEXT NOT NULL,
        lamport_ts INTEGER NOT NULL,
        uploaded_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        FOREIGN KEY(recipient_device_id) REFERENCES devices(device_id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_relay_recipient ON relay_items(recipient_device_id, lamport_ts);

    -- Per-device tier and quota overrides.
    -- tier: 'free' | 'pro'
    -- quota_override_max_devices: NULL means use tier default
    -- quota_override_max_history:  NULL means use tier default
    CREATE TABLE IF NOT EXISTS device_quotas (
        device_id TEXT PRIMARY KEY NOT NULL,
        tier TEXT NOT NULL DEFAULT 'free',
        quota_override_max_devices INTEGER,
        quota_override_max_history  INTEGER,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        FOREIGN KEY(device_id) REFERENCES devices(device_id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_device_quotas_tier ON device_quotas(tier);
";

pub fn open(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_applies_schema() {
        let conn = open_in_memory().expect("should open in-memory DB");
        // Verify device_quotas table was created.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='device_quotas'",
                [],
                |row| row.get(0),
            )
            .expect("query should succeed");
        assert_eq!(count, 1, "device_quotas table must exist after schema init");
    }

    #[test]
    fn device_quotas_tier_defaults_to_free() {
        let conn = open_in_memory().expect("should open in-memory DB");
        // Insert a device then a quota row without specifying tier.
        conn.execute(
            "INSERT INTO devices (device_id, public_key, token, registered_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["dev-1", "pubkey", "tok", 0i64],
        ).expect("insert device");
        conn.execute(
            "INSERT INTO device_quotas (device_id, created_at, updated_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["dev-1", 0i64, 0i64],
        ).expect("insert quota");

        let tier: String = conn
            .query_row(
                "SELECT tier FROM device_quotas WHERE device_id = 'dev-1'",
                [],
                |row| row.get(0),
            )
            .expect("query should succeed");
        assert_eq!(tier, "free");
    }
}
