# Contributing to CopyPaste

## Development Setup

```bash
# Start Docker dev environment
docker compose up -d dev

# Run tests
docker compose exec dev bash -c "cd /workspace && cargo test --workspace"

# Or use the helper script
bash scripts/dev.sh test
```

## Code Style

- `cargo fmt --all` before every commit
- `cargo clippy --workspace -- -D warnings` must pass (zero warnings)
- No `unwrap()` in non-test code unless infallible and commented
- `#[allow(...)]` requires a comment explaining why

## Commit Messages

Conventional commits: `feat(scope):`, `fix(scope):`, `chore:`, `docs:`, `bench:`, `test:`

Scopes: `core`, `daemon`, `cli`, `relay`, `app`, `android`

## Adding a New Platform

1. Add cfg-gated module to `crates/copypaste-daemon/src/platform/`
2. Implement `ClipboardBackend` + `KeystoreBackend` traits
3. Add build/CI matrix entry in `.github/workflows/ci.yml`
4. Add platform docs in `docs/`

## Security

See `SECURITY.md` for vulnerability reporting.

Never commit keys, tokens, or credentials. `cargo deny check` must pass.
