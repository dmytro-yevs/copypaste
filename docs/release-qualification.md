# Release qualification

`Release` accepts an explicit `qualify` dispatch input. It is false by default.
With `qualify=true` and `publish=false`, the workflow builds the usual release
artifacts and runs the signed Windows package and Android emulator checks, and the
three-platform native-parity gate. The resulting artifacts and
receipts remain run artifacts: no GitHub Release, tag, or Homebrew tap update is
created.

A tag push and `publish=true` both imply qualification. `publish` remains the
only job with `contents: write`, and it is the only path that can create a
release or publish to the tap. A dispatch with both inputs false remains the
build-only mode.

Qualification verifies the current run's artifact bytes against its native
receipts. It does not promote those artifacts into a later publication; durable
artifact digest binding and promotion are separate release work.

## v2.0.0-alpha.35 and v2.0.0-alpha.36 evidence exceptions

Only `2.0.0-alpha.35` and `2.0.0-alpha.36` may qualify with their individually
approved records for the same 58 pending native-evidence states. The release
gate pins each record's version, authorization date, sorted IDs, and SHA-256
digest in
`config/release-evidence-exceptions.json`; any added, removed, renamed, or
resolved state fails the exception. These are one-alpha risk acceptances, not
verification claims or precedents. Pending states remain absent from receipt
expectations, and every other version still requires complete evidence.
