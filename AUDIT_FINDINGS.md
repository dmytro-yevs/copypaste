# CopyPaste — Detailed Audit Findings

Date: 2026-06-19 · Tracking issue: CopyPaste-o7me · Auditors: 10 parallel read-only review streams + local quality gates.

Severity scale: **P0** critical (exploitable now / key or plaintext compromise / data loss) · **P1** high · **P2** medium · **P3** low.

Method: static code reading (Rust compile gates blocked locally — see Gate Results). Every finding cites `file:line`. Where a reviewer's severity was adjusted during synthesis, the original is noted.

---

## Gate Results (run locally)

| Gate | Result |
|---|---|
| `cargo audit` | ✅ no advisories |
| `cargo deny check` | ✅ advisories/bans/licenses/sources ok — 1 stale ignore (`RUSTSEC-2024-0429`, `deny.toml:37`, matches nothing) |
| `node scripts/parity-check.mjs` | ✅ 53/53 design tokens within ±5/255 |
| UI `vitest run` | ✅ 171 tests / 18 files passed |
| UI `tsc && vite build` | ✅ typecheck + production build clean |
| `cargo fmt --check` / `clippy -D warnings` / `cargo test` / `cargo check` | ⛔ **BLOCKED** — local toolchain rustc **1.95.0** < workspace MSRV **1.96** (`Cargo.toml:26`). CI installs ≥1.96 so this is a local-env limitation, not a repo defect. Not forced via MSRV edit (out of scope for a non-destructive audit). |

---

## P0 — Critical

**None confirmed.** No exploitable-now path to key material, plaintext exfiltration, MITM, nonce reuse, or data loss was found. The Tauri null-CSP (UI reviewer rated P0) is down-classified to P1 below because no injection sink currently reaches the WebView; it remains the top hardening item.

---

## P1 — High

### P1-1 [Security/Privacy] Sensitive items ARE uploaded to relay/cloud/P2P despite a documented "never uploaded" guarantee
- **Files:** `crates/copypaste-daemon/src/relay.rs:564-684` (push loop), `:459-520` (build_content), `:1633`/`:1847` (`is_sensitive: false` hardcoded on the wire item); `docs/relay-api.md:105` ("Sensitive items are **never uploaded**").
- **What:** The relay/cloud/P2P push paths have no `if item.is_sensitive { continue; }` guard. The outbound wire item hardcodes `is_sensitive: false`. So an item detected as sensitive (API key, card, SSH key) is re-encrypted under the sync key and pushed to the relay/cloud and to peers; the receiver re-derives sensitivity from plaintext (`sync_orch.rs:670-679`).
- **Why it matters:** Items never leave in plaintext (still E2E-encrypted), so this is **not** a key/plaintext compromise. But it directly contradicts a stated security guarantee. A user who enabled sync trusting "sensitive never leaves this device" has encrypted secrets sitting in the relay inbox (TTL ≥ 24h) and replicated to every paired device. Verified inline (`rg is_sensitive crates/copypaste-daemon/src/relay.rs`).
- **Verify:** Copy an API key, enable relay sync, inspect the relay DB inbox — a ciphertext row appears.
- **Fix:** Decide the intended contract. Either (a) add `if item.is_sensitive { continue; }` to the relay/cloud/P2P push paths to honor the doc, or (b) update `relay-api.md:105` + README to state sensitive items DO sync (encrypted) and remove the false guarantee. Recommend (a) for relay/cloud, optionally allow P2P.
- **Tests:** Yes — a daemon test asserting a sensitive item never enters `pending_uploads`/push channel.

