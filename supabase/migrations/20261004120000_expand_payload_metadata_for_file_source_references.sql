-- File source paths and Android content URIs travel with signed payload
-- metadata. Keep the database bound aligned with the shared client contract.

alter table public.clipboard_items
    drop constraint clipboard_items_payload_metadata_bounded,
    add constraint clipboard_items_payload_metadata_bounded
        check (
            payload_metadata is null
            or octet_length(payload_metadata) between 1 and 49152
        );
