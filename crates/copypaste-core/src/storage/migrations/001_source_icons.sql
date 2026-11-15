CREATE TABLE source_app_icons (
    id TEXT PRIMARY KEY NOT NULL,
    png_base64 TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
ALTER TABLE clipboard_items ADD COLUMN source_icon_id TEXT REFERENCES source_app_icons(id);
CREATE INDEX idx_items_source_icon ON clipboard_items(source_icon_id);
CREATE INDEX idx_items_source_app ON clipboard_items(app_bundle_id) WHERE deleted = 0;
CREATE TRIGGER source_icon_link_insert AFTER INSERT ON source_app_icons
WHEN NEW.id LIKE 'app:%'
BEGIN
    UPDATE clipboard_items SET source_icon_id = NEW.id
      WHERE source_icon_id IS NULL AND app_bundle_id = substr(NEW.id, 5) AND deleted = 0;
END;
CREATE TRIGGER source_icon_release_delete AFTER DELETE ON clipboard_items
WHEN OLD.source_icon_id IS NOT NULL
BEGIN
    DELETE FROM source_app_icons WHERE id = OLD.source_icon_id
      AND NOT EXISTS (SELECT 1 FROM clipboard_items WHERE source_icon_id = OLD.source_icon_id);
END;
CREATE TRIGGER source_icon_release_update AFTER UPDATE OF source_icon_id ON clipboard_items
WHEN OLD.source_icon_id IS NOT NULL AND OLD.source_icon_id IS NOT NEW.source_icon_id
BEGIN
    DELETE FROM source_app_icons WHERE id = OLD.source_icon_id
      AND NOT EXISTS (SELECT 1 FROM clipboard_items WHERE source_icon_id = OLD.source_icon_id);
END;
