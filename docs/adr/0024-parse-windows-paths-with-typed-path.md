# ADR-0024: Parse Windows paths with `typed-path`

Status: accepted, 2026-08-13.

## Decision

`windows_attribution.rs` extracts the last segment of a Windows path with
`typed-path`'s `Utf8WindowsPath::file_name`. Two input-cleanup rules stay in our
code: surrounding double quotes are trimmed, and an entry ending in a separator
is refused because it names a directory rather than the image of a process.

`exclusions.ts` extracts it with `path-browserify-win32`'s `win32.basename` and
keeps the same two cleanup rules. No rule 1 exemption is claimed on either side.

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

## Why the package, in TypeScript

The earlier claim here — that the browser-safe path packages are POSIX-only —
held for `pathe` and not for the two packages that exist for exactly this. All
three were run on this host against `node:path.win32` over the exclusion table
plus `C:chrome.exe`, `C:\chrome.exe` and `a:b:c.exe`:

- [`path-browserify-win32@2.0.1`](https://www.npmjs.com/package/path-browserify-win32)
  — Node's path module forked for browsers with the `win32` half kept. MIT, no
  dependencies, and its bundle contains no `process`, `window` or `navigator`
  reference at all. Identical to `node:path.win32` on every case, and to
  `typed-path` on all three of the extra ones.
- [`path-unified@0.2.0`](https://www.npmjs.com/package/path-unified) — same
  answers, and the ESM subpath export buys nothing: 16.17 kB against 15.85 kB
  for the same entry in an isolated Vite build. Rejected because its module
  scope assigns `window.process` and sniffs `navigator.userAgent`, and both
  survive into the bundle. A path parser that writes a global into the shipped
  WebView costs more than the parsing is worth.
- `pathe@2.0.3` — POSIX only, as recorded: `basename('C:chrome.exe')` →
  `C:chrome.exe`. No drive prefix, so no Windows semantics.

The costs, stated rather than claimed as an exemption: the chunk that carries
the exclusion list grows from 18.85 kB to 28.99 kB (gzip 5.85 → 9.32 kB),
because the package is CommonJS and none of it tree-shakes; the entry chunk is
unchanged. Its `index.d.ts` declares the module in terms of itself, so the
import types as `any` and the single call site is cast to Node's own signature.

## Drift prevention

The two copies must agree on what a user can type. Both test modules assert the
same spellings — the `every_way_a_user_writes_one_process_names_it` table in
Rust and the matching `it.each` in `exclusions.test.ts` — and a divergence
shows as a failing test on one side while the other passes.

Two packages parsing the same grammar removes the one divergence this section
used to record: the hand-written split read a colon inside a segment as a
separator, so `a:b:c.exe` gave `c.exe` in the WebView and `b:c.exe` in the
daemon. Both now give `b:c.exe`.