### P1-2 [Privacy] App-exclusion silently fails open when `lsappinfo` errors → password-manager copies captured
- **Files:** `crates/copypaste-daemon/src/daemon.rs:1596-1597`.
- **What:** `frontmost_bundle_id` is obtained via `Command::new("lsappinfo")...output().ok()`. On any failure `.ok()` yields `None`, the exclusion check is skipped, and capture proceeds as if no exclusion list existed — so 1Password/Bitwarden copies get ingested.
- **Why:** Fail-open on a privacy control. Silent (no warn log).
- **Verify:** Make `lsappinfo` unavailable; copy from an excluded app → item is stored.
- **Fix:** `tracing::warn!` on failure; consider failing closed (skip capture) when the exclusion list is non-empty and frontmost is unknown.
- **Tests:** Yes — tick test with non-empty exclusion list and a stubbed lsappinfo failure.

### P1-3 [Reliability] Blocking `lsappinfo` subprocess on the async tick path stalls the tokio runtime
- **Files:** `crates/copypaste-daemon/src/daemon.rs:1594-1610`.
- **What:** `std::process::Command::new("lsappinfo").output()` (a blocking fork+wait) is called directly inside async `handle_tick`, no `spawn_blocking`. Fires every tick (default 500 ms) whenever `excluded_app_bundle_ids` is non-empty.
- **Why:** Can stall the runtime tens–hundreds of ms per tick under load; blocks IPC/sync tasks on the same worker.
- **Fix:** Wrap in `tokio::task::spawn_blocking(...).await`.
- **Tests:** Async tick test with exclusion list set.

### P1-4 [Reliability] `unreachable!()` can panic-crash the daemon
- **Files:** `crates/copypaste-daemon/src/daemon.rs:152` (`KeyLoad::Locked => unreachable!("Open plan implies a Ready key")`).
- **What:** Relies on an internal invariant of `decide_db_startup` (`:2390`), not a type guarantee. A future variant or refactor makes this reachable → process abort.
- **Fix:** Replace with a graceful `run_degraded(...)` fallback + `tracing::error!`.
- **Tests:** Unit test feeding the `Open`+`Locked` combination.

### P1-5 [Reliability/Linux] systemd unit `ReadWritePaths` is a macOS path → Linux DB writes fail silently (EROFS)
- **Files:** `contrib/systemd/copypaste-daemon.service:16,18` (`ProtectHome=read-only` + `ReadWritePaths=%h/Library/Application Support/CopyPaste`).
- **What:** Linux DB path is `~/.local/share/copypaste/`, not `~/Library/...`. With `ProtectHome=read-only`, the real DB dir is read-only → every capture/sync write fails. Daemon starts but silently discards all captures.
- **Fix:** `ReadWritePaths=%h/.local/share/copypaste` (XDG).
- **Tests:** Linux integration smoke under systemd.

### P1-6 [Architecture/Security] CLI writes the macOS Keychain directly AND sends the Supabase password in plaintext over IPC
- **Files:** `crates/copypaste-cli/src/commands/cloud.rs:42-58` (`set_generic_password`, service `com.copypaste.daemon`), `:163-180` (also sends `supabase_password` in `set_config` IPC); `crates/copypaste-cli/Cargo.toml:31-34` (`security-framework` dep); daemon side `ipc.rs:4559-4596`. Four `FIXWAVE` comments acknowledge it's half-done.
- **What:** Breaks the "daemon is the ONLY Keychain owner" contract. The CLI links `security-framework` and writes the Keychain, but the daemon doesn't read that entry back, so the password is *also* sent in cleartext over the socket. On non-macOS / ephemeral builds it stays in `config.json` (0600).
- **Why:** Boundary violation + residual plaintext-secret exposure window (IPC round-trip; on-disk on non-macOS).
- **Fix:** Move password storage into a daemon IPC verb (daemon does the Keychain write), drop the CLI `security-framework` dep, remove the plaintext IPC field.
- **Tests:** Daemon-side Keychain read of `supabase-password`; assert CLI never transmits the password.

### P1-7 [Security hardening] Tauri WebView has no Content-Security-Policy (`"csp": null`)  *(UI reviewer: P0)*
- **Files:** `crates/copypaste-ui/src-tauri/tauri.conf.json:32`.
- **What:** `"csp": null` → no CSP on a WebView that has Tauri IPC access. No current injection sink found (QR SVG is generated by the Rust `qrcode` crate; no `dangerouslySetInnerHTML` on untrusted data), hence P1 not P0 — but any future XSS would have full IPC + network reach.
- **Fix:** Set a strict CSP, e.g. `default-src 'self'; script-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:; style-src 'self' 'unsafe-inline'`.
- **Tests:** Integration test asserting the CSP header is present on the WebView.

