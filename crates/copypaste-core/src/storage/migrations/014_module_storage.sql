-- What a loaded module declared for its records on this device. Local only:
-- a module registers it again whenever it loads.
CREATE TABLE module_storage (
    module_id TEXT PRIMARY KEY NOT NULL,
    search_fields TEXT NOT NULL,
    keep_conflicts INTEGER NOT NULL
);
-- Full-text index over the declared fields of live module records.
CREATE VIRTUAL TABLE module_record_fts USING fts5(body);
CREATE TABLE module_record_search (
    module_id TEXT NOT NULL,
    key TEXT NOT NULL,
    fts_rowid INTEGER NOT NULL,
    PRIMARY KEY (module_id, key)
);
-- Local values sync replaced, kept until the module handles them.
CREATE TABLE module_record_conflicts (
    id INTEGER PRIMARY KEY NOT NULL,
    module_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value_json TEXT NOT NULL
);
CREATE INDEX idx_module_record_conflicts ON module_record_conflicts(module_id, id);
