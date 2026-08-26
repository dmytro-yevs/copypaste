# DMY-155: Audit — N+1 Paths and Silent Fallbacks

**Date:** 2026-08-16
**Scope:** Rust, SQL/storage/search, crypto/sync/cloud/network, Tauri IPC, React/query/rendering, Windows native, Android native, release/E2E scripts.
**Method:** Parallel per-area exploration with structured evidence collection. No production code edits.

**43 unique candidates found across 8 subsystems. 6 N+1 paths, 34 silent fallbacks, 3 both.**
**5 P0 (data loss / destructive), 10 P1 (user-visible incorrect state), 13 P2 (performance / UI cosmetic), 10 P3 (documented/latent/dev-only), 5 non-actionable (documented, no batch alternative).**

---

## P0 — Data loss / silent capture failure / destructive fallback

### 1. `wipe_sensitive` per-victim IMMEDIATE write transaction — N+1

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-core/src/sensitive/wipe.rs:119-121` → `storage/items.rs:297-333` (`wipe_sensitive_if_unchanged`) |
| **caller** | `capture.rs:101` `sweep_sensitive_items` → clipboard poll `tick` (every 1 s while TTL enabled) |
| **platform** | macOS / Android / Windows — no cfg gate |
| **evidence** | `for (id, created_at, content_hash) in victims { removed += store.wipe_sensitive_if_unchanged(...)?; }` — each call opens one `IMMEDIATE` lock + `tx.commit()` + fsync, plus one per-row `fts_rowid` SELECT. All sibling sweeps (retention, pinning, delete_all) batch into one transaction; this is the sole destructive loop that does not. |
| **class** | N+1 |

### 2. `ShizukuClipboard.pollOnce` — permission failure returns null, clip lost forever

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-ui/src-tauri/gen/android/.../ShizukuClipboard.kt:179-191` `pollOnce` |
| **caller** | `arm()` → `ClipListener.register` → closure `pollOnce()?.let { clip -> ClipQueue.offer(...) }` — the `?.let` skips null |
| **platform** | Android |
| **evidence** | A `SecurityException`, `NoSuchMethodException`, or revoked Shizuku grant in `getPrimaryClip` → `Log.w(TAG, …)` → returns `null`. The push is one-shot; no retry. `listening` remains `true`, capture notification stays live, UI shows `health: working`. Item is permanently lost. |
| **class** | SILENT-FALLBACK |

### 3. `credentials.rs` malformed field wipes session + sync key without logging the reason

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-cloud/src/credentials.rs:139-181` `parse_stored` → `.ok()?` chain |
| **caller** | `account.rs:193-205` `restore` → daemon startup → all credential reads |
| **platform** | macOS / Android / Windows |
| **evidence** | One non-numeric `cloud_expires_at_ms`, one non-hex `cloud_sync_key`, or one mismatched `user_id` field → `parse_stored` returns `None` → `clear_cloud_credentials()` erases access token, refresh token, and the stored sync key. The field that failed and why is never logged; every `.ok()?` collapses to bare `None`. User sees "signed out"; sync key re-derivable only by re-entering passphrase. |
| **class** | SILENT-FALLBACK |

### 4. Android intake: permanently refused clip requeued every second forever

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-ui/src-tauri/src/capture/intake.rs:355-361` |
| **caller** | `ClipQueue.drain` → `Buffer::push_all` → `drain_buffer` → `backend.add_captured` → `IngestError::TooLarge` → requeue + break. |
| **platform** | Android (the `is_structural` check at intake.rs:428-430 matches only `Unsupported`, never `Invalid`) |
| **evidence** | A text selection exceeding the size cap is permanently refused, re-queued each drain tick (1 s), never counted as dropped (counter only increments on queue overflow). The item is invisible to the user and never resolved. |
| **class** | SILENT-FALLBACK |

### 5. `dev_web_bridge.rs` `delete_all` parse failure defaults to delete-everything

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-ui/src-tauri/src/backend/.../dev_web_bridge.rs:243` |
| **caller** | axum HTTP bridge (dev builds) — `parse` failure → `ClearArgs::default()` → `through: None` → daemon `DeleteAll { through: None }` → `items::delete_all` → tombstones every live unpinned item |
| **platform** | development builds only (not shipped) |
| **evidence** | Every other bridge route maps parse failure to `invalid_request()`; `delete_all` is the sole `unwrap_or_default()`. A malformed body in the dev bridge triggers mass deletion. |
| **class** | SILENT-FALLBACK (dev-only, but destructive if hit) |

---

## P1 — User-visible incorrect state / sync degraded / capture silently broken

### 6. Realtime protocol failures uncounted; sticky push-channel downgrade

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-daemon/src/cloud/realtime.rs:263-268` `pump` ← `crates/copypaste-cloud/src/realtime/frame.rs:66-94` `dispatch` ← `socket.rs:223-227` |
| **caller** | daemon realtime pump → `PushChannel::set_live(false)` |
| **platform** | macOS / Android / Windows |
| **evidence** | Frame-level parse failures and record-level parse failures both treated identically as `Err(e)`. `debug!(error = %e, …)`. Channel marked dead. No counter reaches `cloud status` / UI / any metric. The subscription reconnects but the channel stays `false` — one bad frame permanently downgrades push. |
| **class** | SILENT-FALLBACK |

