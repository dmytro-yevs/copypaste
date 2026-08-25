# ADR-0028: Centralize portable CI gates

## Decision

Keep portable CI commands in `config/ci-gates.json`. CI and local mirrors select
gate IDs through one runner, which resolves Rust from Cargo metadata.

## Dependency exemption

Exemption 1: no maintained package provides the required behaviour. We evaluated
[Task](https://taskfile.dev/docs/guide),
[just](https://github.com/casey/just), and
[act](https://github.com/nektos/act). Task and just execute shell recipes but do
not prove that distributed GitHub jobs select the same profile as WSL, and their
string recipes reintroduce the shell-source ambiguity this gate rejects. Act runs
workflow jobs in Docker instead of the named checkout's host toolchain and cache.
The runner therefore uses Python's standard JSON and process APIs with argv
arrays, while the wiring test owns the repository-specific execution-graph check.

## Consequence

Platform setup remains in workflow jobs; only commands portable to WSL share the
registry. Wiring tests reject missing, duplicate, or advisory substitutions.
