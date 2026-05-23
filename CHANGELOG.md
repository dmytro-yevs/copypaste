# Changelog

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
