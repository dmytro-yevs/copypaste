# CopyPaste — Feature Improvement Roadmap

All 128 PCA findings are assigned to exactly one phase. Every PCA-ID appears once.

---

## Phase 1 — Must Fix Before Release

Security/privacy issues, data loss paths, broken core flows, misleading device/sync status, broken pairing, and trust-affecting platform drift. Comprises the P0, the hard P1 release blockers, and P2 items that constitute true release blockers.

| PCA-ID | Severity | Title | Platforms |
|--------|----------|-------|-----------|
| PCA-001 | P0 | Silent DB-key regeneration wipes entire Android clipboard history | Android |
| PCA-002 | P1 | Sensitive plaintext indexed in FTS and returned by `search` over IPC | macOS, daemon, Linux |
| PCA-003 | P1 | Android `private_mode` toggle does not suppress capture | Android |
| PCA-004 | P1 | Sensitive images/files silently dropped at capture on Android | Android |
| PCA-005 | P1 | Trust label hardcoded "Verified"; no daemon-sourced trust field | macOS, Android, daemon |
| PCA-006 | P1 | Legacy `pair_peer` trusts a peer by asserted fingerprint with no PAKE/SAS | daemon |
| PCA-007 | P1 | `sync_enabled` master kill-switch stub / UI text drift | macOS, Android |
| PCA-008 | P1 | `export --include-sensitive` dumps all plaintext over IPC/stdout unguarded | macOS, CLI, Linux |
| PCA-009 | P1 | `revoke_all_peers` destructive action with weak/absent UI confirmation | macOS, Android, daemon |
| PCA-010 | P1 | Two divergent wire-incompatible IPC protocol definitions (`id: u64` vs `id: String`) | daemon, IPC, CLI |
| PCA-011 | P1 | `persist_private_mode` writes the flag file world-readable (0644) | macOS, Linux |
| PCA-012 | P1 | `NSFilenamesPboardType` fallback treats a plist XML array as a `file://` URL → files dropped | macOS |
| PCA-013 | P1 | PiiScrubber not applied to daemon tracing log sink (PII leak risk) | macOS, Linux |
| PCA-014 | P1 | `evict_stale_daemon` SIGTERMs an IPC-reported PID without kernel validation | macOS, Linux |
| PCA-015 | P1 | Android release APK falls back to debug-signed when keystore secrets absent | Android |
| PCA-016 | P1 | `replace_cloud_item_by_item_id` INSERT may omit `deleted` → tombstone resurrection | macOS |
| PCA-017 | P1 | Cloud (Supabase) sync does not carry `pinned`/`pin_order` to macOS | macOS |
| PCA-018 | P1 | `upsert_fts` non-atomic DELETE+INSERT → items permanently unsearchable | macOS, Android, Linux |
| PCA-019 | P1 | `revoke_device` non-atomic DELETE+INSERT → lost revocation audit | macOS, Android |
| PCA-020 | P1 | macOS bulk delete: no confirmation and no undo | macOS |
| PCA-021 | P1 | Android single-item delete: no confirmation/undo, propagates tombstone | Android |
| PCA-022 | P1 | macOS "Clear history" and "Revoke all" use misclick-prone inline confirms | macOS |
| PCA-023 | P1 | Android "Clear All" from Settings swallows errors, skips queue drain | Android |
| PCA-024 | P1 | Android shows raw exception text in user-facing toasts and QR error surface | Android |
| PCA-025 | P1 | Android `sync_on_wifi_only` toggle is dead; sync runs on cellular | Android |
| PCA-026 | P1 | Android `excludedAppBundleIds` privacy control is unenforceable | Android |
| PCA-027 | P1 | Android `lanVisibility` toggle does nothing; device always discoverable | Android |
| PCA-028 | P1 | Android logs and crash reports written to world-discoverable external storage | Android |
| PCA-029 | P1 | Android background clipboard capture unreliable on 10+ and self-contradictory | Android |
| PCA-044 | P2 | Android sensitive-item sync filtering unverified (no `isSensitive` in OutboundMutationQueue) | Android |
| PCA-097 | P2 | macOS Release ad-hoc signing / no notarisation (blocks 1.0 only) | macOS |

