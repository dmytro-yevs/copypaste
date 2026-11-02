//! Retained encrypted payload accounting shared by metadata edits and restore.

macro_rules! content_bytes_sql {
    ($metadata:literal) => {
        concat!(
            "LENGTH(COALESCE(content_ciphertext, X'')) + ",
            "LENGTH(CAST(COALESCE(",
            $metadata,
            ", '') AS BLOB)) + ",
            "COALESCE((SELECT SUM(LENGTH(c.ciphertext) + LENGTH(c.nonce)) ",
            "FROM file_transfer_chunks c JOIN file_transfer_files f ON f.blob_id = c.blob_id ",
            "WHERE f.item_id = clipboard_items.id), 0) + ",
            "COALESCE((SELECT LENGTH(p.ciphertext) FROM shared_binary_payloads p ",
            "JOIN history_file_payloads f ON f.blob_id = p.id ",
            "WHERE f.item_id = clipboard_items.id), 0)"
        )
    };
}

pub(super) use content_bytes_sql;
