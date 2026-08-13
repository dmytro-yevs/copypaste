# ADR-0023: Hand-written exclusion-key normalisation

Status: accepted, 2026-08-13. Exemption 1 under CLAUDE.md rule 1.

## Decision

The four-line `file_name` / `exclusion_key` helpers in
`windows_attribution.rs` and `exclusions.ts` are kept hand-written rather than
replaced by a maintained package.

## Why no package fits

The function extracts the last segment of a Windows path using `\`, `/` and `:`
as separators, on any host. Evaluated:

- **`normpath`** (Rust, 12 M downloads) — canonicalises real paths; it calls
  into the OS, so a `C:\…` path on a Linux CI host is not a Windows path.
- **`path.basename` / `path.parse`** (Node built-in) — uses the host's
  separators; a `C:\…` literal in a Linux test is one segment, not two.
- **`std::path::Path::file_name`** (Rust stdlib) — same host-separator issue;
  the doc comment in `windows_attribution.rs:40` exists because of it.

No maintained package splits by all three Windows separators on a non-Windows
host while also handling quoted paths, which is what the predicate needs.

## Consequences

The Rust and TypeScript copies must agree — `exclusions.ts` names
`windows_attribution.rs` as the reference, and both share the same table test.
A change to either must update both, which the comment and the shared test
enforce.
