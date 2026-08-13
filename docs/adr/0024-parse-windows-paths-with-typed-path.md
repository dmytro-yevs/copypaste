# ADR-0024: Parse Windows paths with `typed-path`

Status: accepted, 2026-08-13.

## Decision

`windows_attribution.rs` extracts the last segment of a Windows path with
`typed-path`'s `Utf8WindowsPath::file_name`. Two input-cleanup rules stay in our
code: surrounding double quotes are trimmed, and an entry ending in a separator
is refused because it names a directory rather than the image of a process.

`exclusions.ts` keeps a hand-written equivalent. Exemption 1 under AGENTS.md
rule 1 applies to that copy only, and the reason is the bundle it ships in.

## Why the package, in Rust

An exclusion entry and a process image path are Windows paths whatever host is
running the code. `std::path::Path::file_name` uses the *host's* separators, so
off Windows it calls an entire `C:\…` string one file name — the tests would
pass on a Linux CI runner while the shipped behaviour did the opposite.

`typed-path` parses Windows paths anywhere, including the drive prefix that
makes `C:chrome.exe` one relative path rather than two segments. It adds one
package and no transitive dependencies. The earlier claim here — that the
package could not serve because it does not split on `:` and does not strip
quotes — was wrong twice over: a drive prefix is not a separator, and quote
trimming is input cleanup that happens before any parsing.

Evaluated and not used: `normpath` canonicalises through the OS, so a `C:\…`
path is not a path at all on a Linux host.

## Why not the package, in TypeScript

`exclusions.ts` ships inside the WebView bundle, which has no `node:path`, and
the maintained browser-safe path packages implement POSIX semantics only.
Checked directly on this host:

- `path.win32.basename('C:chrome.exe')` → `chrome.exe`, but `node:path` is not
  available in the bundle.
- `pathe@2.0.3` — `basename('C:\\Program Files\\chrome.exe')` → `chrome.exe`,
  but `basename('C:chrome.exe')` → `C:chrome.exe`: no drive prefix, so no
  Windows semantics.

Polyfilling `node:path` into the bundle to reach `path.win32` is the cost this
declines: a Node shim in the shipped WebView for one comparison key.

## Drift prevention

The two copies must agree on what a user can type. Both test modules assert the
same nine spellings — the `every_way_a_user_writes_one_process_names_it` table
in Rust and the matching `it.each` in `exclusions.test.ts` — and a divergence
shows as a failing test on one side while the other passes.

They agree on every spelling either side can receive. They differ on a colon
inside a path segment, which the TypeScript split treats as a separator and
`typed-path` does not: `a:b:c.exe`. No Windows image path and no path pasted
from Explorer can carry that shape, and an exclusion entry that did would name
no running process on either side.
