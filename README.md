# CopyPaste

Encrypted clipboard manager for macOS and Android.

---

## This branch is a rewrite in progress

`main` was reset to an empty history on the date of the first commit below. It
currently contains **no application code** — only the specification, the
compatibility evidence, and the working rules for building v2.

The previous implementation is preserved in full at
**`archive/v0.4.1-pre-rewrite`** (2,153 commits). Nothing was lost; it is one
`git switch` away.

## Why

An audit of all eight subsystems of v0.4.1 found ~150k lines of Rust and ~22k of
TypeScript for a clipboard manager, with the same problems solved repeatedly:
six independent retry/backoff implementations, three rate limiters, three
models of one wire contract, two hand-written ASN.1 parsers, two regex secret
engines. Alongside that, several subsystems were dead — a complete
HELLO/HAVE/WANT sync engine (~5k lines including tests) that the daemon never
instantiated, and a telemetry crate whose own documentation stated it was never
wired to a caller.

The cause was a "prefer hand-rolling" norm that had outlived the document
defining it. v2 inverts that default. See [CLAUDE.md](CLAUDE.md).

## What is here

| Path | What it is |
|---|---|
| `CLAUDE.md` | The working rules. Dependencies are the default; hand-rolling needs an ADR. |
| `docs/rewrite/port-manifest/` | ~9,100 lines of specification harvested from v1 and its tests: ~500 acceptance tests, 200+ recovered bug IDs. **These are the requirements.** |
| `docs/rewrite/target-architecture.md` | The library-first stack, per subsystem, and the six things that stay custom on purpose. |
| `docs/rewrite/design-reference.html` | Visual reference for the v1 UI; its tokens are captured in manifest 06. |

## No upgrade path

v2 does not read data written by v0.4.x. Existing installs lose their clipboard
history and their paired devices; devices must be paired again.

This is deliberate. It removes the migration ladder, `key_version` dispatch, the
rotation and repair sweeps, and every wart kept only for bug-compatibility — a
large share of the complexity this rewrite exists to shed. v2 uses a distinct
database filename, so an old file is never opened or modified and remains on
disk if you downgrade.

## What the manifests cover

| Manifest | Subject |
|---|---|
| 01 | Clipboard capture — NSPasteboard quirks, privacy markers, 39 invariants |
| 02 | Crypto — verbatim HKDF info strings and AAD layouts, key-version semantics |
| 03 | Storage — schema, the v1→v15 migration ladder, SQLCipher parameters |
| 04 | IPC — the full method catalogue and error codes |
| 05 | Sync — merge ordering, and the relay→Supabase parity checklist |
| 06 | UI — behaviour contract, accessibility, design tokens |
| 07 | Secret detection — the full ruleset with confidence thresholds |

## Building v2

Nothing to build yet. The intended order is:

1. Core crate: crypto and storage — a single schema and a single key derivation,
   with no legacy paths.
2. Daemon: capture and IPC, against manifests 01 and 04. Manifest 01 is where
   the hard-won macOS behaviour lives and is binding in full.
3. Sync against manifest 05, on Supabase rather than a bespoke relay.
4. UI against manifest 06, on the library stack in the architecture doc.

[`docs/rewrite/port-manifest/README.md`](docs/rewrite/port-manifest/README.md)
records which manifest sections are binding requirements and which became
reference material when backward compatibility was dropped.

## Licence

MIT or Apache-2.0, at your option.
