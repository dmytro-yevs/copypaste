-- Disposable authenticated previews are populated only when pages are read.
CREATE TABLE history_preview_cache (
    item_id TEXT PRIMARY KEY NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    nonce BLOB NOT NULL,
    ciphertext BLOB NOT NULL
);
-- A tombstone must remove the derived entry in the same transaction.
CREATE TRIGGER history_preview_release_tombstone AFTER UPDATE OF deleted ON clipboard_items
WHEN NEW.deleted <> 0
BEGIN
    DELETE FROM history_preview_cache WHERE item_id = NEW.id;
END;
