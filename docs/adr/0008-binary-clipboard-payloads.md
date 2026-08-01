# ADR-0008 — Binary clipboard payloads are encrypted chunks

**Status:** accepted · 2026-08-01

Image and file clipboard values are raw bytes, never text stand-ins.  The
store keeps one self-describing encrypted chunk envelope per item; its id is
derived from SHA-256 of the raw bytes, and its header records the byte length,
full digest and chunk count.  Every chunk has its own fresh XChaCha nonce and
is authenticated against the item id and chunk index.

`chacha20poly1305` is already the maintained AEAD dependency.  Its stream
adapter cannot bind the persisted content id and per-chunk index in the AAD
format needed by this store, so this small envelope composes the existing
one-shot AEAD rather than adding a second crypto stack.  Binary rows never
receive FTS text, and transports carry decoded bytes under an explicit binary
encoding rather than reinterpreting them as UTF-8.
