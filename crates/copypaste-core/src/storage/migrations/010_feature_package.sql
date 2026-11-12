-- Library records (snippets, rules, pipelines, preferences, AI grants, clip
-- notes) ride the organization channel; their projections and the local
-- per-clip state never enter the version stream themselves.
ALTER TABLE organization_records ADD COLUMN kind_class INTEGER NOT NULL DEFAULT 1;
ALTER TABLE organization_records ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_organization_kind_versions ON organization_records(kind_class, created_at, id);
DROP VIEW sync_records;
CREATE VIEW sync_records AS
SELECT id, content_ciphertext, nonce, content_type, content_hash, created_at,
       pinned, pin_order, pin_updated_at, deleted, origin_device_id,
       app_bundle_id, app_name, payload_metadata, source_icon_id
FROM clipboard_items
UNION ALL
SELECT id, content_ciphertext, nonce, 'application/vnd.copypaste.organization+json', content_hash,
       created_at, 0, NULL, 0, deleted, origin_device_id, NULL, NULL, NULL, NULL
FROM organization_records;

CREATE TABLE snippets (
    id TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL,
    collection_id TEXT,
    title TEXT NOT NULL,
    abbreviation TEXT,
    abbreviation_key TEXT,
    template INTEGER NOT NULL,
    pinned INTEGER NOT NULL,
    pin_order REAL,
    order_key TEXT NOT NULL,
    concealed INTEGER NOT NULL,
    deleted_at INTEGER,
    purged INTEGER NOT NULL DEFAULT 0,
    meta_stamp INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    created_device TEXT NOT NULL,
    fts_rowid INTEGER
);
CREATE INDEX idx_snippets_abbreviation ON snippets(abbreviation_key)
    WHERE deleted_at IS NULL AND abbreviation_key IS NOT NULL;
CREATE INDEX idx_snippets_collection ON snippets(collection_id, order_key);
CREATE INDEX idx_snippets_pinned ON snippets(pinned, pin_order);
CREATE TABLE snippet_bodies (
    id TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL,
    revision TEXT NOT NULL,
    ancestors_json TEXT NOT NULL,
    body_text TEXT NOT NULL,
    body_rich TEXT,
    body_stamp INTEGER NOT NULL
);
CREATE VIRTUAL TABLE snippet_fts USING fts5(id UNINDEXED, title, abbreviation, body_text);
CREATE TABLE snippet_collections (
    id TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    icon TEXT NOT NULL,
    accent TEXT NOT NULL,
    order_key TEXT NOT NULL,
    deleted_at INTEGER
);
CREATE TABLE content_rules (
    id TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL,
    origin TEXT NOT NULL,
    builtin_version INTEGER,
    customized_json TEXT NOT NULL,
    name TEXT NOT NULL,
    definition_json TEXT NOT NULL,
    device_scope_json TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    effective_from INTEGER NOT NULL,
    deleted_at INTEGER,
    stamp INTEGER NOT NULL
);
CREATE TABLE content_rule_placement (
    id TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    order_key TEXT NOT NULL
);
CREATE TABLE transform_pipelines (
    id TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    steps_json TEXT NOT NULL,
    deleted_at INTEGER,
    stamp INTEGER NOT NULL
);
CREATE TABLE library_preferences (
    namespace TEXT NOT NULL,
    key TEXT NOT NULL,
    record_id TEXT NOT NULL,
    value_json TEXT NOT NULL,
    stamp INTEGER NOT NULL,
    PRIMARY KEY (namespace, key)
);
CREATE TABLE library_set_members (
    set_name TEXT NOT NULL,
    member TEXT NOT NULL,
    record_id TEXT NOT NULL,
    present INTEGER NOT NULL,
    stamp INTEGER NOT NULL,
    PRIMARY KEY (set_name, member)
);
CREATE TABLE ai_grants (
    id TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL,
    owner_device TEXT NOT NULL,
    name TEXT NOT NULL,
    client_name TEXT NOT NULL,
    client_executable TEXT,
    scopes_json TEXT NOT NULL,
    lookback TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_day INTEGER,
    revoked INTEGER NOT NULL,
    revoked_at INTEGER,
    deleted INTEGER NOT NULL
);
CREATE TABLE clip_notes (
    clip_id TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL,
    hidden_from_ai INTEGER NOT NULL,
    hidden_changed_at INTEGER NOT NULL,
    base_stamp INTEGER NOT NULL,
    keep INTEGER NOT NULL,
    expires_at INTEGER,
    expiry_duration_ms INTEGER,
    provenance_json TEXT,
    orphan_since INTEGER
);
CREATE INDEX idx_clip_notes_record ON clip_notes(record_id);
CREATE INDEX idx_clip_notes_orphan ON clip_notes(orphan_since) WHERE orphan_since IS NOT NULL;

