-- Optional module embeddings share the encrypted history database. They never
-- contain plaintext clip fragments and are local derived data, not sync payloads.
CREATE TABLE module_search_documents (
    scope TEXT NOT NULL,
    module_id TEXT NOT NULL,
    item_id TEXT NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    next_offset INTEGER NOT NULL DEFAULT 0,
    complete INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (scope, item_id)
);
CREATE INDEX idx_module_search_owner ON module_search_documents(module_id);
CREATE TABLE module_search_vectors (
    scope TEXT NOT NULL,
    item_id TEXT NOT NULL,
    chunk INTEGER NOT NULL,
    vector BLOB NOT NULL,
    PRIMARY KEY (scope, item_id, chunk),
    FOREIGN KEY (scope, item_id) REFERENCES module_search_documents(scope, item_id) ON DELETE CASCADE
);