**Cross-cutting epics implied by Phase 1:**
- Epic: Kill all dead Android settings — wire or remove every inert toggle before shipping
- Epic: Real per-peer trust model — `list_peers` must emit `trust`; UIs must render unverified state
- Epic: Sensitive-data egress hardening — FTS, export, logs, and IPC must all enforce the same "sensitive stays controlled" invariant
- Epic: Atomic storage writes — every multi-statement storage operation must be in a transaction

---

## Phase 2 — Should Fix Soon

Partial features, missing platform parity, unreliable edge cases, important missing tests, weak UX states, and Android reliability gaps that do not block shipping but will generate user complaints immediately after launch.

| PCA-ID | Severity | Title | Platforms |
|--------|----------|-------|-----------|
| PCA-030 | P2 | Telemetry `PiiScrubber` implemented but never wired to any caller (dead code) | all |
| PCA-031 | P2 | `has_sensitive_items` swallows DB errors and returns `false` → sensitive data persists past TTL | macOS, Android, Linux |
| PCA-032 | P2 | `revoked_devices` table created outside versioned migrations | macOS, Android, Linux |
| PCA-033 | P2 | `reqwest::Client` fallback path has no timeout (cloud/IPC) → loops block forever | macOS, Linux |
| PCA-034 | P2 | Relay push loop drops items on transient failure (no retry queue / no backoff) | macOS |
| PCA-035 | P2 | Relay/P2P task JoinHandles dropped → panics silently kill sync subsystems | macOS, Linux |
| PCA-036 | P2 | Poisoned sync key-cache Mutex recovered silently (corrupt-key risk) | macOS, Android, Linux |
| PCA-037 | P2 | P2P `push_catchup` unbounded `send().await` → connector deadlock | macOS, Linux |
| PCA-038 | P2 | `migration_v4` unbounded `i64::MAX` fetch → OOM on large databases | macOS, Android, Linux |
| PCA-039 | P2 | Relay watermark not persisted across daemon restarts (cursor gap risk) | macOS |
| PCA-040 | P2 | Android relay ingest: image/file items bypass LWW (duplicate/stale on re-poll) | Android |
| PCA-041 | P2 | Android Lamport clock migration: old wall-millis values bias LWW against macOS | Android |
| PCA-042 | P2 | `sync_orch` auto-apply SQL `OR 1=1` may nullify the device filter | macOS |
| PCA-043 | P2 | Cloud passphrase change does not re-encrypt already-uploaded items | macOS, Android |
| PCA-045 | P2 | `delete_all` tombstones sequentially (N serial spawn_blocking) — slow on large history | macOS, Linux |
| PCA-046 | P2 | CLI import: 64 MiB file cap exceeds the 16 MiB IPC request cap → cryptic failure | macOS, Linux, CLI |
| PCA-047 | P2 | No per-request IPC read timeout; a stalled client holds a slot + DB Mutex indefinitely | macOS, Linux |
| PCA-048 | P2 | `lsappinfo front` forked every poll tick and blocks signal handling | macOS |
| PCA-049 | P2 | Self-write sentinel pre-stamp off-by-one under a 3rd-party write race | macOS |
| PCA-050 | P2 | File pre-check stat and read are separate spawn_blocking calls (TOCTOU) | macOS |
| PCA-051 | P2 | Broadcast `Lagged` drops to sync subscribers are unmetered | macOS, Linux |
| PCA-052 | P2 | Socket not cleaned up on SIGKILL/OOM/panic; no pid/lock file | macOS, Linux |
| PCA-053 | P2 | `AppLogger` Android: no redaction layer on external-storage logs | Android |
| PCA-054 | P2 | UDL zeroization contract not honored on the Kotlin side | Android |
| PCA-055 | P2 | Android auto-apply silently overwrites the user's current clipboard | Android |
| PCA-056 | P2 | Android sync failures are invisible to the user (silent Log.w only; 401 collapsed to empty) | Android |
| PCA-057 | P2 | Android outbound mutation queue drain is not periodic (pin/delete can stall) | Android |
| PCA-058 | P2 | Android P2P is Doze/OEM-kill fragile (no WakeLock; lifecycle tied to FGS) | Android |
| PCA-059 | P2 | Android P2P outbound has up to 30s latency vs daemon's near-immediate push | Android |
| PCA-060 | P2 | No sync-key rotation / revoke-and-rotate wired on Android | Android |
| PCA-061 | P2 | Sensitive items remain findable via search (masked only); FTS policy undocumented | macOS, Android, daemon |
| PCA-062 | P2 | Sensitive image/file TTL semantics diverge from text (`expires_at` vs `wall_time`) | macOS, Android, Linux |
| PCA-063 | P2 | FTS5 external-content index plaintext at-rest: design tradeoff undocumented | macOS, Android, Linux |
| PCA-064 | P2 | Sensitive-pattern keyword list misses common token forms | all |
| PCA-065 | P2 | Telemetry PII scrubber misses single-segment base64url tokens | all |
| PCA-066 | P2 | `derive_storage_key_v1` returns an unzeroized `[u8; 32]` | all |
| PCA-067 | P2 | `DeviceKeypair::ecdh` returns an unzeroized ECDH secret copy; stale doc reference | all |
| PCA-068 | P2 | Responder side shows SAS without the peer fingerprint | daemon, macOS, Android |
| PCA-069 | P2 | Online/offline derivation diverges across platforms and can mislead | daemon, macOS, Android |
| PCA-070 | P2 | Android image/file rows omit the source-app icon; reorder gesture diverges | Android, macOS |
| PCA-071 | P2 | Android image copy-back from PreviewOverlay uses a narrow SystemUI URI grant | Android |
| PCA-072 | P2 | `delete_all` (clear history) not reachable from the macOS history view | macOS |
| PCA-073 | P2 | macOS LogView: no offline/daemon-down state, no tests, raw error/path leakage | macOS |
| PCA-074 | P2 | HistoryView does not refresh after a successful backup import | macOS |
| PCA-075 | P2 | macOS accessibility permission prompt has no completion feedback (and no test) | macOS |
| PCA-076 | P2 | ErrorBoundary wraps only the whole app, not individual views | macOS |
| PCA-077 | P2 | macOS QR / SAS / daemon error surfaces render raw IPC strings (path/PII leakage) | macOS |
| PCA-078 | P2 | Tray "Private Mode" checkmark not re-synced after a daemon restart | macOS |
| PCA-079 | P2 | SyncStatusChip can show stale "connected" for up to 10s after going offline | macOS |
| PCA-080 | P2 | macOS private-mode active shows the wrong empty-state copy | macOS |
| PCA-081 | P2 | macOS "Revoke & rotate" / SAS confirm buttons show "..." with no accessible label | macOS |
| PCA-082 | P2 | Android `imageQuality` slider is dead (PNG@100 hardcoded) | macOS, Android |
| PCA-083 | P2 | Android `pasteAsPlainText` is a structural no-op | Android |
| PCA-084 | P2 | Android `SyncBadgeState::Syncing` never emitted; Android badge not driven by IPC `badge_state` | macOS, Android |
| PCA-085 | P2 | Android `maxHistoryItems` slider stored but never enforced | macOS, Android |
| PCA-086 | P2 | Android `logcatCaptureWorking` set optimistically → misleading "WORKING" status | Android |
| PCA-087 | P2 | Android dangerous-extension guard keyed on raw filename, not the sanitized name | Android |
| PCA-088 | P2 | Android notification denial silently degrades capture; no in-app warning | Android |
| PCA-089 | P2 | `armeabi-v7a` excluded from APK abiFilters — 32-bit devices silently stub | Android |
| PCA-090 | P2 | Android ABI mismatch is non-fatal; ABI-gate function may be renamed by R8 | Android |
| PCA-091 | P2 | Android SAS code is copyable to the clipboard during pairing | Android |
| PCA-092 | P2 | Relay `MAX_PULL_BYTES_BUDGET = 128 MiB` under a single global mutex (scale bottleneck) | relay |
| PCA-093 | P2 | `WireItem::clamp_timestamps` not enforced at deserialize; negative timestamps can persist | macOS, Android, Linux |
| PCA-094 | P2 | `cloud_sign_in`/`cloud_sign_out` undocumented wire methods; cloud controls return `not_implemented` in non-cloud builds | macOS, daemon |
| PCA-095 | P2 | Supabase Realtime embeds `apikey` in the WS URL query string; `user_id` filter is opt-in | macOS, Android |
| PCA-096 | P2 | Backup depends on external `sqlcipher` CLI (not bundled); restore doesn't stop the daemon; no round-trip test | macOS, Linux |
| PCA-099 | P3 | Mutating storage/IPC ops and clipboard wire methods have no backend/contract tests; daemon poll/IPC tests are `#[ignore]` | daemon, core, all |
| PCA-110 | P3 | CI gaps: no ESLint, `cargo test --all-features`, Android lint on PRs, instrumented tests, committed keystores, non-blocking quality jobs | all (CI) |
| PCA-111 | P3 | Test-coverage gaps: relay cross-device token, FTS-on-TTL, PAKE confirm, nonce/wrong-key, detector recall, ErrorCode round-trip, keychain ACL | all |
| PCA-118 | P3 | Android critical paths (capture/key/restart) + loaded-`.so` FFI errors untested; many tests cosmetic/copy-of-logic | Android |

