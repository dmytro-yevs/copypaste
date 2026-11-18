-- ---------------------------------------------------------------------------
-- Bounds on what one row costs a device to read, and one order for its id.
--
-- Every device signed into an account downloads every row of it, so a single
-- row written straight through the REST API reaches them all. The client
-- applies these limits on the way in (`crates/copypaste-cloud/src/rest/body.rs`
-- and `sync/pull.rs`); refusing the row at write time is the half that also
-- covers a client older than those limits.
--
-- A row that already breaks one of the two checks fails this migration, which
-- names the constraint. Such a row is one no device can open, so removing it
-- loses nothing a device holds.
-- ---------------------------------------------------------------------------

alter table public.clipboard_items
    -- The one text column that had no bound. 32 characters today (a 24-byte
    -- XChaCha20 nonce); looser than that for the reason the signature bound is,
    -- and tight enough that a nonce cannot outgrow the row it belongs to.
    add constraint clipboard_items_nonce_bounded
        check (nonce is null or nonce ~ '^[A-Za-z0-9+/=]{0,128}$'),

    -- Standard base64, as `CloudItem` writes and decodes it. A character
    -- outside the alphabet makes the row undecodable, and one that JSON has to
    -- escape makes it up to six times its stored size on the wire, past
    -- `clipboard_items_payload_bounded`. Measured at 20 ms for a 16 MiB value.
    add constraint clipboard_items_payload_base64
        check (ciphertext is null or ciphertext ~ '^[A-Za-z0-9+/=]*$');

-- `item_id` is the tie-break of the pull order and of its keyset bound
-- (`order=created_at.asc,item_id.asc`, `item_id.gt.<id>`). Under the database
-- default both compare by locale rules, which a client comparing ids as bytes
-- does not reproduce and which move with the operating system's collation data
-- under the indexes built on them. "C" is byte order, the same on every server
-- and in every client. The two indexes on the column are rebuilt here.
alter table public.clipboard_items
    alter column item_id type text collate "C";
