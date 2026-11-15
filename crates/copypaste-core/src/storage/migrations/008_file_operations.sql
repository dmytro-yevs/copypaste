-- Local operation history and deferred sources are never synchronized.
CREATE TABLE file_operations (
    id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL,
    phase TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    record TEXT NOT NULL
);
CREATE INDEX idx_file_operations_started ON file_operations(phase, started_at DESC, id DESC);
CREATE INDEX idx_file_operations_item ON file_operations(item_id);
CREATE TABLE file_references (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    reference TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    modified TEXT
);
CREATE INDEX idx_file_references_reference ON file_references(reference);
CREATE TRIGGER file_reference_release_update AFTER UPDATE OF content_ciphertext ON clipboard_items
WHEN OLD.content_ciphertext IS NOT NEW.content_ciphertext
BEGIN
    DELETE FROM file_references WHERE item_id = OLD.id;
END;

ALTER TABLE downloaded_files ADD COLUMN sha256 TEXT NOT NULL DEFAULT '';

-- Recover known previous downloads without inventing unavailable transfer times.
INSERT INTO file_operations(id,item_id,phase,started_at,record)
SELECT 'previous-' || d.item_id, d.item_id, 'finished', 0,
    json_object('id','previous-' || d.item_id,'item_id',d.item_id,
        'name',CASE WHEN COALESCE(json_extract(i.payload_metadata,'$.privacy.secret'),0)=1 THEN 'Hidden file' ELSE COALESCE(json_extract(i.payload_metadata,'$.file.filename'),'File') END,
        'kind','receive','phase','completed',
        'source_device',COALESCE((SELECT name FROM sync_device_name WHERE device_id=i.origin_device_id),'Unknown source'),
        'destination_device','This device',
        'written',COALESCE(json_extract(i.payload_metadata,'$.file.transfer.size_bytes'),0),
        'total',COALESCE(json_extract(i.payload_metadata,'$.file.transfer.size_bytes'),0),
        'started_at',0,'finished_at',NULL,'error',NULL,'local_path',d.destination)
FROM downloaded_files d JOIN clipboard_items i ON i.id=d.item_id WHERE i.deleted=0;
