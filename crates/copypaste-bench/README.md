# copypaste-bench

## Purpose
Beta-bonus Criterion benchmark harness for CopyPaste. The library half is empty; all measurable work lives in `benches/*.rs` and runs against public APIs of the production crates — no impl reaching.

## Public API
None. `src/lib.rs` is a doc-only stub. `publish = false`; not a release artifact.

## Benchmarks
Declared in `Cargo.toml` (`harness = false`, Criterion-owned):

| Bench        | Crate under test | What it measures                                                  |
| ------------ | ---------------- | ----------------------------------------------------------------- |
| `encryption` | `copypaste-core` | XChaCha20-Poly1305 encrypt / decrypt / roundtrip @ 1 KB / 100 KB / 10 MB |
| `storage`    | `copypaste-core` | SQLite insert + list throughput                                   |
| `hash`       | (self / `sha2`)  | SHA-256 throughput across payload sizes                           |
| `sync`       | `copypaste-sync` | Diff / patch synchronisation                                      |
| `ipc_roundtrip` | `copypaste-ipc` | JSON serialize → bytes → deserialize for Request / Response (small Ping, medium HistoryList 100, large Import 1000) |
| `cli_parse`  | `clap` derive grammar | `Parser::parse_from` cost for the five hot CLI subcommands (`pin`, `history`, `export`, `import --dedup`, `daemon start`) |

## Platform support
All platforms.

## Status
beta-bonus (not on the critical path; runs on demand).

## Internal vs published
Internal workspace crate. `publish = false`. Not published to crates.io.

## Quick example

```bash
# Fast sanity check (no measurement).
cargo build -p copypaste-bench --benches

# Run one bench (HTML report → target/criterion/).
cargo bench -p copypaste-bench --bench encryption
cargo bench -p copypaste-bench --bench ipc_roundtrip
cargo bench -p copypaste-bench --bench cli_parse

# Run everything.
cargo bench -p copypaste-bench
```

Criterion writes HTML reports to `target/criterion/<group>/<bench>/report/` (iteration time, throughput, sample distribution plot).

## Tests
No `tests/` directory. Benchmarks themselves serve as smoke tests.

```bash
cargo test -p copypaste-bench   # no-op (no unit/integration tests)
```

## CI guidance
Do not run `cargo bench` on host for routine commits — slow and noisy. The recommended pre-commit smoke test is:

```bash
cargo build -p copypaste-bench --benches
```

Full bench runs should happen on a dedicated, quiesced machine; results land under `docs/benchmarks/runs/`.

[Criterion]: https://docs.rs/criterion
