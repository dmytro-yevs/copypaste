-- Explicit imports retain separate row identities while sharing binary bytes.
DROP INDEX idx_items_dedup;
CREATE UNIQUE INDEX idx_items_dedup
    ON clipboard_items(content_hash, created_at / 60000, origin_device_id)
    WHERE deleted = 0 AND content_hash <> '' AND id NOT LIKE 'file-import-%';
CREATE TABLE shared_binary_payloads (
    id TEXT PRIMARY KEY NOT NULL,
    ciphertext BLOB NOT NULL
);
CREATE TABLE history_file_payloads (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES clipboard_items(id),
    blob_id TEXT NOT NULL REFERENCES shared_binary_payloads(id)
);
CREATE INDEX idx_history_file_blob ON history_file_payloads(blob_id);
CREATE TRIGGER history_file_release_delete AFTER DELETE ON clipboard_items
BEGIN
    DELETE FROM history_file_payloads WHERE item_id = OLD.id;
END;
CREATE TRIGGER history_file_release_update AFTER UPDATE OF content_ciphertext ON clipboard_items
WHEN OLD.content_ciphertext IS NOT NEW.content_ciphertext
BEGIN
    DELETE FROM history_file_payloads WHERE item_id = OLD.id;
END;
CREATE TRIGGER history_file_blob_release AFTER DELETE ON history_file_payloads
BEGIN
    DELETE FROM shared_binary_payloads WHERE id = OLD.blob_id
      AND NOT EXISTS (SELECT 1 FROM history_file_payloads WHERE blob_id = OLD.blob_id);
END;
