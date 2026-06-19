# CopyPaste — Manual QA Checklist (post-fix)

Date: 2026-06-19 · Tracking: CopyPaste-d6th. Scenarios that cannot be fully verified by static read or host gates — require a running macOS app, an Android build (cargo-ndk + Gradle), or two paired devices. Grouped by area. `[BLOCKER]` = tied to an open reopened issue; verify after the fix lands.

## Pre-req: gates must be green first
- [ ] `cargo fmt --all --check` (PASS today)
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **[BLOCKER R1/iqkm]** currently FAIL (25 errors)
- [ ] `cargo test --workspace --all-features` — **[BLOCKER R2/xxsw]** currently 1 fail (android live cache test)
- [ ] `pnpm -C crates/copypaste-ui run test && run build` (PASS today)
- [ ] `node scripts/parity-check.mjs` (PASS 53/53)
- [ ] Android: `./gradlew assembleDebug test lint` after `./scripts/generate-android-bindings.sh` — not run in this env (cargo-ndk OOM-guard; single-build rule)

## Cross-device sync
- [ ] Copy text on macOS → appears on Android (P2P on LAN)
- [ ] Copy text on Android → appears on macOS
- [ ] Copy a non-sensitive item on macOS → appears on Android (confirm normal sync still works after the jbao filter)
- [ ] Cross-platform list ordering: bump an item via copy-back on one device → same order on both (PG-19 still open — expect drift until fixed)

## Sensitive / private
- [ ] **[BLOCKER R5/jbao]** Copy an API key (`AKIAIOSFODNN7EXAMPLE`) with relay sync on → inspect relay inbox DB → **no ciphertext row** for it
- [ ] Same for cloud: confirm no Supabase row; backlog sweep marks it `is_synced=1` without uploading
- [ ] Copy a sensitive item on Android → confirm capture behavior matches the documented contract (currently Android drops at capture; macOS stores+TTL — confirm intended)
- [ ] Private mode ON (both platforms) → copy something → nothing stored or synced
- [ ] **[R-PG14]** macOS degraded boot (revoke Keychain access, restart) with private mode previously ON → daemon starts degraded, private mode still ON, no capture
- [ ] Sensitive TTL: copy a secret, wait `sensitive_ttl_secs` → item auto-wiped (macOS sweeps every 5 s; **[PG-24]** Android only on `getItems()` — force-stop app, wait, reopen → expect item still present until fixed)
- [ ] **[R5]** Span masking: copy a long line containing one credit-card number → macOS bullet-masks the span; Android shows it unmasked (**PG-4 still open**)
- [ ] Export with no flag → sensitive items excluded (**tj9s, no test** — verify manually); `--include-sensitive` → included + stderr warning
- [ ] Import a crafted JSON with `is_sensitive:false` on a credential → stored as sensitive (PG-26 — has tests, confirm on device)
- [ ] Logs: tail daemon log + `adb logcat` during sensitive copy/sync → no clipboard content, keys, tokens, SAS, or passwords
- [ ] **[FTS orphan e5oe]** cloud-overwrite an item (same item_id, new content) on device B → search the OLD content → must NOT appear (currently expected to FAIL)

## Pairing / QR / SAS
- [ ] QR pairing macOS ↔ Android end-to-end
- [ ] **[PG-8]** Reveal QR on Android PairActivity, wait 120 s for TTL auto-refresh → stays revealed (no re-blur); exit+re-enter → starts blurred
- [ ] QR blur/reveal on macOS DevicesView (already correct) and Android
- [ ] QR expiry + regeneration preserves blur state on both
- [ ] SAS 6-digit shown on both; both sides must explicitly confirm (no auto-accept); 60 s timeout aborts
- [ ] **[PG-47]** Peer fingerprint in the Android SAS confirm modal shows full (not truncated)
- [ ] **[PG-9 DESIGN]** Own-device fingerprint on macOS card: confirm display style acceptable; confirm hidden (not blank) when P2P disabled
- [ ] **[BLOCKER R3/kmcr PG-2]** Android relay registration: after binding regen + call-site, register → relay accepts `pop_b64`; wrong device_id → rejected
- [ ] **[BLOCKER R4/8qcm PG-12]** Android revoke a peer + rotate: new key in Keystore, old key zeroed, revoked device can no longer read new cloud blobs
- [ ] **[BLOCKER R7/7d8x PG-1]** macOS sends Unpair to Android (FGS alive) → Android evicts peer, closes connection, refuses reconnect at TLS, no items after

## Devices screen / sync status
- [ ] Device metadata parity: compare every field shown on macOS vs Android device cards (name source differs — PG-38)
- [ ] **[PG-37]** Offline peer dot is RED on both (Android was grey)
- [ ] **[PG-11/PG-10]** Idle a paired peer >5 min on both → both show idle/grey (recency gate). Then: kill the macOS daemon while Android network is up → **expect divergent badges** (PG-10 offline-signal mismatch still open)
- [ ] Sync status states: connected / idle / syncing / error consistent across platforms
- [ ] **[PG-6 zfqa]** Run a daemon one protocol version ahead of the UI → confirm console.warn fires; note **no UI banner** appears (handler unassigned)

## Settings parity
- [ ] Theme System/Light/Dark on both; fresh install is light-first (clear localStorage, reload → `data-theme="light"`)
- [ ] **[PG-13]** macOS Supabase email+password: enter, Save → input clears, "set ✓" on reload, nothing in DevTools/DOM
- [ ] **[PG-30 j9xj]** macOS master sync toggle off → per-transport toggles disabled; restart daemon → **expect sync NOT actually stopped until tke7** (known-incomplete; document in release notes)
- [ ] **[PG-34]** showSensitiveWarnings toggle gates the reveal confirmation
- [ ] **[PG-32]** history display limit slider persists across app restart
- [ ] Each setting: change on both platforms, restart, confirm persistence + single source of truth
- [ ] **[PG-29]** lan_visibility toggle present on Android (still open)
- [ ] **[v6wh/nq39]** After `cloud setup`, `cat ~/.local/share/copypaste/config.json` → no `supabase_password` field on macOS; Keychain shows `com.copypaste.daemon/supabase-password`

## UI / Tauri
- [ ] **[wb2c]** Tauri app loads with the new CSP → DevTools console shows zero CSP violations (history, QR via blob:, settings)
- [ ] **[2hxr]** Start daemon slowly → Popup/Devices/Settings each show "Starting up…" not a generic error
- [ ] **[54h5]** Daemon offline → ErrorBoundary shows only "Something went wrong"; spawn-error banner generic (no file paths)
- [ ] Daemon unavailable / ipc_not_ready handled gracefully in every view
- [ ] relay unavailable / cloud disabled / cloud misconfigured → correct status, no crash, no secret in error text

## Theme / visual
- [ ] **[PG-36]** macOS accent renders `#4D8DFF` (inspect computed `--ide-accent-rgb: 77 141 255`), not `#3D8BFF`, including first-paint
- [ ] **[vo79]** Android selection/multi-select uses `#4D8DFF`

## Regression spot-checks (must still work)
- [ ] macOS clipboard polling / copy / delete / search / watch unchanged
- [ ] **[oti6]** Force `KeyLoad::Locked` (revoke Keychain) → daemon enters degraded mode (banner), does NOT crash; `tracing::error!` logged
- [ ] **[26pd]** Under heavy macOS load with an app-exclusion set → clipboard ticks not blocked by `lsappinfo`
- [ ] **[lszh]** Break `lsappinfo`, copy from an excluded app (1Password) → capture skipped (fail-closed)
- [ ] **[ptb8]** Version mismatch → CLI shows the upgrade prompt
