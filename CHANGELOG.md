# Changelog

## [0.3.2] - 2026-05-24

Post-install user feedback fixes from v0.3.1.

### Fixed
- **storage:** clipboard captures permanently blocked by stuck migration_state gate. Daemon now calls migration_v4_sweep_resumable + force_complete_if_no_v1_rows() on startup.
- **ui (macos):** app missing from Cmd-Tab when window open. Removed LSUIElement=true from bundle plist; app starts as .accessory via runtime objc2 call, flips to .regular when window visible.
- **ui:** Pair window unreachable. Added "Pair…" button to history toolbar + "Pair Device…" tray menu item, both wired to PairWindow.show().
- **ui:** long-text history rows showed only "…". Wrapped preview Text in clip-true Rectangle + pinned width to parent.width so elide truncates with tail ellipsis.

## [0.3.1] - 2026-05-24

Emergency release. v0.3.0 shipped broken; this release rolls up post-tag fixes from commits 06b8f84, 11b282a, and Wave 0 UI/daemon repairs.

### Fixed

- **paste:** "authentication tag mismatch" — IPC paste decrypt always used v1 AAD regardless of `key_version` (broke every `key_version=2` item)
- **ui:** `history_window.slint` HorizontalBox height constraints clipped Button text to zero (buttons rendered as black rectangles)
- **ui:** tray icon missing/grey — `load_icon` used hardcoded 22x22 dims instead of actual PNG dimensions
- **ui:** `tray-icon-active.png` added to tray icon candidate search list
- **ui:** Settings and Pair window callbacks now wired via `wire_to_ipc()` in `main.rs` (were firing into void)
- **ui:** `on_settings_requested` now opens `SettingsWindow` (was only logging to stderr)
- **ui:** tray `on_open_preferences` and `on_paste_item` callbacks wired (were `None`)
- **ui:** tray icon uses `.with_icon_as_template(true)` — adapts automatically to macOS dark/light menubar
- **ui:** tray `load_icon` fallback now emits `tracing::warn!` when bundle icon is missing (was silent)
- **ui:** history list rows no longer overlap — `clip: true`, `wrap: no-wrap`, `height: 18px` on preview `Text`
- **ui:** `SettingsWindow.app_version` bound to `env!("CARGO_PKG_VERSION")` (was hardcoded `"0.1.0"`)
- **macos:** app now appears in Cmd-Tab when window is open — `NSApp` activation policy toggles `.accessory` ↔ `.regular` on window show/hide
- **macos:** daemon failed to spawn after DMG install — `build-dmg-ci.sh` now copies `com.copypaste.daemon.plist` into `Contents/Resources/`; `-x` → `-f` guard fixed in `make_app_bundle.sh` and `make_dmg.sh` (CI strips exec bits)
- **storage:** schema v7 — added `pinned` column to prevent TTL prune deleting pinned items (data loss: prune only cleared `expires_at`, not the pin)
- **storage:** `pin_item` now sets `pinned=1`; added `unpin_item`; `delete_expired`/`prune_history` now filter `AND pinned=0`
- **ipc:** `delete_item`/`delete_fts` errors no longer silently swallowed; 3 server loops changed from `.ok()` to `if let Err` with logging
- **security:** `SessionKey` gains `ZeroizeOnDrop` (key material scrubbed on drop); `KeystoreBackend::load_or_create`, `local_enc_key`, and `load_local_key` now return `Zeroizing<[u8;32]>`
- **daemon:** `cloud.rs` — fixed 5 compile errors: `pinned` field added to `ClipboardItem` literals; `rx.recv()` double-borrow restructured
- **dist:** Android APK now built and uploaded in `release.yml` (Gradle `assembleDebug`)
- **dist:** `release.yml` auto-updates Homebrew Cask after publish
- **dist:** DMG scripts add `/Applications` symlink for drag-install UX
- **dist:** `xattr -cr` inside DMG image to clear quarantine on install
- **dist:** `build-dmg-ci.sh` fixed `CFBundleExecutable` substitution (`copypaste-daemon` → `copypaste-ui`)
- **dist:** Homebrew Cask repo URL, version, and DMG filename pattern fixed
- **ci:** Install OpenSSL on Windows runner for SQLCipher; pin `rust-toolchain.toml` to `channel = stable`

### Added

- **i18n:** 4 previously hardcoded strings wrapped in `@tr()`: search placeholder, "(coming soon)", and 2 Supabase UI placeholders
- **tests:** Slint headless + ViewModel test suite — 225 tests
- **android:** `versionName` bumped to `"0.3.1"`, `versionCode` → 4

### Known limitations

- Tray Private Mode IPC plumbing not wired (deferred to v0.4)
- QR pair flow incomplete (deferred to v0.4)
- APK is debug-signed (production signing in v0.3.2)
- Dead code in `src/tray_menu.rs` (cleanup deferred to v0.3.2)

## [0.3.0-dev] — Unreleased

v0.3 development branch. Cut from release/v0.2.0-beta after Wave 5 verify-gate.
See docs/release/v0.3-plan.md for scope.

**Scope (2026-05-23):** dropped Windows (frozen — see
`docs/adr/ADR-012-windows-frozen-homebrew-only.md`). Distribution:
Homebrew Cask only (no Apple notarization, no Sparkle update feed).
Signed DMG continues to ship as a GitHub release asset for
reproducibility, but is not the promoted install path.

### Features
- **UI:** Text preview in the history list is now capped at 1 024 bytes server-side
  (full content is still stored encrypted); large clipboard entries no longer stall
  the UI rendering thread. Image items show a `[image — id:XXXXXXXX]` placeholder;
  full rich preview is planned for v0.4.
- **UI:** In-app auto-update via Homebrew Cask: daily check + notification +
  one-click upgrade. No Sparkle (Homebrew-only per ADR-012).
- **Telemetry:** real Sentry SDK backend (opt-in, default `Disabled`). PII
  scrubber runs pre-send; `send_default_pii=false`,
  `traces_sample_rate=0.0`, `attach_stacktrace=false`. Disabled consent is
  a true no-op (no SDK init, no network). `sentry` dep is crate-local —
  not promoted to the workspace.

### Build infrastructure
- Native amd64 CI runner for Android (`ubuntu-latest-xlarge`, no Rosetta).
- Pre-baked OpenSSL 3.0.13 + SQLCipher 4.5.6 in the Android builder image,
  saving ~15–20 min of host-side C compile per cold build.
- sccache (Rust) + ccache (C) wired into the Android container, persisted
  across runs via `sccache-android` / `ccache-android` named volumes.
- `[profile.release]` switched to `lto = "thin"` for 30–50 % faster link
  time at ~5 % binary-size cost; `[profile.release-size]` re-pins `lto = "fat"`
  for size-critical mobile / embedded artifacts.
- `make android-docker` / `make android-docker-clean-cache` for incremental
  Docker builds; see `docs/release/build-perf.md`.

  **Cold-build envelope:** 30–60 min → 5–10 min on amd64-xlarge.
  **Warm-build envelope:** 5–10 min → 1–2 min for code-only changes.

### Breaking changes
- Removed `copypaste-config` crate (orthogonal to `core::config::AppConfig` and `daemon::ipc::AppConfig`; see ADR-011)
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
