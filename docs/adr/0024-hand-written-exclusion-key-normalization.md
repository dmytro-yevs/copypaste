# ADR-0024: Hand-written exclusion-key normalisation

Status: accepted, 2026-08-13. Exemption 1 under AGENTS.md rule 1.

## Decision

The four-line `file_name` / `exclusion_key` helpers in
`windows_attribution.rs` and `exclusions.ts` are kept hand-written rather than
replaced by a maintained package.

## Evaluated packages

- **`typed-path`** (Rust, docs.rs/typed-path) — provides `WindowsPath` that
  splits by `\` and `/` on any host, but does not strip surrounding quotes
  (`"C:\…\chrome.exe"` yields `chrome.exe"` as the file name) and does not
  split on `:` (a bare `C:chrome.exe` is a valid relative Windows path our
  input can carry).
- **`normpath`** (Rust, 12 M downloads) — canonicalises real paths via the OS;
  a `C:\…` path on a Linux CI host is not a Windows path to it.
- **`path.win32.basename`** (Node built-in) — handles `\` and `/` on any host,
  but does not strip surrounding quotes and does not split on `:`. A direct
  Node check confirmed that `path.win32.basename('"C:\\…\\chrome.exe"')`
  returns `chrome.exe"`, not `chrome.exe`.
- **`std::path::Path::file_name`** (Rust stdlib) — uses the host's separators;
  a `C:\…` literal on a Linux CI host is one segment, not two.

## Why no package fits

The function extracts the last segment of a Windows path using `\`, `/` and `:`
as separators, on any host, after stripping surrounding double-quotes. Users
paste paths from Explorer's address bar (often quoted by the shell), and a
failure here is a silently non-matching exclusion entry. No evaluated package
handles all three separators AND quote-stripping.

## Drift prevention

The Rust and TypeScript copies must produce the same canonical form: a spelling
this file accepts and the daemon does not is an exclusion the user believes is
active while it never fires. `exclusions.ts` names `windows_attribution.rs` as
the reference. Both test modules assert the same nine input spellings (the
`every_way_a_user_writes_one_process_names_it` table in Rust and the matching
`it.each` in `exclusions.test.ts`). A mismatch would show as a failing test on
one side while the other passes.