CREATE TABLE clip_local_state (
    clip_id TEXT PRIMARY KEY NOT NULL,
    expires_at INTEGER,
    expiry_duration_ms INTEGER,
    expiry_mono_ms INTEGER,
    rule_secret INTEGER NOT NULL DEFAULT 0,
    rule_hidden_from_ai INTEGER NOT NULL DEFAULT 0,
    applied_rules_json TEXT,
    eval_flags INTEGER NOT NULL DEFAULT 0,
    capture_channel TEXT,
    rules_revision INTEGER
);
CREATE INDEX idx_clip_local_state_expiry ON clip_local_state(expires_at) WHERE expires_at IS NOT NULL;
CREATE TABLE clip_rule_tags (
    clip_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    PRIMARY KEY (clip_id, tag_id)
);
CREATE INDEX idx_clip_rule_tags_tag ON clip_rule_tags(tag_id, clip_id);
CREATE TABLE rule_originals (
    clip_id TEXT PRIMARY KEY NOT NULL,
    content_ciphertext BLOB NOT NULL,
    nonce BLOB NOT NULL,
    content_type TEXT NOT NULL,
    purge_at INTEGER NOT NULL
);
CREATE INDEX idx_rule_originals_purge ON rule_originals(purge_at);
CREATE TABLE content_rule_conflicts (
    id TEXT PRIMARY KEY NOT NULL,
    rule_id TEXT NOT NULL,
    definition_json TEXT NOT NULL,
    from_device TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE snippet_usage (
    id TEXT PRIMARY KEY NOT NULL,
    last_used_at INTEGER NOT NULL,
    use_count INTEGER NOT NULL
);
CREATE TABLE snippet_origin (
    id TEXT PRIMARY KEY NOT NULL,
    clip_id TEXT NOT NULL
);
CREATE TABLE paste_queue_entries (
    entry_id TEXT PRIMARY KEY NOT NULL,
    position REAL NOT NULL,
    source_kind TEXT NOT NULL,
    clip_id TEXT,
    snapshot_ciphertext BLOB,
    snapshot_nonce BLOB,
    override_ciphertext BLOB,
    override_nonce BLOB,
    content_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    added_at INTEGER NOT NULL,
    added_via TEXT NOT NULL,
    delivered_at INTEGER,
    rendered_at INTEGER
);
CREATE INDEX idx_paste_queue_position ON paste_queue_entries(position);
CREATE INDEX idx_paste_queue_clip ON paste_queue_entries(clip_id) WHERE clip_id IS NOT NULL;
CREATE TABLE paste_queue_state (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    last_activity_at INTEGER,
    revision INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE ai_grant_verifiers (
    grant_id TEXT PRIMARY KEY NOT NULL,
    verifier BLOB NOT NULL
);
CREATE TABLE ai_access_log (
    seq INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    grant_id TEXT NOT NULL,
    tool TEXT NOT NULL,
    item_ids_json TEXT NOT NULL,
    result_count INTEGER NOT NULL,
    error TEXT
);
CREATE TABLE cloud_withheld (
    clip_id TEXT PRIMARY KEY NOT NULL,
    since INTEGER NOT NULL
);
CREATE TABLE expired_ids (
    clip_id TEXT PRIMARY KEY NOT NULL,
    expired_at INTEGER NOT NULL
);
CREATE INDEX idx_expired_ids_at ON expired_ids(expired_at);

-- Per-clip local state follows its clip. Inserts never clear it, so a restore
-- that copies clips before their state keeps every row. A tombstoned clip
-- tombstones its synced note at a stamp derived from the clip's own, so the
-- deleting device and every receiver write the same version.
CREATE TRIGGER clip_state_tombstone AFTER UPDATE OF deleted ON clipboard_items
WHEN OLD.deleted = 0 AND NEW.deleted = 1 BEGIN
  DELETE FROM clip_local_state WHERE clip_id = NEW.id;
  DELETE FROM clip_rule_tags WHERE clip_id = NEW.id;
  DELETE FROM rule_originals WHERE clip_id = NEW.id;
  UPDATE organization_records SET deleted = 1, content_ciphertext = X'', nonce = X'',
      created_at = MAX(created_at + 1, NEW.created_at)
  WHERE deleted = 0 AND id IN (SELECT record_id FROM clip_notes WHERE clip_id = NEW.id);
  DELETE FROM clip_notes WHERE clip_id = NEW.id;
END;
CREATE TRIGGER clip_state_delete AFTER DELETE ON clipboard_items BEGIN
  DELETE FROM clip_local_state WHERE clip_id = OLD.id;
  DELETE FROM clip_rule_tags WHERE clip_id = OLD.id;
  DELETE FROM rule_originals WHERE clip_id = OLD.id;
  DELETE FROM organization_records WHERE id IN (SELECT record_id FROM clip_notes WHERE clip_id = OLD.id);
  DELETE FROM clip_notes WHERE clip_id = OLD.id;
END;
CREATE TRIGGER clip_note_adopt AFTER INSERT ON clipboard_items WHEN NEW.deleted = 0 BEGIN
  UPDATE clip_notes SET orphan_since = NULL WHERE clip_id = NEW.id AND orphan_since IS NOT NULL;
END;

ALTER TABLE resource_revisions ADD COLUMN snippets INTEGER NOT NULL DEFAULT 0;
ALTER TABLE resource_revisions ADD COLUMN rules INTEGER NOT NULL DEFAULT 0;
ALTER TABLE resource_revisions ADD COLUMN queue INTEGER NOT NULL DEFAULT 0;
CREATE TRIGGER resource_snippets_insert AFTER INSERT ON snippets BEGIN UPDATE resource_revisions SET snippets = snippets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_snippets_update AFTER UPDATE ON snippets BEGIN UPDATE resource_revisions SET snippets = snippets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_snippets_delete AFTER DELETE ON snippets BEGIN UPDATE resource_revisions SET snippets = snippets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_snippet_bodies_insert AFTER INSERT ON snippet_bodies BEGIN UPDATE resource_revisions SET snippets = snippets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_snippet_bodies_update AFTER UPDATE ON snippet_bodies BEGIN UPDATE resource_revisions SET snippets = snippets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_snippet_bodies_delete AFTER DELETE ON snippet_bodies BEGIN UPDATE resource_revisions SET snippets = snippets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_snippet_collections_insert AFTER INSERT ON snippet_collections BEGIN UPDATE resource_revisions SET snippets = snippets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_snippet_collections_update AFTER UPDATE ON snippet_collections BEGIN UPDATE resource_revisions SET snippets = snippets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_snippet_collections_delete AFTER DELETE ON snippet_collections BEGIN UPDATE resource_revisions SET snippets = snippets + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_rules_insert AFTER INSERT ON content_rules BEGIN UPDATE resource_revisions SET rules = rules + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_rules_update AFTER UPDATE ON content_rules BEGIN UPDATE resource_revisions SET rules = rules + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_rules_delete AFTER DELETE ON content_rules BEGIN UPDATE resource_revisions SET rules = rules + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_rule_placement_insert AFTER INSERT ON content_rule_placement BEGIN UPDATE resource_revisions SET rules = rules + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_rule_placement_update AFTER UPDATE ON content_rule_placement BEGIN UPDATE resource_revisions SET rules = rules + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_rule_placement_delete AFTER DELETE ON content_rule_placement BEGIN UPDATE resource_revisions SET rules = rules + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_queue_insert AFTER INSERT ON paste_queue_entries BEGIN UPDATE resource_revisions SET queue = queue + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_queue_update AFTER UPDATE ON paste_queue_entries BEGIN UPDATE resource_revisions SET queue = queue + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_queue_delete AFTER DELETE ON paste_queue_entries BEGIN UPDATE resource_revisions SET queue = queue + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_queue_state_insert AFTER INSERT ON paste_queue_state BEGIN UPDATE resource_revisions SET queue = queue + 1 WHERE singleton = 0; END;
CREATE TRIGGER resource_queue_state_update AFTER UPDATE ON paste_queue_state BEGIN UPDATE resource_revisions SET queue = queue + 1 WHERE singleton = 0; END;
