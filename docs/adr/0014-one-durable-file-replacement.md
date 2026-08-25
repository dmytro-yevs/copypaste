# ADR-0014: One durable file replacement, over `tempfile`

Status: accepted

## Decision

`copypaste-fs::write_atomically` is the only tmpfile-and-rename in the
workspace. Five call sites had a copy — `copypaste-p2p::peers::file`,
`copypaste-p2p::peers::cursor`, `copypaste-ui::shell::shortcut`,
`copypaste-ui::backend::embedded::state`, and the `sensitive-rules` generator —
along with two copies each of the `0600` helper and the directory `fsync`.

They had drifted, which is the cost rule 1 names. The peer sync cursors were
written with **no `fsync` at all**, so a rename could publish contents that had
never reached the disk; a cursor that survives a power loss ahead of what the
peer actually has costs a re-sync of everything between the two. The generator's
copy used a fixed `.tmp` sibling, so two runs collided and a failure left the
file behind. The peers file, which holds every pairing PSK, narrowed the mode
before writing; the settings file did not.

`tempfile` does the part that is genuinely hard — secure temporary creation,
removal on drop, `persist` as `rename(2)`. What is written here is the policy
on top of it: create the parent, narrow the mode *before* any bytes go in,
`fsync` the file, rename, `fsync` the directory.

## Why not a crate — AGENTS.md rule 1, exemption 1

Both maintained candidates were evaluated and neither contracts the property
this workspace needs, and each adds a second major version of a crate already
in the tree:

- [`atomicwrites` 0.4.4](https://crates.io/crates/atomicwrites) (Sep 2024) —
  `fsync`s the file and the directory, and hands the callback a `&mut File`, so
  the mode could be set before writing. But that ordering would be ours to
  maintain, not the crate's contract. It depends on `rustix ^0.38`; the
  workspace is on `rustix 1`.
- [`atomic-write-file` 0.3.0](https://crates.io/crates/atomic-write-file)
  (Sep 2025) — its documented default mode is `0o666 & ~umask`, and `mode()`
  applies to the committed file rather than to the temporary one, so key
  material would exist at a wider mode for the span of a write. It depends on
  `nix ^0.30` (new to the tree) and `rand ^0.9`; the workspace is on `rand 0.8`.

Cost of the decision, stated rather than waved past: ~90 lines of production
code we own and must keep correct, against ~40 lines of glue plus a duplicate
major version and a weaker guarantee. Neither candidate creates the parent
directory, which every call site needs.

## Consequences

The cursor store is now `fsync`ed and the settings file's mode is unchanged
(`Visibility::Inherited`) — a deliberate split, because only the pairing store
and the shortcut file are read-sensitive. `OWNER_ONLY_MODE` is one constant, so
a test cannot assert `0600` against a mode that has since moved. Adding a fifth
persisted file is a call, not a copy.
