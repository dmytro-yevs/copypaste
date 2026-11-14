-- Synced module records ride the organization channel like library records;
-- this is their projection, kept whether or not the module is installed.
CREATE TABLE module_records (
    module_id TEXT NOT NULL,
    key TEXT NOT NULL,
    record_id TEXT NOT NULL,
    value_json TEXT NOT NULL,
    deleted INTEGER NOT NULL,
    stamp INTEGER NOT NULL,
    PRIMARY KEY (module_id, key)
);
