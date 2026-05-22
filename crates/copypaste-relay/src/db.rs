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
