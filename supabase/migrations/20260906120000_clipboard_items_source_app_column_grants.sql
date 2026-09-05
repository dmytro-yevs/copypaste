-- Source-app attribution was added after the authenticated column grants.
-- Re-assert the complete client-writable set without changing row policies.
revoke all on public.clipboard_items from authenticated;

grant select on public.clipboard_items to authenticated;

grant insert (item_id, ciphertext, nonce, content_type, payload_metadata,
              source_app_bundle_id, source_app_name, created_at, deleted,
              origin_device_id, signature)
    on public.clipboard_items to authenticated;

grant update (item_id, ciphertext, nonce, content_type, payload_metadata,
              source_app_bundle_id, source_app_name, created_at, deleted,
              origin_device_id, signature)
    on public.clipboard_items to authenticated;

grant delete on public.clipboard_items to authenticated;
