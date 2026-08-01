# ADR-0008 — Binary clipboard payloads use RustCrypto STREAM

**Status:** accepted · 2026-08-01

Image and file clipboard values are raw bytes, never text stand-ins. The store
keeps one content-addressed encrypted envelope per item. Format version 3 has a
64-byte header: `CPB2`, version, total plaintext length, SHA-256 digest and the
19-byte nonce prefix required by `StreamBE32<XChaCha20Poly1305>`.

`EncryptorBE32` and `DecryptorBE32` authenticate position and the final-block
flag. The item id and complete header are AAD for every segment, which also
binds total length and digest. Segment boundaries are implicit from the fixed
chunk size and authenticated total length, so the body is only ciphertext and
Poly1305 tags. v2 does not retain a decoder for the obsolete manual framing.

`ItemKey` exposes no reusable raw bytes: a crypto-internal constructor builds
the STREAM objects. Binary rows never enter FTS, and transports carry decoded
bytes under an explicit binary encoding rather than treating them as UTF-8.
