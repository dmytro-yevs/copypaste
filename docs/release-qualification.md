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

## v2.0.0-alpha.33 evidence exception

Only `2.0.0-alpha.33` may qualify with its recorded 58 pending native-evidence
states. The release gate pins their sorted IDs and SHA-256 digest in
`config/release-evidence-exceptions.json`; any added, removed, renamed, or
resolved state fails the exception. This is a one-alpha risk acceptance, not a
verification claim or precedent. Pending states remain absent from receipt
expectations, and every other version still requires complete evidence.
