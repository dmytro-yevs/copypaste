# UniFFI — Kotlin Binding Regeneration

This directory documents the Rust ↔ Android (Kotlin) FFI boundary built on
[mozilla/uniffi-rs](https://mozilla.github.io/uniffi-rs/).

The Rust source of truth lives in `crates/copypaste-android/`. The generated
Kotlin bindings live in `android/app/src/main/java/com/copypaste/generated/`
and are committed to the repository so contributors can build the Android app
without a Rust toolchain.

Because the generated Kotlin is checked in, it can drift from the UDL. The
`scripts/regen-uniffi.sh` script is the canonical way to bring them back in
sync.

## TL;DR

```bash
# See what the script would do, no side effects.
scripts/regen-uniffi.sh --dry-run

# Actually regenerate.
scripts/regen-uniffi.sh

# Show all options.
scripts/regen-uniffi.sh --help
```

## When to regenerate

Run `scripts/regen-uniffi.sh` whenever any of the following changes:

| Change                                                        | Why                                                      |
|---------------------------------------------------------------|----------------------------------------------------------|
| `crates/copypaste-android/uniffi/copypaste_android.udl`       | Function signatures / types changed.                     |
| `crates/copypaste-android/src/lib.rs` (UniFFI scaffold)       | Implementation of exported functions changed.            |
| `crates/copypaste-android/Cargo.toml` — `uniffi` version bump | Generated code format / runtime ABI may have changed.    |
| Cargo.lock — transitive `uniffi` or `uniffi_*` crate bump     | Scaffolding crate must match the bindgen binary version. |
| Rust target / NDK toolchain bump                              | cdylib layout / symbol mangling may shift.               |

You do **not** need to regenerate after:

- Editing pure Android (Kotlin / Java / XML) code.
- Editing other Rust crates that the Android app does not link against.
- Editing documentation or build scripts.

## What the script does

1. Verifies the UDL file exists at
   `crates/copypaste-android/uniffi/copypaste_android.udl`.
2. Runs `cargo build -p copypaste-android` to produce the cdylib and to
   guarantee the local `uniffi` crate version matches the bindgen binary.
3. Runs `cargo build -p copypaste-android --bin uniffi-bindgen` to compile
   the bindgen binary used in the next step.
4. Invokes `uniffi-bindgen generate ... --language kotlin --out-dir
   android/app/src/main/java/com/copypaste/generated/`.
5. Validates the output:
   - at least one `.kt` file exists,
   - the main binding file is non-trivial (>100 bytes),
   - if `ktlint` is installed, runs a syntax / style check (warning only).

Exit codes:

| Code | Meaning                                            |
|------|----------------------------------------------------|
| `0`  | Success (or dry-run completed).                    |
| `1`  | UDL file missing.                                  |
| `2`  | `cargo build` failed.                              |
| `3`  | `uniffi-bindgen generate` failed.                  |
| `4`  | Output validation failed (no files / too small).   |
| `64` | Unknown CLI flag.                                  |

## Flags

| Flag                | Purpose                                                       |
|---------------------|---------------------------------------------------------------|
| `-h`, `--help`      | Print usage and exit.                                         |
| `-n`, `--dry-run`   | Print every action without building or writing.               |
| `-v`, `--verbose`   | Trace every command (`set -x`).                               |

## Output layout

After a successful run you should see (at minimum):

```
android/app/src/main/java/com/copypaste/generated/
└── uniffi/
    └── copypaste_android/
        └── copypaste_android.kt
```

The exact path inside the `generated/` directory depends on the `namespace`
declared in the UDL file. The committed file currently lives at
`com/copypaste/generated/uniffi/copypaste_android/copypaste_android.kt`; if
that changes after regen, commit the move along with the regenerated content.

## Troubleshooting

### "UniFFI scaffolding mismatch" at runtime

The Rust crate was built against a different `uniffi` version than the one
that generated the Kotlin bindings.

**Fix:** run `scripts/regen-uniffi.sh`. The script rebuilds the bindgen
binary from the same Cargo workspace as the cdylib, which guarantees a
matching version.

If the mismatch persists, run `cargo clean -p copypaste-android` first to
force a full rebuild of the scaffolding macro output.

### "uniffi-bindgen binary not found at target/debug/uniffi-bindgen"

The `cargo build --bin uniffi-bindgen` step succeeded but produced the binary
in an unexpected location (typically because `CARGO_TARGET_DIR` is set).

**Fix:** either unset `CARGO_TARGET_DIR` for the run, or update `BINDGEN` at
the top of `scripts/regen-uniffi.sh` to point at the right path.

### "no .kt files emitted"

`uniffi-bindgen` exited 0 but wrote nothing. The most common cause is a UDL
file with an empty `namespace { ... }` block.

**Fix:** check the UDL declares at least one function / interface /
dictionary, then re-run.

### Generated file is unexpectedly small (<100 bytes)

This is what validation step 4b catches. It almost always means
`uniffi-bindgen` wrote only the header and skipped the body. Re-run with
`--verbose` to see the bindgen invocation and inspect its stderr.

### ktlint reports style issues

Style complaints in generated code are harmless — UniFFI's emitted Kotlin
does not target ktlint's default ruleset. Treat them as warnings unless the
report includes actual syntax errors. To suppress them entirely, add an
`.editorconfig` rule scoping the `generated/` directory out of ktlint.

### "cargo build failed"

Run `cargo build -p copypaste-android` by hand and read the Rust compiler
error. The regen script intentionally does not swallow build output beyond
piping its own stderr; the first compile error is the real issue.

## Why this script (and not the older `generate-android-bindings.sh`)

`scripts/generate-android-bindings.sh` predates the beta cycle and still
works, but it has no `--dry-run`, no `--help`, and no validation. The new
`scripts/regen-uniffi.sh` is meant to become the single supported entrypoint
going forward; the older script will be retired in a future release.

## CI note

The script is **not** wired into CI. Regenerating bindings is a manual,
intentional act because:

- The output is committed to the repo.
- Regen requires a working Rust + cargo install on the runner.
- Surprise diffs in checked-in Kotlin make code review noisier.

Run it locally, commit the regenerated files in the same commit as the UDL
or Rust change that motivated the regen, and call out the regen in the
commit message.
