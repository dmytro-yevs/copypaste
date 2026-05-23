# Changelog

## [0.3.0-dev] — Unreleased

v0.3 development branch. Cut from release/v0.2.0-beta after Wave 5 verify-gate.
See docs/release/v0.3-plan.md for scope.

**Scope (2026-05-23):** dropped Windows (frozen — see
`docs/adr/ADR-012-windows-frozen-homebrew-only.md`). Distribution:
Homebrew Cask only (no Apple notarization, no Sparkle update feed).
Signed DMG continues to ship as a GitHub release asset for
reproducibility, but is not the promoted install path.

### Features
- **UI:** In-app auto-update via Homebrew Cask: daily check + notification +
  one-click upgrade. No Sparkle (Homebrew-only per ADR-012).

### Breaking changes
- **Crypto:** dropped the legacy empty-AAD AEAD decrypt fallback in
  `copypaste-core::crypto::encrypt`. The `encrypt_item` / `decrypt_item`
  wrapper functions (empty-AAD variants) have been removed entirely;
  callers must use `encrypt_item_with_aad` / `decrypt_item_with_aad`
  with `build_item_aad(item_id, AAD_SCHEMA_VERSION)`.

  **v0.2 → v0.3 upgrade path:** run `copypaste migrate v3` (which
  backfills AAD across the row population) BEFORE upgrading the daemon.
  If the v0.2 daemon is killed before the backfill completes, those rows
  are unreadable in v0.3 — this is a one-way break we are explicitly
  accepting in v0.3 in exchange for closing the substitution-attack
  surface that the empty-AAD fallback left open.

## [0.1.0-alpha.1] — 2026-05-23

### Added
- macOS daemon: NSPasteboard polling, Keychain X25519 keypair, launchd autostart, tray menu
- SQLCipher at-rest encryption with chunked XChaCha20-Poly1305 for clipboard content
- FTS5 full-text search across history
- CLI: list / search / copy / paste / clear / pin / private / status / count / export / stats
- Slint UI: HistoryWindow, SettingsWindow, PairWindow (pairing UI is preview)
- IPC: Unix socket with newline-delimited JSON; socket perms `0o600`; 16 MiB request cap
- Sensitive content detection with NFKC normalisation
- Cloud sync (Supabase): HTTPS-only, fail-closed auth, 401 refresh, 429 Retry-After
- Audit reports (4 audits + readiness): `docs/audit/2026-05-23-*.md`

### Security
- Random bearer tokens (was deterministic SHA256 of pubkey)
- Real cert fingerprints (was hostname+pid hash)
- Versioned HKDF salt
- Lamport clock saturating arithmetic
- Schema downgrade returns explicit error (was silent corruption)
- Concurrent writer integration test (3 tasks × 1000 items)
- TLS handshake 10s timeout

### Known issues
See `docs/known-issues.md`

### Architectural debt
See `docs/architectural-debt.md`
