-- A multi-file clip keeps one source reference and one saved copy per member.
-- Dependent triggers are recreated around the table rebuilds.
DROP TRIGGER file_reference_release_update;
DROP TRIGGER file_transfer_release_update;
DROP TRIGGER file_transfer_release_delete;

CREATE TABLE file_references_next (
    item_id TEXT NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    reference TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    modified TEXT,
    file_index INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (item_id, file_index)
);
INSERT INTO file_references_next (item_id, reference, size_bytes, modified)
SELECT item_id, reference, size_bytes, modified FROM file_references;
DROP TABLE file_references;
ALTER TABLE file_references_next RENAME TO file_references;
CREATE INDEX idx_file_references_reference ON file_references(reference);

CREATE TABLE downloaded_files_next (
    item_id TEXT NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    destination TEXT NOT NULL,
    sha256 TEXT NOT NULL DEFAULT '',
    file_index INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (item_id, file_index)
);
INSERT INTO downloaded_files_next (item_id, destination, sha256)
SELECT item_id, destination, sha256 FROM downloaded_files;
DROP TABLE downloaded_files;
ALTER TABLE downloaded_files_next RENAME TO downloaded_files;

CREATE TRIGGER file_reference_release_update AFTER UPDATE OF content_ciphertext ON clipboard_items
WHEN OLD.content_ciphertext IS NOT NEW.content_ciphertext
BEGIN
    DELETE FROM file_references WHERE item_id = OLD.id;
END;
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

-- Interrupted downloads resume from the last durable chunk. A row outlives
-- its item only until the next sweep, which also removes the partial file.
CREATE TABLE file_download_partials (
    item_id TEXT NOT NULL,
    file_index INTEGER NOT NULL,
    file_version TEXT NOT NULL,
    partial_path TEXT NOT NULL,
    destination TEXT NOT NULL,
    verified_bytes INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (item_id, file_index)
);