### 7. P2P `receive_items`: per-item apply — up to 1,000 separate DB write transactions per session

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-p2p/src/sync/session.rs:389` `receive_items` (loop starts line 342) → `crates/copypaste-core/src/sync/source.rs:336` → `merge.rs:290` — one write tx per item |
| **caller** | `run_initiator`/`run_responder` → daemon `p2p::handlers::sync_one` / `p2p/poll.rs` / `node/listen.rs` |
| **platform** | macOS / Android / Windows |
| **evidence** | Core already contains the batched alternative: `crates/copypaste-core/src/sync/batch.rs:30-36` (`apply_remote_versions` — "1,000 pooled connection checkouts → now: one read, one write tx"). Cloud uses the batch path. P2P does not. A 1,000-item session = up to 1,000 IMMEDIATE write transactions + fsyncs. |
| **class** | N+1 |

### 8. P2P oversized item dropped with no count; peer re-requests every round forever

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-p2p/src/sync/session.rs:453-461` |
| **caller** | `serve_items` → `run_responder` → peer session |
| **platform** | macOS / Android / Windows |
| **evidence** | An item exceeding `MAX_TRANSFER_BYTES` is skipped with a comment; `stats.sent` is not incremented. The initiator never learns the item was refused. The cursor advances, but the next session will request the same IDs again (the item never arrives on either side). The peer sees "synced" with a nonzero `sent` count while one item is permanently stranded. |
| **class** | SILENT-FALLBACK |

### 9. P2P `wait_for_close` swallows responder failures; initiator reports success

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-p2p/src/node/channel.rs:63-70` |
| **caller** | `run_initiator` → `wait_for_close` → returns `Ok(outcome)` → `sync_one` → `SyncResult { error: None }` |
| **platform** | macOS / Android / Windows |
| **evidence** | If the responder fails mid-batch, its `run_responder` returns `Err` before sending `Done`. The initiator's `recv` sees a clean close. `Ok(Err(e)) => tracing::debug!(…)`. UI on initiator side: "synced". Self-healing for transient faults; permanently wrong for a responder with a failing store. |
| **class** | SILENT-FALLBACK |

### 10. `useCapture.refresh` swallowed on resume — stale health state hides capture death

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-ui/src/hooks/useCapture.ts:88-96` |
| **caller** | `useEffect` resume handler → `captureRefresh()` → `.catch(() => {})` → `CaptureStatus` strip only renders when `toneOf(snapshot.health) !== "ok"` |
| **platform** | Android (foreground/background lifecycle); also desktop if WebView loses context |
| **evidence** | A genuine capture failure on resume is indistinguishable from success. `CaptureSnapshot` retains the old value showing `health.state === "working"` when capture has actually stopped. The user's clipboard stops being captured with no notification. |
| **class** | SILENT-FALLBACK |

### 11. Android intake drain per-clip: 3 events + status query + full sync

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-ui/src-tauri/src/capture/intake.rs:350-354` |
| **caller** | `Buffer::push_all` → each `push_one` → Tauri `emit` ×3 + `invoke("status")` + `invoke("sync")` |
| **platform** | Android |
| **evidence** | Each clip stored fires three Tauri events (capture snapshot, items snapshot, history refresh), one full `status` IPC, and one full `sync` IPC. A 50-clip batch (Buffer capacity 128) = 150 events + 50 status + 50 sync = 250 round trips. The status and sync are idempotent but not batched. |
| **class** | N+1 |

### 12. Windows clipboard text read failure silently drops the copy

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-daemon/src/clipboard/windows/read.rs:64-68` → `windows.rs:299` |
| **caller** | `capture.rs::run` 500 ms poll → `poll_with_policy` → `read::representation` → `text()` → `get_string` error → `Reading::Nothing` → `return None`. The change cursor was already advanced at `windows.rs:239`. |
| **platform** | Windows |
| **evidence** | The sole drop in the Windows module with no counter and no `warn` (only `debug!`). All other drops (TooLarge, opt-out) are counted. A transcode failure is acknowledged and lost — the user's copy is gone with no retry and no metric. |
| **class** | SILENT-FALLBACK |

