-- Transactional identities for bounded derived caches, shared by all writers.
CREATE TABLE resource_revisions (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 0),
    generation TEXT NOT NULL,
    semantic INTEGER NOT NULL DEFAULT 0,
    facets INTEGER NOT NULL DEFAULT 0
);
INSERT INTO resource_revisions(singleton, generation) VALUES (0, lower(hex(randomblob(16))));

CREATE TRIGGER resource_items_insert AFTER INSERT ON clipboard_items BEGIN
  UPDATE resource_revisions SET facets = facets + 1, semantic = semantic + 1 WHERE singleton = 0;
END;
CREATE TRIGGER resource_items_delete AFTER DELETE ON clipboard_items BEGIN
  UPDATE resource_revisions SET facets = facets + 1, semantic = semantic + 1 WHERE singleton = 0;
END;
CREATE TRIGGER resource_items_semantic_update AFTER UPDATE OF deleted, content_hash, content_type, fts_rowid ON clipboard_items
WHEN OLD.deleted IS NOT NEW.deleted OR OLD.content_hash IS NOT NEW.content_hash OR OLD.content_type IS NOT NEW.content_type OR OLD.fts_rowid IS NOT NEW.fts_rowid
BEGIN
  UPDATE resource_revisions SET semantic = semantic + 1 WHERE singleton = 0;
END;
CREATE TRIGGER resource_facets_update AFTER UPDATE OF deleted, origin_device_id, app_bundle_id, app_name, source_icon_id, created_at ON clipboard_items
WHEN OLD.deleted IS NOT NEW.deleted OR OLD.origin_device_id IS NOT NEW.origin_device_id OR OLD.app_bundle_id IS NOT NEW.app_bundle_id OR OLD.app_name IS NOT NEW.app_name OR OLD.source_icon_id IS NOT NEW.source_icon_id OR OLD.created_at IS NOT NEW.created_at
BEGIN
  UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0;
END;
CREATE TRIGGER resource_names_insert AFTER INSERT ON sync_device_name BEGIN UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_names_delete AFTER DELETE ON sync_device_name BEGIN UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_names_update AFTER UPDATE ON sync_device_name WHEN OLD.device_id IS NOT NEW.device_id OR OLD.name IS NOT NEW.name BEGIN UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_device_class_insert AFTER INSERT ON sync_device_state WHEN NEW.key LIKE 'device_class:%' BEGIN UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_device_class_delete AFTER DELETE ON sync_device_state WHEN OLD.key LIKE 'device_class:%' BEGIN UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_device_class_update AFTER UPDATE ON sync_device_state WHEN (OLD.key LIKE 'device_class:%' OR NEW.key LIKE 'device_class:%') AND (OLD.key IS NOT NEW.key OR OLD.value IS NOT NEW.value) BEGIN UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_documents_insert AFTER INSERT ON module_search_documents BEGIN UPDATE resource_revisions SET semantic = semantic + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_documents_delete AFTER DELETE ON module_search_documents BEGIN UPDATE resource_revisions SET semantic = semantic + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_documents_update AFTER UPDATE ON module_search_documents BEGIN UPDATE resource_revisions SET semantic = semantic + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_vectors_insert AFTER INSERT ON module_search_vectors BEGIN UPDATE resource_revisions SET semantic = semantic + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_vectors_delete AFTER DELETE ON module_search_vectors BEGIN UPDATE resource_revisions SET semantic = semantic + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_vectors_update AFTER UPDATE ON module_search_vectors BEGIN UPDATE resource_revisions SET semantic = semantic + 1 WHERE singleton = 0; END;
