-- Raw bytes are shared by text, image, and file representations. Dedup only
-- repeated captures of the same representation, while content_hash remains the
-- stable transport and integrity hash.
DROP INDEX idx_items_dedup;
CREATE UNIQUE INDEX idx_items_dedup
    ON clipboard_items(content_hash, content_type, created_at / 60000, origin_device_id)
    WHERE deleted = 0 AND content_hash <> '' AND id NOT LIKE 'file-import-%';
