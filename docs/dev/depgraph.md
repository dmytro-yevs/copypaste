# Workspace Dependency Graph

`scripts/dep-graph.sh` renders the Cargo workspace dependency graph as an SVG
using [`cargo-depgraph`](https://crates.io/crates/cargo-depgraph) and
[`graphviz`](https://graphviz.org). `scripts/find-cycles.sh` scans the same
graph and exits non-zero if any directed cycle is detected.

Both scripts are local dev tooling — outputs land in `reports/`, which is
gitignored.

## Prerequisites

```sh
cargo install cargo-depgraph
brew install graphviz          # macOS
sudo apt-get install graphviz  # Debian/Ubuntu
```

## Generating the graphs

```sh
# Workspace-only graph (the common case): reports/depgraph.svg
scripts/dep-graph.sh

# Full graph including external crates: reports/depgraph-full.svg
scripts/dep-graph.sh --full

# Custom output path
scripts/dep-graph.sh --output reports/deps-2026-05.svg

# Dry-run (print commands, no cargo invocation)
scripts/dep-graph.sh --dry-run
```

Open the SVG in a browser or any SVG viewer.

## Detecting cycles

```sh
scripts/find-cycles.sh           # workspace-only (default)
scripts/find-cycles.sh --full    # include external deps
```

Exit codes:

| Code | Meaning                                |
| ---- | -------------------------------------- |
| 0    | No cycles found.                       |
| 1    | One or more cycles detected (printed). |
| 127  | `cargo-depgraph` is not installed.     |

## When to regenerate

Regenerate the SVG whenever the workspace structure shifts:

- A new crate is added to or removed from the workspace.
- A `[dependencies]` edge between workspace crates is added or removed.
- A crate is split or merged.
- Before reviewing a refactor that moves modules across crate boundaries.
- Before a release, as a sanity check on layering.

Run `scripts/find-cycles.sh` on the same triggers — and ideally as a pre-merge
local check whenever workspace `Cargo.toml` files change.

## How to read the graph

`cargo depgraph` emits a directed graph where each node is a crate and each
edge `A -> B` means "crate `A` depends on crate `B`".

- **Top of the graph**: leaf crates that nothing else depends on (binaries,
  test crates).
- **Bottom of the graph**: foundational crates (`copypaste-core`, shared
  protocol types) that many others depend on.
- **Edge color / style**: `cargo-depgraph` uses different colors for normal,
  build, and dev dependencies. See the
  [cargo-depgraph README](https://crates.io/crates/cargo-depgraph) for the
  current legend.
- **A back-edge** (an arrow pointing "upward" against the general flow) is a
  red flag — that is exactly what `find-cycles.sh` is designed to catch.

A healthy workspace graph is a DAG with a small number of foundational nodes
at the bottom and a fan-out of consumers above them. If you see a wide cluster
of crates all pointing at each other, that is a hint to extract a shared
lower-level crate.
