alter table public.clipboard_items
    add column if not exists source_app_bundle_id text,
    add column if not exists source_app_name text,
    add constraint clipboard_items_source_app_bundle_id_bounded
        check (source_app_bundle_id is null or length(source_app_bundle_id) between 1 and 255),
    add constraint clipboard_items_source_app_name_bounded
        check (source_app_name is null or length(source_app_name) between 1 and 120),
    add constraint clipboard_items_tombstone_has_no_source_app
        check (not deleted or (source_app_bundle_id is null and source_app_name is null));
