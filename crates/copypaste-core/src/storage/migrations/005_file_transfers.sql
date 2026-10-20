-- File bodies are local encrypted chunks, never ordinary sync payloads.
CREATE TABLE file_transfer_chunks (
    blob_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    ciphertext BLOB NOT NULL,
    nonce BLOB NOT NULL,
    PRIMARY KEY (blob_id, chunk_index)
);
CREATE TABLE file_transfer_files (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    blob_id TEXT NOT NULL
);
CREATE INDEX idx_file_transfer_blob ON file_transfer_files(blob_id);
CREATE TABLE file_transfer_staging (
    lease_id TEXT PRIMARY KEY NOT NULL,
    blob_id TEXT NOT NULL
);
CREATE TRIGGER file_transfer_release_stage AFTER DELETE ON file_transfer_staging
BEGIN
    DELETE FROM file_transfer_chunks WHERE blob_id = OLD.blob_id
      AND NOT EXISTS (SELECT 1 FROM file_transfer_files WHERE blob_id = OLD.blob_id)
      AND NOT EXISTS (SELECT 1 FROM file_transfer_staging WHERE blob_id = OLD.blob_id);
END;
CREATE TABLE downloaded_files (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    destination TEXT NOT NULL
);
CREATE TRIGGER file_transfer_release_update AFTER UPDATE OF content_ciphertext ON clipboard_items
WHEN OLD.content_ciphertext IS NOT NEW.content_ciphertext
BEGIN
    DELETE FROM file_transfer_files WHERE item_id = OLD.id;
    DELETE FROM downloaded_files WHERE item_id = OLD.id;
END;
CREATE TRIGGER file_transfer_release_delete AFTER DELETE ON clipboard_items
BEGIN
    DELETE FROM file_transfer_files WHERE item_id = OLD.id;
    DELETE FROM downloaded_files WHERE item_id = OLD.id;
END;
CREATE TRIGGER file_transfer_release_blob AFTER DELETE ON file_transfer_files
BEGIN
    DELETE FROM file_transfer_chunks WHERE blob_id = OLD.blob_id
      AND NOT EXISTS (SELECT 1 FROM file_transfer_files WHERE blob_id = OLD.blob_id)
      AND NOT EXISTS (SELECT 1 FROM file_transfer_staging WHERE blob_id = OLD.blob_id);
END;