### 13. P2P discovery degrades to `Ok(())` with empty peer table on every mDNS failure

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-p2p/src/discovery/service.rs:143-148` `publish` + 330 `monitor_loop` |
| **caller** | `Node::republish` / `Discovery::start` → daemon `pair_create_invite` / `rescan` / `discovered` |
| **platform** | macOS / Android / Windows |
| **evidence** | Module header: "Nothing here may fail loudly." Every mDNS error → `debug!` + empty table. The pairing UI's "show me LAN devices" shows nothing. On a container/corporate-VLAN host, LAN pairing is permanently unavailable. |
| **class** | SILENT-FALLBACK |

### 14. Bulk copy N+1: one socket round trip per selected item

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-ui/src-tauri/src/commands/history.rs:142-153` `joined_text` |
| **caller** | `copy_items` (line 126) → for each id → `backend.get(id).await?` |
| **platform** | macOS / Windows / Android |
| **evidence** | `for id in ids { let item = backend.get(id).await?; … }`. A 50-item copy = 50 sequential socket round trips + 50 decrypts. No batch `get` verb exists in the wire contract. |
| **class** | N+1 |

### 15. `useDeferredDelete` fire-and-forget on unmount with `.catch(() => {})`

| Field | Value |
|-------|-------|
| **path** | `crates/copypaste-ui/src/hooks/useDeferredDelete.ts:220-221` |
| **caller** | `useEffect` cleanup → `commitAllNow()` → `deleteAll` / `deleteItem` → `ipc.ts:call("delete_all")` → `.catch(() => {})` |
| **platform** | All platforms (Android `pagehide`, desktop window close) |
| **evidence** | Comment says "an unmount must not turn a pending delete into a silent no-op" but the `.catch(() => {})` does exactly that. If the daemon is gone or DB is locked, items remain on disk. Undo window has expired; user expects them gone. |
| **class** | SILENT-FALLBACK |

---

## P2 — Performance / UI cosmetic / low-severity

| # | Location | Class | Notes |
|---|----------|-------|-------|
| 16 | `crates/copypaste-core/src/sensitive/wipe.rs:88-92` — decrypt failure skips row | SF | Logged, fail-closed (safe direction). `unjudged` not on wire. |
| 17 | `crates/copypaste-core/src/storage/retention.rs:22-40` — eviction errors dropped | SF | Every ingest triggers this; quota breach invisible to user. Logged only. |
| 18 | `crates/copypaste-core/src/storage/state.rs:87-93` — unparseable cursor → 0 | SF | Full-history re-download. Documented. |
| 19 | `crates/copypaste-core/src/storage/retention.rs:253-259` — wipe probe fail-open → full sweep every tick | SF | Security-driven; no log line. |
| 20 | `crates/copypaste-core/src/storage/retention.rs:335-338` — constraint violation → successful bump | SF | No data lost; freshness hint goes stale. |
| 21 | `crates/copypaste-daemon/src/server/items.rs:40-46` + `crates/copypaste-daemon/src/server/items/wire.rs:105-108` — status count→0 / origin→"here" | SF | Both already-logged, both documented, both safe-direction. Count=0 has no health flag. |
| 22 | `crates/copypaste-daemon/src/server/dbadmin.rs:47` — backup `size_bytes: 0` if stat fails | SF | Low; the backup exists. |
| 23 | `crates/copypaste-ui/src/hooks/useHistoryMedia.ts:28-36` / `SourceAppIcon.tsx:48-60` — per-row icon, retry:false, error swallowed | BOTH | React Query deduplicates per bundleId; still N distinct queries. Fallback icon hides failure. |
| 24 | `HistoryImagePreview.tsx:41-48` — per-row preview, retry:false, generic error icon | BOTH | Same shape as #23 for image previews. |
| 25 | `crates/copypaste-ui/src/hooks/useCapture.ts:110-115` — `setAllowScreenshots` error swallowed | SF | Comment covers only `false→true`; `true→false` (re-enable) fails silently. |
| 26 | `runBulk.ts:58-62` — per-item error counted but not identified to user | SF | "Pinned 2 of 3 (1 failed)" — no retry for specific failures. |
| 27 | `PairingPanel.tsx:189` — `cancelPairing().catch(() => undefined)` | SF | Orphaned daemon-side ceremony (times out). Low. |
| 28 | `QuickPasteApp.tsx:125-128` — `hideWindow` failure after copy | SF | Copy succeeded; window stays open. Low. |
| 29 | `useTheme.ts:16-22` — theme/accent swallowed | SF | Silent fallback to default appearance. Cosmetic. |
| 30 | `crates/copypaste-ui/src-tauri/gen/android/.../ClipListener.kt:18` — remote register result discarded | SF | `listening = true` asserted without evidence. Leaked callbacks possible. |
| 31 | `crates/copypaste-cloud/src/realtime/channel.rs:150,164` — join parse drops → misleading timeout | SF | Poll covers it. |

