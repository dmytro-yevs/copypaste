# Changelog

## [0.1.0-alpha.1] - 2026-05-23

### Added
- macOS clipboard history daemon (NSPasteboard monitor, SQLite + SQLCipher storage)
- Unix IPC socket protocol (list, search, copy/paste-back, stats, pin, private mode)
- Full-text search (FTS5) over clipboard history
- CLI: list, search, copy (INDEX/--id/--search/--list), stats, watch, export, import, clear, pin
- Slint UI: HistoryWindow with paginated history, IPC wiring, dark Catppuccin theme
- Slint UI: SettingsWindow + PairWindow (fingerprint display, device pairing)
- macOS system tray icon (tray-icon crate) + launchd autostart
- XChaCha20-Poly1305 at-rest encryption for all clipboard content
- Sensitive data detection (27 password manager patterns, auto-wipe TTL)
- Private/pause mode (IPC toggle, daemon stops recording)
- SHA-256 content deduplication (60s window)
- Relay server: HTTP REST (Axum), device auth, push/pull sync, rate limiting, quotas
- P2P stack: mDNS-SD discovery, mutual TLS (rustls/rcgen), Lamport+LWW sync engine
- Supabase cloud sync: GoTrue auth, Realtime WebSocket (Phoenix Channel)
- Image clipboard: PNG/TIFF capture + WebP + chunked XChaCha20 encryption
- Android skeleton: UniFFI Rust↔Kotlin bindings, ClipboardService, SyncManager
- Windows: named-pipe IPC stub, platform abstraction traits
- CI: GitHub Actions (macOS + Windows matrix, cargo test + clippy + audit)
- Structured logging: rotating file appender, env-filter (COPYPASTE_LOG)

### Platform Support
- macOS 12+ (primary)
- Android (alpha skeleton)
- Windows (compilation stubs)
