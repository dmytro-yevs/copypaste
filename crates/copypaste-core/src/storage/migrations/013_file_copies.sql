-- Copy mode keeps an encrypted copy of each captured member beside its
-- source reference, in the shared chunk store. A copy never changes the
-- synced version. Chunk cleanup now also honours these copies.
CREATE TABLE file_copies (
    item_id TEXT NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    file_index INTEGER NOT NULL,
    blob_id TEXT NOT NULL,
    PRIMARY KEY (item_id, file_index)
);
CREATE INDEX idx_file_copies_blob ON file_copies(blob_id);

DROP TRIGGER file_transfer_release_stage;
DROP TRIGGER file_transfer_release_blob;
CREATE TRIGGER file_transfer_release_stage AFTER DELETE ON file_transfer_staging
BEGIN
    DELETE FROM file_transfer_chunks WHERE blob_id = OLD.blob_id
      AND NOT EXISTS (SELECT 1 FROM file_transfer_files WHERE blob_id = OLD.blob_id)
      AND NOT EXISTS (SELECT 1 FROM file_copies WHERE blob_id = OLD.blob_id)
      AND NOT EXISTS (SELECT 1 FROM file_transfer_staging WHERE blob_id = OLD.blob_id);
END;
CREATE TRIGGER file_transfer_release_blob AFTER DELETE ON file_transfer_files
BEGIN
    DELETE FROM file_transfer_chunks WHERE blob_id = OLD.blob_id
      AND NOT EXISTS (SELECT 1 FROM file_transfer_files WHERE blob_id = OLD.blob_id)
      AND NOT EXISTS (SELECT 1 FROM file_copies WHERE blob_id = OLD.blob_id)
      AND NOT EXISTS (SELECT 1 FROM file_transfer_staging WHERE blob_id = OLD.blob_id);
END;
CREATE TRIGGER file_copy_release_blob AFTER DELETE ON file_copies
BEGIN
    DELETE FROM file_transfer_chunks WHERE blob_id = OLD.blob_id
      AND NOT EXISTS (SELECT 1 FROM file_transfer_files WHERE blob_id = OLD.blob_id)
      AND NOT EXISTS (SELECT 1 FROM file_copies WHERE blob_id = OLD.blob_id)
      AND NOT EXISTS (SELECT 1 FROM file_transfer_staging WHERE blob_id = OLD.blob_id);
END;
CREATE TRIGGER file_copy_release_update AFTER UPDATE OF content_ciphertext ON clipboard_items
WHEN OLD.content_ciphertext IS NOT NEW.content_ciphertext
BEGIN
    DELETE FROM file_copies WHERE item_id = OLD.id;
END;
CREATE TRIGGER file_copy_release_delete AFTER DELETE ON clipboard_items
BEGIN
    DELETE FROM file_copies WHERE item_id = OLD.id;
END;
