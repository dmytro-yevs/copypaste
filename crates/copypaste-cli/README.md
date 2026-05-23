# copypaste-cli

## Purpose
Command-line client for the CopyPaste daemon. Talks to the daemon over its Unix-socket IPC and exposes scripting-friendly subcommands.

## Public API
Binary-only crate (`src/main.rs` + `src/commands/`). Subcommands:

- `list` — list clipboard history (with `--limit`, `--offset`).
- `status` — daemon liveness, version, uptime, history count (`--json`).
- `count` — total stored items.
- `delete <id>` — delete by UUID.
- `search <query>` — FTS5 search.
- `copy [index|--id|--search]` — copy a past entry back to the clipboard.
- `pin` — pin/unpin items.
- `clear`, `vacuum` — maintenance.
- `export`, `import`, `backup` — round-trip JSON / encrypted backups.
- `daemon` — start/stop/status the local daemon.
- `private`, `watch`, `stats` — auxiliary.

Generated shell completions (bash / zsh / fish) ship via the `completions` subcommand.

## Platform support
All platforms where the daemon runs (macOS, Linux, Windows).

## Status
beta.

## Internal vs published
Internal binary crate. Not published to crates.io.

## Quick example

```bash
copypaste status --json
copypaste search "TODO" --limit 10
copypaste copy 1            # most recent
copypaste export --out backup.json
```

## Tests
2 integration tests under `tests/`: shell-completion generation, export/import round-trip.

```bash
cargo test -p copypaste-cli
```

## Related ADRs
- [ADR-002](../../docs/adr/ADR-002-unix-socket-ipc.md) — Unix-socket IPC contract.
- [ADR-007](../../docs/adr/ADR-007-ipc-protocol-versioning.md) — Wire protocol versioning.
