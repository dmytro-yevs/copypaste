-- Removing a clip cascades into its search documents, and every content
-- change deletes them by item: both must seek instead of scanning the table.
CREATE INDEX idx_module_search_item ON module_search_documents(item_id);
