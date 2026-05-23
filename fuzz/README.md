# copypaste-fuzz — coverage-guided fuzz harness

Five libFuzzer targets that hammer the wire/decoder surfaces a malicious or
buggy peer can reach:

| Target                | Decoder under test                                              | Reachable from                |
|-----------------------|-----------------------------------------------------------------|-------------------------------|
| `ipc_protocol_parse`  | `copypaste_ipc::{Request, Response}` (JSON)                     | local UDS (UI / CLI → daemon) |
| `image_decode`        | `copypaste_core::image::thumbnail` (PNG / TIFF)                 | clipboard contents            |
| `sync_event_decode`   | `copypaste_sync::protocol::Message::decode` (JSON)              | remote P2P peer               |
| `aead_decrypt`        | `copypaste_core::decrypt_item_with_aad` (XChaCha20-Poly1305)    | tampered DB row, P2P peer     |
| `snapshot_parse`      | `copypaste_ipc::types::ImportItem` snapshot dumps (JSON)        | CLI `import` file, UDS        |

The invariant for every target is the same: **no panics, no aborts**. Errors
returned via `Result::Err` (or `EncryptError::AuthFailed` for AEAD) are the
expected outcome for malformed / adversarial input.

## Why not a workspace member?

`fuzz/Cargo.toml` declares `[workspace]` (empty) on purpose. `cargo-fuzz`
injects nightly-only flags (`-Z build-std`, libfuzzer link config) that
would otherwise leak into the stable top-level workspace build.

## Prerequisites

* Rust **nightly** toolchain (`rustup toolchain install nightly`)
* `cargo-fuzz` (`cargo install cargo-fuzz`)
* On macOS: the bundled `libFuzzer` shipped with the nightly Apple-clang
  toolchain. No extra install needed.

## Running

From the **repo root** (not from `fuzz/`):

```bash
# Build all three targets (sanity check, no fuzzing yet)
cargo +nightly fuzz build --fuzz-dir fuzz

# Fuzz a single target for one minute
cargo +nightly fuzz run ipc_protocol_parse  --fuzz-dir fuzz -- -max_total_time=60
cargo +nightly fuzz run image_decode        --fuzz-dir fuzz -- -max_total_time=60
cargo +nightly fuzz run sync_event_decode   --fuzz-dir fuzz -- -max_total_time=60
cargo +nightly fuzz run aead_decrypt        --fuzz-dir fuzz -- -max_total_time=60
cargo +nightly fuzz run snapshot_parse      --fuzz-dir fuzz -- -max_total_time=60
```

A crash drops a reproducer into `fuzz/artifacts/<target>/crash-*`. Replay it
with:

```bash
cargo +nightly fuzz run <target> --fuzz-dir fuzz fuzz/artifacts/<target>/crash-<hash>
```

## Corpus

`fuzz/corpus/<target>/` is populated automatically on the first run and is
git-ignored. Seed corpora can be added by dropping files into that directory
before running.

## CI

`.github/workflows/fuzz-smoke.yml` runs each target for 30 s on every push
to `release/v0.2.0-beta`. The job is marked `continue-on-error: true` —
fuzzing is signal, not a gate.
