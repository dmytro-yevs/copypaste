# Changelog

All notable changes are documented here.

## [Unreleased]

### Added (Phase 2a-3 + extras)
- FTS5 full-text search in clipboard history (`search` IPC + CLI command)
- SQLCipher at-rest encryption (in progress)
- Tauri 2 macOS menu bar app with tray icon, search, copy, delete
- Relay SQLite persistence (survives restarts)
- Relay v2: constant-time token auth, rate limiting (60 req/min), 500-item quota
- Android UniFFI Rust crate skeleton + Kotlin app scaffold
- CLI: `search`, `copy`, `watch`, `export`, `clear`, `stats` commands
- Platform abstraction traits (ClipboardBackend, KeystoreBackend) for Windows/Linux
- GitHub Actions CI/CD matrix (ubuntu + macos) + release pipeline
- Criterion benchmarks for crypto operations
- ADR documents: XChaCha20 choice, Unix socket IPC, SQLCipher
- `history_limit` enforcement: daemon prunes oldest items after each insert

### Fixed
- Zeroized IKM copy in `local_enc_key()` (security fix)

## [0.1.0] — Phase 0+1 — 2026-05-22

### Added
- `copypaste-core`: XChaCha20-Poly1305 encryption, X25519 keypair, HKDF key derivation
- `copypaste-core`: SQLite WAL storage with FTS5 schema
- `copypaste-core`: Sensitive data detection (20+ pattern types: AWS, GitHub, Stripe, JWT...)
- `copypaste-core`: AppConfig with TOML load/save
- `copypaste-daemon`: macOS clipboard monitor (NSPasteboard polling)
- `copypaste-daemon`: macOS Keychain key storage
- `copypaste-daemon`: Unix socket IPC (list/delete/count/status)
- `copypaste-daemon`: launchd user agent integration
- `copypaste-relay`: HTTP REST relay server (device registration, item sync)
- `copypaste-cli`: CLI with list/count/delete/status/copy/search/watch/export/clear/stats
- Docker Compose dev environment (Rust 1.75)
