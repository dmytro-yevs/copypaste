-- Hosted databases that already applied 20260730120100 may still hold
-- table-level INSERT/UPDATE for authenticated from the image defaults.
-- Re-assert column-limited privileges without changing policies.
revoke all on public.clipboard_items from authenticated;

grant select on public.clipboard_items to authenticated;

grant insert (item_id, ciphertext, nonce, content_type, payload_metadata,
              created_at, deleted, origin_device_id, signature)
    on public.clipboard_items to authenticated;

grant update (item_id, ciphertext, nonce, content_type, payload_metadata,
              created_at, deleted, origin_device_id, signature)
    on public.clipboard_items to authenticated;

grant delete on public.clipboard_items to authenticated;