**Cross-cutting epics implied by Phase 2:**
- Epic: Wire telemetry or remove it — the dead `copypaste-telemetry` crate and misleading privacy policy must be resolved
- Epic: Unified sync-status model — `SyncBadgeState::Syncing`, online/offline derivation, and the Android re-derived badge must converge on a single daemon-authoritative signal
- Epic: Android P2P reliability — Doze-aware strategy, WakeLock, bounded latency, supervised lifecycle
- Epic: Key hygiene sweep — zeroize every ECDH/storage key return value, honor UDL ByteArray zeroization contract

---

## Phase 3 — Product Polish

UI consistency, empty/error/loading states, onboarding, diagnostics, settings organization, and cosmetic parity gaps that do not affect correctness.

| PCA-ID | Severity | Title | Platforms |
|--------|----------|-------|-----------|
| PCA-098 | P3 | CLI lacks `reorder_pinned`, media commands, and device-management commands | CLI |
| PCA-100 | P3 | No search content-type filter; `search` response omits `preview`/`kind`/`pinned` | CLI, macOS, Android, daemon |
| PCA-101 | P3 | CLI `copy`/`delete`/`list`/`search` use legacy IPC methods and omit fields | macOS, Linux, CLI |
| PCA-102 | P3 | CLI destructive/exit hygiene: `restore` no confirm; `process::exit` skips Zeroizing drops; status output mixing | macOS, Linux, CLI |
| PCA-103 | P3 | No CLI `reset-database` command for degraded-mode recovery | macOS, CLI |
| PCA-104 | P3 | `SyncEngine`/`LamportClock` structs are dead on the production path (false test confidence) | macOS |
| PCA-105 | P3 | No jitter in relay/P2P backoff; mDNS IP-correlation can dial the wrong peer | macOS |
| PCA-106 | P3 | Relay receive loop: 401 during burst-drain loses progress; missing `originDeviceId` on Android relay push | macOS, Android |
| PCA-107 | P3 | Android `OutboundMutationQueue` Supabase tombstone push status unclear; Supabase poll limit=20 slow catch-up | macOS, Android |
| PCA-108 | P3 | Migration ladder integration tests end at v4 (v5–v11 untested on real files); FTS5 not rebuilt by vacuum | all |
| PCA-109 | P3 | Export silently skips image/file items with no user warning | macOS, Linux |
| PCA-112 | P3 | Android parity gaps: no P2P LAN sync, single-transport SyncBackend, missing recovery/export, SAS metadata, single-IP display | Android |
| PCA-113 | P3 | GUI maintenance/parity gaps: no DB-level backup/restore, no vacuum/stats UI; CLI backup bypasses the daemon | macOS, Android, CLI |
| PCA-114 | P3 | Theme-default doc/spec drift: PARITY-SPEC says light-first, both platforms default dark | macOS, Android, docs |
| PCA-115 | P3 | Android STUN public-IP response not transaction-ID-validated (spoofable, informational value only) | Android |
| PCA-116 | P3 | Pairing UX/dead-code gaps: CLI `pair-qr --raw` scroll-back; `poll_peer_events` wiring unverified; Android Devices states/dead helpers; empty pairing-notification name | CLI, daemon, Android, macOS |
| PCA-117 | P3 | `ClipboardFloatingActivity` duplicates `dispatchClipData`; `FgsSyncLoop` dies if dial-setup throws before the per-peer loop | Android |
| PCA-119 | P3 | Architecture/docs drift, dead constants, and `todo!()` test hazards | all (docs/CI) |
| PCA-120 | P3 | `#[allow(dead_code)]`/blanket-allow cleanup and unzeroized-on-takeover dead-code review | macOS, Linux |
| PCA-121 | P3 | Relay observability/precision: no version in `/health`, `registered_at` via `Instant::elapsed()`, silent-prune backpressure, 500 echoes internal strings | relay |
| PCA-122 | P3 | macOS UI polish: double stale-daemon banner, thin behavioral test coverage, protocol-mismatch message direction, sensitive-export safeguard | macOS |
| PCA-123 | P3 | `paste_to_frontmost` 80ms focus-delay race; QR/visibility edge UX; `ToastProvider` mounting unverified | macOS |
| PCA-124 | P3 | Remaining UX P3s: silent load-more/search-fallback failures, history error-flash, no scanning state, import warning, app-bundle-id exposure, Android scroll/i18n | macOS, Android |
| PCA-125 | P3 | `ip_with_port` 0.70-confidence pattern auto-wipes private IP:port pastes (false-positive data loss) | all |