### P1-8 [Data hygiene] Android retains the raw 32-byte DB key (unzeroized) in `DB_BY_PATH`, surviving `close_database`
- **Files:** `crates/copypaste-android/src/lib.rs:2257-2277` (`Mutex<HashMap<(String,[u8;32]), Database>>`), `:2335-2341` (`close_database` does not evict).
- **What:** The 32-byte key is part of the cache map key and lives on the heap for the cache lifetime; `close_database` removes from `DB_HANDLES` but not `DB_BY_PATH`. All call-site keys are `Zeroizing`, but the extracted `[u8;32]` map key bypasses zeroization. Also a use-after-close footgun (P2-12).
- **Fix:** Key the cache by a hash of the key bytes (e.g. `blake3::hash(&key)`), or a separate path→handle index; evict on close.
- **Tests:** Open→close→assert no live key bytes remain in the cache.

### P1-9 [Build/Release] daemon `Cargo.toml` missing `rust-version`
- **Files:** `crates/copypaste-daemon/Cargo.toml` ([package] has no `rust-version`); all 11 other crates use `rust-version.workspace = true`.
- **What:** The most complex crate lacks the MSRV guard; a too-old toolchain would not report an MSRV error for it.
- **Fix:** Add `rust-version.workspace = true`.

### P1-10 [Release] Version drift: `package.json` and Android Gradle defaults stuck at 0.7.1 while workspace/tauri/cask are 0.7.4
- **Files:** `crates/copypaste-ui/package.json:4` (**0.7.1**), `android/app/build.gradle.kts:106-107` (`versionName` **0.7.1** / `versionCode` **701**) vs `Cargo.toml:19` (0.7.4), `tauri.conf.json:4` (0.7.4), `Casks/copypaste.rb:4` (0.7.4), `CHANGELOG.md:3` (0.7.4). Verified inline.
- **What:** pnpm/Tauri version detection and locally-built APKs report 0.7.1; release CI overrides Gradle from the tag but never patches `package.json`.
- **Fix:** Single-source the version: patch `package.json` + Gradle defaults in the release workflow (`release.yml:108-130`), or a `set-version` script writing all files.

### P1-11 [Docs] `docs/known-issues.md` is missing — README links it (dead link)
- **Files:** `README.md:128` → `docs/known-issues.md` (absent — verified `ls` 404).
- **Fix:** Create the file or remove/replace the link.

### P1-12 [Docs] README claims native x86_64 (Intel) support, but release builds arm64-only
- **Files:** `README.md:42`, `.github/workflows/release.yml:77` (only `aarch64-apple-darwin`), `Casks/copypaste.rb:10` (arm64 DMG). `scripts/build-macos.sh` supports `universal` but isn't wired to CI.
- **What:** Intel Macs run the arm64 binary under Rosetta 2; README implies a native Intel build.
- **Fix:** Publish a universal DMG (add `lipo` to release.yml) **or** state in README that x86_64 is via Rosetta 2.

### P1-13 [Docs] `docs/protocol.md` omits three stable error codes
- **Files:** `docs/protocol.md` (error table) vs `crates/copypaste-daemon/src/protocol.rs` — missing `version_mismatch`, `migration_in_progress`, `rate_limited`.
- **Fix:** Add the three codes with semantics to the spec table.

---

## P2 — Medium

