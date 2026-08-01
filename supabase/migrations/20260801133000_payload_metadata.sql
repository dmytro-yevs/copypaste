-- File payloads need a filename and MIME type for native paste-back. They are
-- metadata, never plaintext content: keep them bounded, sign them on clients,
-- and clear them with a tombstone.

alter table public.clipboard_items
    add column payload_metadata text;

alter table public.clipboard_items
    add constraint clipboard_items_payload_metadata_bounded
        check (payload_metadata is null or octet_length(payload_metadata) between 1 and 1024);

alter table public.clipboard_items
    drop constraint clipboard_items_tombstone_has_no_payload,
    add constraint clipboard_items_tombstone_has_no_payload
        check (
            not deleted
            or (
                coalesce(ciphertext, '') = ''
                and coalesce(nonce, '') = ''
                and payload_metadata is null
            )
        );

grant insert (payload_metadata) on public.clipboard_items to authenticated;
grant update (payload_metadata) on public.clipboard_items to authenticated;