**Cross-cutting epics implied by Phase 3:**
- Epic: CLI as a full first-class client — every daemon IPC method reachable from the CLI with typed methods and structured output
- Epic: Behavioral test coverage — replace skin/cosmetic tests with state-mapping/error-branch/offline tests per view on both platforms
- Epic: Settings organization audit — document which settings are platform-scoped vs universal; add "(macOS only)" labels where appropriate

---

## Phase 4 — Major Upgrades

Deeper sync reliability, conflict resolution, device management, backup/export flows, shared cross-platform state models, and automated parity testing infrastructure.

| PCA-ID | Severity | Title | Platforms |
|--------|----------|-------|-----------|
| PCA-126 | P4 | Orphaned/dead UI prefs and stubs: `previewSize`, Advanced tab, `supabase_account_id`, duplicated default shortcut | macOS |
| PCA-127 | P4 | FileChip list-row renders a hardcoded MIME (`application/octet-stream`) | macOS |
| PCA-128 | P4 | Low-impact platform/cosmetic items: rekey O(n) key scan, image dedup hash truncation, ad-hoc Keychain ThisDeviceOnly skip, launchd plist USERNAME, Linux log-path docs, tauri.conf version sed, Linux CI, db_key backup docs, Android `Log.d` insert-id | macOS, Android, Linux, docs |

**Cross-cutting epics implied by Phase 4:**
- Epic: Shared cross-platform state models — daemon-served, UniFFI-mirrored state for sync status, device trust, and settings so parity drift cannot recur silently
- Epic: Conflict resolution & operational visibility — relay/cloud burst-drain, Supabase tombstone audit, shard the relay store for scale
- Epic: Complete Android backup/export/import — history portability with sensitive-data gating
- Epic: Automated cross-platform parity CI — a CI job that runs the same scenario on macOS daemon + Android emulator and asserts identical outcomes (sync, delete, pin, sensitive detection)
- Epic: Key rotation UX — passphrase change must re-encrypt all cloud-stored items, with progress indication and blocking until complete