### Sensitive detector — false-positives above the 0.70 auto-wipe floor (silent deletion of wanted data)
- **Files:** `crates/copypaste-core/src/sensitive/patterns.rs`.
  - `discord_bot_token` (`:71-76`, conf 0.85) — over-broad dot-separated shape; FP triggers auto-wipe.
  - `twilio_auth_token` `SK[a-f0-9]{32}` (`:77`, 0.90) — matches Twilio **SID** not the token, no `\b`, matches hex substrings.
  - `ssn_us` (`:145`, 0.80) — matches dates/units like `012 31 2024`; no structural validator.
  - `IBAN` (`:139-143`, 0.85) — auto-wipes legitimately-copied bank details; the `is_sensitive_for_autowipe` doc claims Financial is excluded but code includes it (0.85 ≥ 0.70).
  - `generic_bearer` (`:113-117`, 0.80) — fires on tutorial/mock `Bearer …` strings; no entropy guard.
- **Why:** Auto-wipe (`detector.rs:186-225`) gates on confidence only, not category → benign data silently deleted after TTL.
- **Fix:** Lower these below 0.69, or add structural/entropy validators (mirror `is_credential_value_strong`). Add FP tests.

### P2 — `openai_legacy` pattern comment claims a non-existent `(?!proj-)` lookahead
- **Files:** `patterns.rs:53`, `detector.rs:862`. Misleading comment + real FP risk on any 48-char alnum after `sk-`. Fix comment or add the lookahead.

### P2 — Missing detector coverage: Azure / GCP service-account JSON / Cloudflare / SendGrid / Terraform Cloud
- **Files:** `patterns.rs` (absent). Add prefixed-token patterns (0.9+) common in dev clipboards.

### P2 — No startup purge of already-expired sensitive items
- **Files:** `daemon.rs:1178-1256` (TTL cleanup only inside tick loop). Sub-second window after DB open / before first tick where expired sensitive items are readable/searchable. Fix: run `run_ttl_cleanup(sensitive+general)` once after DB open, before binding the socket.

### P2 — `export` IPC returns bulk decrypted plaintext (incl. sensitive) with no audit log
- **Files:** `daemon.rs:7605-7757`. By design (same-uid socket, 0600) but unlogged and unthrottled; an `include_sensitive` flag + audit log is prudent.

### P2 — IPC: daemon emits `invalid_argument` on version gate, but CLI listens for `version_mismatch` (dead upgrade prompt)
- **Files:** `daemon ipc.rs:3202-3209`, `cli ipc.rs:173`, `ADR-007:48-50`. Friendly "upgrade CLI/restart daemon" message never shows. Fix: daemon should emit `ERR_CODE_VERSION_MISMATCH`.

### P2 — IPC: legacy `delete`/`copy`/`paste`/`set_private_mode` arms return `Response::err` without `error_code`
- **Files:** `daemon ipc.rs:3288-3298`, `:5077`. Machine clients can't classify the error. Fix: tag `ERR_CODE_INVALID_ARGUMENT`.

### P2 — `cloud setup` transmits `supabase_password` over IPC (see P1-6); on non-macOS persists to config.json
- **Files:** `cli/commands/cloud.rs:158-180`, `daemon ipc.rs:4559-4596`.

### P2 — `pair-qr --raw` prints the single-use pairing token to stdout with no warning
- **Files:** `cli/commands/pair_qr.rs:34-36`. 120 s TTL token can land in shell history/logs. Fix: stderr "do not share / expires in Ns" note.

