-- Organization versions use the existing encrypted item transports, but never
-- enter clipboard history, retention, search, notifications, or live counts.
CREATE TABLE organization_records (
    id TEXT PRIMARY KEY NOT NULL,
    content_ciphertext BLOB NOT NULL,
    nonce BLOB NOT NULL,
    content_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    origin_device_id TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_organization_versions ON organization_records(created_at, id);
CREATE TABLE history_tags (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE history_clip_tags (
    clip_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    assigned INTEGER NOT NULL,
    PRIMARY KEY (clip_id, tag_id)
);
CREATE INDEX idx_history_tag_clips ON history_clip_tags(tag_id, assigned, clip_id);
CREATE TABLE history_pin_state (
    clip_id TEXT PRIMARY KEY NOT NULL,
    pinned INTEGER NOT NULL,
    pin_order REAL,
    updated_at INTEGER NOT NULL
);
CREATE VIEW sync_records AS
SELECT id, content_ciphertext, nonce, content_type, content_hash, created_at,
       pinned, pin_order, pin_updated_at, deleted, origin_device_id,
       app_bundle_id, app_name, payload_metadata, source_icon_id
FROM clipboard_items
UNION ALL
SELECT id, content_ciphertext, nonce, 'application/vnd.copypaste.organization+json', content_hash,
       created_at, 0, NULL, 0, 0, origin_device_id, NULL, NULL, NULL, NULL
FROM organization_records;

CREATE TRIGGER organization_tag_insert AFTER INSERT ON history_tags BEGIN
  UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0;
END;
CREATE TRIGGER organization_tag_update AFTER UPDATE ON history_tags BEGIN
  UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0;
END;

CREATE TRIGGER organization_tag_delete AFTER DELETE ON history_tags BEGIN
  UPDATE resource_revisions SET facets = facets + 1 WHERE singleton = 0;
END;
