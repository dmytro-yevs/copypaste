# Compatibility evidence from v0.4.1

These files are **artifacts, not fixtures to be regenerated**. Every byte here
was produced by the released v0.4.1 implementation, which no longer exists in
this branch. They are the only implementation-independent way to answer the
question a user cares about most: *after I upgrade, is my clipboard history
still there?*

## Why this directory exists

The previous codebase had no such files. Every "can we still read old data" test
built the old format by calling the code under test — proving only that the code
agreed with itself. That check passes just as happily after a rewrite gets the
key derivation wrong.

## Contents

| File | What it pins |
|---|---|
| `manifest.json` | The recorded device secret, the derived key chain, and per-item AAD/nonce metadata |
| `*.v1.ct.bin` / `*.v2.ct.bin` | Item ciphertext under each key version |
| `*.plaintext.bin` | Expected plaintext for each |
| `chunked.wire.bin` | The chunk container framing |
| `v1-encrypted.db` | A SQLCipher database at `user_version = 1` |
| `v1-plaintext.db` | An unencrypted v1 database — the pre-SQLCipher install shape |

Cases cover empty input, multi-byte UTF-8, an embedded NUL, all 256 byte values,
and a multi-chunk payload.

## The two properties most easily lost

**The v1 item key is the HKDF output itself, not a second derivation of it.**
`manifest.json` records `seed_v1_hex` and `v1_item_key_hex` as identical values.
A previous build derived once more on top, and every legacy row silently failed
its authentication tag. Because AAD is authenticated but not encrypted, a wrong
key or wrong AAD is indistinguishable from corrupt data — it fails closed and
looks exactly like data loss.

**An unencrypted v1 database must be encrypted in place on open.** Nobody still
has one of these except users who never upgraded, which is precisely why the
path gets dropped in a rewrite.

## Do not regenerate

Nonces come from the OS RNG, so the output is not reproducible; the committed
bytes *are* the fixture. Both generators refuse to overwrite an existing
directory. That guard is deliberate — regenerating destroys the evidence and
leaves a test that passes against nothing.

## Wiring these in

`golden_v041_compat.rs` and `golden_v1_migration.rs` are the v1 tests, kept here
because there is not yet a crate to attach them to. Move them into the new core
crate's `tests/` as soon as it can encrypt and open a database, pointing them at
this directory. They should be the first tests that pass, not the last.

`gen_golden_fixtures.rs` and `gen_golden_db.rs` are retained to document exactly
how the bytes were produced. They will not compile against v2 and are not meant
to be run again.