### P2 — `copypaste-core` ships `tracing-subscriber` + `tracing-appender` as production deps and exports `init_global`
- **Files:** `copypaste-core/Cargo.toml:34,38`, `src/logging.rs`. Library owns a global subscriber initializer (double-init panics; bloats Android `.so` with a file appender it can't use). Fix: move `init_global`/`init_with_file_rotation` to binary crates; keep `tracing-subscriber` as a core dev-dep only.

### P2 — Nine `spawn_blocking` closures capture an unzeroized `[u8;32]` key copy
- **Files:** `daemon ipc.rs:3882,4015,4208,7449,7616,7838,8005,8102,8159` (`let v1_key: [u8;32] = **self.local_key;`). The double-deref drops the `Zeroizing` wrapper; bytes linger in the worker thread on panic/cancel. Correct pattern at `:5200/5206`. Fix: wrap each in `zeroize::Zeroizing::new(...)`.

### P2 — Android `close_database` doesn't evict the `DB_BY_PATH` connection cache (use-after-close)
- **Files:** `android/lib.rs:2335-2341`. After close, `with_cached_db` keeps reusing the cached connection. Fix: evict on close, or document `DB_BY_PATH` as a persistent pool.

### P2 — Android `eprintln!` in P2P path may reach logcat
- **Files:** `android/lib.rs:~1694`. Count-only (no item data) but stderr→logcat is not guaranteed-silent. Fix: `tracing::debug!`/`android_logger`.

### P2 — `relay-api.md` badly drifted from the implemented protocol
- **Files:** `docs/relay-api.md` vs `crates/copypaste-relay/src/{state.rs,models.rs,routes/mod.rs}`.
  - Auth: doc says token = first 32 hex of SHA-256(public_key) (`:5`); code issues a **random** 16-byte→32-hex token at registration (`state.rs:800-804`), explicitly NOT derived from the pubkey.
  - Protocol fields: doc documents `nonce`/`sender_device_id`/`lamport_ts`/`since_lamport`; code uses `content_type`/`content_b64`/`wall_time` and `?since=&since_id=&limit=`. None of the documented fields exist.
  - Response field `token` vs actual `auth_token` (+`expires_at`).
  - Undocumented routes: `GET /devices`, `GET /devices/{id}`, `GET /stats`, `GET /metrics`, `GET /devices/{id}/subscribe` (SSE).
- **Fix:** Rewrite `relay-api.md` to the wall-clock protocol + random-token auth; document all routes.

### P2 — `SECURITY.md` describes a *less secure* design than what ships (stale, misleading)
- **Files:** `SECURITY.md:45` ("Bearer token = SHA-256 of public key"), `:49` ("relay bearer tokens use string equality … not constant-time"). Actual code: random per-registration token (`state.rs:800`) compared with `subtle::ct_eq` (`state.rs:1009`, no early-exit). Also `:33-36` presents Windows DPAPI / Linux Secret Service as facts, but both are `unimplemented!()` stubs (Windows frozen, ADR-012). Fix: update SECURITY.md to match shipped code; mark Windows/Linux keystore as ephemeral-fallback.

### P2 — `ARCHITECTURE.md` crate graph omits 7 of 12 crates; README omits most; protocol.md method list incomplete
- **Files:** `ARCHITECTURE.md:4-11` (missing ipc/p2p/sync/supabase/android/telemetry/bench), `README.md:55-59` (omits daemon/ui/ipc/p2p/sync/supabase/telemetry/bench), `docs/protocol.md` (lists 7 methods; `copypaste-ipc/src/methods.rs` defines 27). Fix: regenerate the architecture/method sections.

### P2 — `audit.yml` `--no-fetch || cargo audit` retry can mask a real advisory hit
- **Files:** `.github/workflows/audit.yml:46`, `ci.yml:118`. Fix: `cargo audit --no-fetch || { cargo audit --db-update && cargo audit --no-fetch; }`.

### P2 — `#[allow(...)]` without the required inline comment
- **Files:** `daemon/src/lib.rs:14` (`#![allow(dead_code)]`), `daemon/src/main.rs:1`, `copypaste-ui/src-tauri/src/event_tap.rs:1` (`non_upper_case_globals`), `daemon/src/keychain/acl.rs:1`. Project rule: each `#[allow]` needs an explanatory comment on the line.

### P2 — Orphan file `daemon/src/ipc_win.rs` is never `mod`-declared (dead on disk)
- **Files:** `daemon/src/ipc_win.rs` (no `mod ipc_win` anywhere). Misleads contributors into thinking Windows IPC compiles. Fix: `#[cfg(windows)] mod ipc_win;` or delete + note in ADR-012.

### P2 — Relay `GET /devices` (unauthenticated) lists inbox UUIDs
- **Files:** `crates/copypaste-relay/src/routes/mod.rs:293-298`. Only IP rate-limited; the `device_id`s are the sync-key-derived inbox UUIDs. Enables enumeration/traffic analysis. Fix: require bearer token or remove the endpoint; document.

### P2 — UI parity gaps
- **Default theme is dark, spec mandates light-first** — `src/index.html:12`, `src/store.ts:96` vs PARITY-SPEC §0.
- **Liquid-Blue drift only half-fixed** — `IdeAccent` updated to `#4D8DFF` but `IdeSelection`/`IdeMultiSel` still `#3D8BFF` (`android/.../Color.kt:16,34`).
- **No export/import or backup/restore UI** — `src/lib/ipc.ts` has no wrappers; CLI has these.
- **IMAGE chip uses sky not violet** — `ContentIcon.tsx:119,210` vs spec §6 (intentional per comment — confirm).

### P2 — UI surfaces raw error strings (paths) in the DOM
- **Files:** `src/App.tsx:482-484` (daemon spawn error w/ bundle/socket paths), `src/components/ErrorBoundary.tsx:58-60` (`error.message`). Local-only, not secrets, but leaks install internals. Fix: sanitize before render.

---

## P3 — Low

- **Deprecated empty-AAD `encrypt_item`/`decrypt_item` still `pub`** — `core/src/crypto/encrypt.rs`. Zero production callers, but a future/FFI caller could get empty-AAD ciphertext (item-swap/replay). Fix: `pub(crate)`/`#[doc(hidden)]` or feature-gate.
- **Relay HKDF `None` salt** — `core/src/relay.rs:59,85` (`Hkdf::new(None, sync_key)`) diverges from the codebase's salted pattern. IKM is 32-byte high-entropy so not a break; add a frozen relay salt for defense-in-depth (migration-sensitive).
- **Plaintext decrypted buffers not `Zeroizing`** in the export path — `daemon ipc.rs` export handler. Best-effort hygiene.
- **PoP `[u8;32]` return not `Zeroizing`** — `core` `derive_relay_registration_pop`.
- **Rapid-burst: intermediate clipboard items permanently lost** (structural NSPasteboard polling limit) — `clipboard.rs:596-621`; `SkippedBatch` arm now dead code (`daemon.rs:1794-1803`).
- **`poll_interval_ms` not hot-reloaded** — interval timer created once (`daemon.rs:1178`), not recreated on config change.
- **`AppConfig::load` swallows TOML parse errors** silently to defaults — `daemon.rs:2538`. Add a warn log.
- **File read-before-size-gate** — `daemon.rs:1733-1742` reads the whole file into memory before the size check; add a `metadata().len()` pre-check. (Images: `clipboard.rs:461-465` read before gate too.)
- **Android: Kotlin `CopypasteException` missing `Panicked` variant** — `CopypasteBindings.kt:80-85`; Rust panics surface as generic `DatabaseError`. Add the variant.
- **Android: no Appearance/palette screen** — palette support exists in `Theme.kt` but no UI surface (within CopyPaste-ojsq scope).
- **Android ABI strict-equality gate** — `version.rs check_compatibility` forces a coordinated Play release for any additive UDL change (by design; engineering awareness).
- **mDNS-SD broadcasts device name + stable device UUID in cleartext** — `p2p/src/discovery.rs:33-39,374-455`. Inherent to mDNS (like Bonjour); consider a rotating discovery id + name behind PAKE.
- **Stale "MSRV 1.89" comments** after the 1.96 bump — `core/daemon/cli/relay/android/bench` Cargo.toml + `telemetry/scrubber.rs`.
- **`core-foundation` 0.9 (daemon) vs 0.10 (ui)** — separate binaries so no link clash; add a comment or align.
- **`r2d2`/`r2d2_sqlite` not promoted to workspace deps** — `core/Cargo.toml:26-27`.
- **Relay 15+ `#[allow(dead_code)]` on quota/stats methods** — `relay/src/state.rs`,`quota.rs`; tier/quota system written but unwired into routes (comments present, so policy-compliant; completeness concern).
- **Windows stub `unimplemented!()` without messages** — `platform/windows.rs:43,47` (frozen).
- **`deny.toml` stale ignore `RUSTSEC-2024-0429`** — matches nothing now; track for removal.
- **`ci-matrix.yml` scoped to `release/v0.2.0-beta` only** — `:8-10`; main PRs skip beta toolchain/doc-build/machete. Broaden to `[main, "release/**"]`.
- **Cask `depends_on macos: :sonoma`** (14+) not noted in README supported-platforms — `Casks/copypaste.rb:22`.
- **`open_item_file` temp files never cleaned** — `ui src-tauri/src/ipc.rs:402-403`; can leak opened item content to `$TMPDIR`.
- **`ADR-010` references stale artifact path** `target/release/CopyPaste.dmg` vs actual `dist/*.dmg` — `ADR-010:63`.
- **`SECURITY.md` disclosure email is a placeholder** — `SECURITY.md:14` (`security@copypaste.app (placeholder)`).

---

## Notable items VERIFIED CORRECT (high-value confirmations)

**Crypto:** XChaCha20-Poly1305 with 24-byte OsRng nonces (no AES-GCM); AAD binds `(item_id, schema_version, key_version)`; all HKDF info-strings domain-separated & distinct; `ZeroizeOnDrop` on every secret type; Keychain `ThisDeviceOnly`, no silent ephemeral fallback on macOS; constant-time compares (relay token/PoP, PAKE confirm tags, pairing token, sync key); Argon2id (m=19 MiB,t=2) exceeds RFC 9106; OPAQUE aPAKE bound to the TLS exporter (RFC 5705) → relay-MITM aborts; SAS un-bypassable; no long-term secret in the QR image.

**Sync/P2P:** mTLS authenticates by pinned SHA-256 cert fingerprint (rejects any other cert) — no MITM; tombstone delete-before-create resurrection bug fixed + regression-tested; hostile-peer timestamp clamping; per-transport key separation; cloud `SyncKey` provisioned only over the post-PAKE tunnel; Supabase RLS + client-side `user_id` filter.

**Storage:** SQLCipher actually enabled, fails closed on wrong key; FTS5 plaintext isolated to `clipboard_fts`; **every** delete/clear/TTL/evict path removes FTS rows in the same transaction (no orphaned searchable plaintext); migrations additive/idempotent/atomic with downgrade detection; private mode skips store **and** sync; NFKC bypass guard.

**Daemon:** socket `chmod 0600` (+ dir 0700), tested; self-copy echo-loop guard correct; SIGTERM/SIGINT graceful shutdown, stale-socket cleanup, socket removed on exit; no prod `unwrap()`; autorelease pools per poll tick.

**Relay:** ciphertext-only storage (plain SQLite, never `PRAGMA key`); no secrets/tokens/bodies logged; per-device rate limit 60/min burst 20; quota 500/inbox with oldest-pruned; server-side TTL; body size limit; auth-failure collapses to Unauthorized (no enumeration oracle); `#![deny(clippy::await_holding_lock)]`.

**Android:** panic boundary wraps **every** FFI export (no unwinding into Kotlin); ABI 17 Rust↔Kotlin equality enforced; no clipboard/keys/tokens in logcat; key bytes `Zeroizing` in hot paths; FGS `specialUse` lifecycle correct; manifest perms appropriate; no committed release keystore.

**Boundaries:** CLI & UI do **not** link `copypaste-core` (IPC-only, confirmed in Cargo.toml + source); relay/p2p/supabase/telemetry have zero internal deps; daemon is the sole owner of crypto/Keychain/socket (except the CLI Keychain leak, P1-6); no crypto reimplemented outside core; resolver v2 prevents feature-unification leaks.

**UI:** src-tauri does not link core (26 commands are thin IPC/OS-bridge proxies); no secrets in DOM/console; password inputs masked; capabilities minimal (no fs/shell/http); reduce-motion + prefers-contrast wired; QR blur/reveal + ≤20s countdown implemented.