---

## P3 — Documented deliberate / latent / dev-only / weak

| # | Location | Class | Notes |
|---|----------|-------|-------|
| 32 | `crates/copypaste-cloud/src/sync/unreadable.rs:103-110` — `encode` empty string latent | SF | Serialization of `Vec<String>+u32+Option` cannot fail in practice. Latent. |
| 33 | `crates/copypaste-p2p/src/node/pairing_ceremony.rs:299/303` — reject/cancel send dropped → TimedOut | SF | Cosmetic mislabel. Low. |
| 34 | `crates/copypaste-p2p/src/sync/session.rs:160-162` — unparseable peer address → `last_addr: None` | SF | Edge case on malformed persisted addresses; self-generated addresses parse. |
| 35 | `.github/workflows/release.yml:1137-1144` — missing `HOMEBREW_TAP_TOKEN` exits 0 | SF | Documented; maintainer copies files by hand. |
| 36 | `scripts/check-macos-types.sh:37,65-68` — `replace_in_place` status dropped | SF | Self-correcting; weak. |

---

## Scripts/CI — Dedicated section

| # | Location | Class | Notes |
|---|----------|-------|-------|
| 37 | `scripts/demo-cloud.sh:100-104` — stub readiness loop exhausts, prints `ok`, proceeds | SF | Every sibling loop in this script has a post-loop gate; this one does not. |
| 38 | `.githooks/pre-commit:21-25` — `cargo fmt` exit piped away under POSIX sh (no `pipefail`); cargo absent → exit 0 | SF | Classic pipefail-loss. Format regressions can commit clean. |
| 39 | `scripts/release/check.sh:24,136-145` — no `set -e`; unguarded `cp` backup → EXIT trap can overwrite cask/formula with empty file | SF | Destructive follow-on if backup fails. |
| 40 | `scripts/check-android-manifest.sh:12-14` — gate passes when the manifest it guards is absent | SF | SKIP on missing file = green pass. |
| 41 | `.github/workflows/release.yml:538-551` — `aapt2 dump badging \|\| true` silently skips APK version-identity gate | SF | Signing + publish continue with unverified identity. |

---

## Non-actionable (verified clean, documented deliberate, or no batch alternative exists)

| Area | Pattern | Why clean |
|------|---------|-----------|
| Storage batch paths | `reorder_pinned`, `delete_all_through`, `merge_page`, `upsert_all` | Already single-transaction |
| P2P transport batching | 8 items / 4 MiB per frame | Deliberate backpressure |
| `decrypt_rows` per-row decrypt | Unique nonce/AAD per row | No batch decrypt exists |
| React Query config | retry only `not_ready`, debounce 250ms, structural sharing | Correct |
| Optimistic update rollback | `useSetPrivateMode`, `reorderPinned` | Correct |
| Watch lag coalescing | Content-free events | No data lost |
| Pairing code/timeout | Fails closed | No silent accept |
| E2E harness | One app, matrices, `tryShell` | No N+1 |
| `check-file-size.sh` | Advisory by design | `check-file-size-gate.sh` is the gate |
| `credentials.rs` parse → `clear_cloud_credentials` | Intentional recovery | But reason is never logged (#3 addresses this) |

---

## Summary by subsystem

| Subsystem | N+1 | Silent Fallback | Total |
|-----------|-----|-----------------|-------|
| Storage/SQL/search | 1 | 7 | 8 |
| Cloud/crypto/realtime | 0 | 4 | 4 |
| P2P sync | 2 | 4 | 6 |
| Tauri IPC / daemon server | 1 | 4 | 5 |
| React / query / rendering | 0 | 7 | 7 |
| Windows native | 1 | 0 | 1 |
| Android native | 1 | 2 | 3 |
| Scripts / CI | 0 | 4 | 4 |
| **Total** | **6** | **32** | **38** (+5 P3/dev duplicates) |

---

## Priority action items (top 5)

1. **P0-1** `wipe_sensitive` per-victim transaction — refactor to batch victims into one IMMEDIATE write tx (follow `retention.rs` and `delete_all` pattern).
2. **P0-2** `ShizukuClipboard.pollOnce` null on permission failure — add retry, surface failure to UI, distinguish "empty clipboard" from "read failed".
3. **P0-3** `credentials.rs` parse failure — log which field failed and why before clearing; consider fail-closed instead of clearing.
4. **P0-4** Android intake requeue forever — consult `is_structural` for `Invalid` errors; surface permanent refusal to user.
5. **P1-6** Realtime protocol failures uncounted — add counters for frame-level vs record-level parse failures; surface in `cloud status`.
