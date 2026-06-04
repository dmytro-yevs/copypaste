# CopyPaste — Master Backlog (consolidated audit synthesis)

**Date:** 2026-06-04 · **Branch:** `v0.6.1-integration` @ `ff09eef` (compile-green, not built/installed)
**Author:** orchestrator synthesis of 17 background agents (read-only audit, no code changed).

## How this was produced
13 read-only audit agents + 4 user personas, all writing to `docs/`:
- **Inventory:** `product/features-{macos,android,sync,core-security}.md`
- **Competitive:** `product/competitive-gap-analysis.md`
- **Code review:** `review/review-{macos,android,daemon,core-sync}.md`
- **Performance:** `perf/perf-{macos,android,backend}.md`
- **UX/UI:** `ux/ux-ui-review.md`
- **Personas:** `product/persona-{power-developer,privacy,casual,creative}.md`

**Legend** — Severity: **P0** data-loss/security/crash · **P1** broken feature/major friction · **P2** quality · **P3** nice-to-have. Effort: **S** <½ day · **M** ~1-3 days · **L** >3 days / architectural. "✦ cross-confirmed" = found independently by ≥2 agents (higher confidence).

---

## 1 · P0 — Critical (correctness / data-loss / security / crash)

| ID | Issue | Where | Effort | Source |
|----|-------|-------|--------|--------|
| C1 ✦ | **`delete_all` (clear history) hard-deletes with no tombstone** → cleared items resurrect on next sync; deletion never propagates to peers | `daemon/ipc.rs:~3005` | M | daemon-review, features-sync |
| C2 ✦ | **>2-device sync is broken** — `shared_sync_key()` returns the *first* paired peer's key, so 2nd/3rd devices encrypt under the wrong key and can't decrypt; no warning | `daemon/sync_orch.rs`, daemon | L | daemon-review, features-sync |
| C3 ✦ | **macOS file add throws `RangeError`** — `btoa(String.fromCharCode(...Array.from(bytes)))` overflows the call stack for files >~65 KB → drag-drop/file-pick silently fails despite 100 MB cap | `ui/src/lib/ipc.ts:~525` | S | review-macos, perf-macos |
| C4 | **Android WorkManager poll garbles files** — `storeDecryptedItem` is dead (0 call sites); `SupabasePollWorker.doWork` has no file branch → file bytes UTF-8-decoded as garbage on the 15-min fallback path | `android/SupabasePollWorker.kt` | M | review-android (confirms our deferred note) |
| C5 | **ANR risk** — `runBlocking { repository.localItemsForSync(key) }` on the FGS main thread blocks tens–hundreds of ms on large history | `android/ClipboardService.kt:~345` | S | review-android |
| C6 | **Sensitive auto-wipe uses wrong predicate** — daemon calls `detect().is_some()` not `is_sensitive_for_autowipe()` (`# FIXWAVE`) → low-confidence items (email/phone/passport) get an unintended 30 s TTL and are silently deleted | core sensitive path | S | features-core-security, persona-privacy |
| C7 ✦ | **Image/file sensitive detection absent** — screenshots of recovery phrases / TOTP QR / private-key PDFs are never flagged, never TTL'd, sync freely to all devices | core detection | L | persona-privacy, features-core-security |
| C8 | **VERIFY: `wall_time` clamp** — if `MAX_WALL_TIME_SKEW_MS` (≈Sept-2001 in ms) is used as an *absolute* ceiling rather than `now()+skew`, every 2026 inbound item is clamped → LWW + display corruption. (Name says "skew" → may be relative; confirm before fixing.) | `sync/engine.rs:52` | S (verify) | review-core-sync |

## 2 · P1 — High impact (broken/half-wired features & major friction)

| ID | Issue | Where | Effort | Source |
|----|-------|-------|--------|--------|
| H1 ✦ | **FTS search not wired to UI** — FTS5 table + daemon infra exist, but the UI does client-side substring over the loaded page only → items past the page are silently unsearchable (reads as a bug) | macOS + Android search | M | persona-power-dev, features-macos |
| H2 | **`triggerCatchUpPoll` is a no-op** — fetches a batch on WS reconnect then logs+discards it → catch-up heals nothing | `android/SupabaseRealtimeClient.kt:~603` | S | review-android |
| H3 | **Android FFI AAD mismatch** — `encrypt_text`/`decrypt_text` use `"{id}|3"` but daemon expects `"{id}|4|2"` → `AuthFailed` for anything that path touches | `copypaste-android/src/lib.rs` | M | review-core-sync |
| H4 ✦ | **QR doesn't provision sync** — QR carries only P2P pairing material; relay/Supabase creds never transfer, so sync silently dies off-LAN (cellular) | `core/crypto/pairing_qr.rs` + daemon | M | persona-power-dev, persona-casual, project memory |
| H5 | **Android Settings manual Save loses data** — Mac auto-saves; Android requires a Save tap and silently drops unsaved edits on nav → switch to auto-save | `android/SettingsActivity.kt` | M | ux-ui, persona-casual |
| H6 ✦ | **Off-LAN sync is invisible & unconfigurable for non-devs** — P2P works on LAN, dies elsewhere; the only fix (Supabase) needs manual URL/key entry. Needs a hosted "sign in to sync" or QR-provisioned relay | sync + onboarding | L | persona-casual, persona-power-dev |
| H7 | **`incomingPairing` prop vs `responderPairing` state** — two sources of truth; closing clears state but not the prop → remount can re-open; initiator modal `initialStatus={incomingPairing}` can misread a trailing `idle` as success | `ui/DevicesView.tsx`, `App.tsx` | M | review-macos |
| H8 ✦ | **`peers.json` re-read on every sync item** (file I/O per copy) | `daemon/sync_orch.rs:~119` | S | review-daemon, perf-backend |
| H9 | **`live_peer_sinks` ≡ `p2p_live_sinks`** — two `Arc` clones of the same map masquerading as distinct fields | `daemon/ipc.rs` | S | review-daemon |
| H10 | **Relayed PAKE drops SessionKey w/o TLS channel binding** (`TODO(S3)`) — MitM bridging risk | daemon pairing arms | M | review-daemon |
| H11 | **Supabase rows carry no `key_version`** — cloud transport can't survive a key rotation | `copypaste-supabase` | M | review-core-sync |
| H12 | Notification-ID 1001 collision (FGS vs sensitive-detected) | `android/ClipboardService.kt` | S | review-android |
| H13 | Preview copy-back grants URI only to SystemUI (overlay missed `grantUriToAll`) | `android/HistoryActivity.kt:~1127` | S | review-android |
| H14 | "Test connection" silently saves config + restarts daemon (expected: read-only probe) | `ui/SettingsView` | S | review-macos |
| H15 | 3 daemon config fields missing from `AppSettings` TS type (forces `as unknown` casts) | `ui/src/lib/ipc.ts` | S | review-macos |
| H16 | Android Supabase-polled file items lose `file_name`; outbound delete/pin/reorder don't sync from Android; relay upload path disabled | android sync | M | features-android |

## 3 · Missing features (persona demand × competitive gap)

Ranked by how often it surfaced across personas + competitors.

| ID | Feature | Demand | Effort | Source |
|----|---------|--------|--------|--------|
| F1 | **Paste-as-plain-text per-paste** (`Option/Alt+Enter`), not the global toggle | power-dev (table-stakes), creative, casual | S–M | personas, competitive |
| F2 | **Snippets / templates with keyword expansion** | power-dev (dealbreaker), creative | L | personas, competitive (Raycast/Alfred) |
| F3 | **Rich-text / HTML preservation** (daemon currently discards `public.rtf`/`public.html`) | creative (#1 blocker) | M–L | persona-creative, competitive |
| F4 | **iOS / iPadOS client** | casual (biggest adoption blocker), competitive | L | persona-casual, competitive |
| F5 | **Per-app capture exclusion / blocklist** (vs all-or-nothing private mode) | privacy, competitive | M | persona-privacy, competitive |
| F6 | **Collections / folders / tags / pinboards** (flat list doesn't scale past ~12 pins) | creative | L | persona-creative, competitive |
| F7 | **OCR / image-text search** | creative, competitive (Raycast/Paste) | L | persona-creative, competitive |
| F8 | **Propagated panic-wipe** (clear-all that actually syncs the wipe — overlaps C1) | privacy (P0) | M | persona-privacy |
| F9 | **Manual "mark sensitive" / redact** + per-item local-only flag | privacy | M | persona-privacy |
| F10 | **Content-type quick-filter** ("images only / URLs only") | creative | S | persona-creative, ux |
| F11 | **Undo toast on single delete** (tombstone infra already exists) | power-dev (quick win) | S | persona-power-dev |
| F12 | **Larger/sharper thumbnails** (192px → ~400px long edge) + hover quick-look | creative | M | persona-creative, perf-macos |
| F13 | **Paste-time text transforms** (base64/url/json/case) | power-dev, competitive | M | competitive (PastePal) |
| F14 | **AI / MCP integration** (local MCP server, AI commands) | competitive (Paste/Raycast 2025-26) | L | competitive |
| F15 | **macOS "Launch at login" UI** (commands wired, no toggle) + first-run onboarding | casual | S | features-macos, ux |
| F16 | **Drag-out images/files** to other apps; bulk history export | creative | M | persona-creative, features-macos |

## 4 · Performance

**macOS** (`perf-macos.md`): P-IPC1 history poll 1.2 s→3 s (halves ~98 calls/min); P-IPC2 cache `notify_on_copy` in `AtomicBool` (tray reads `get_config` every 5 s); P-MEM1 cache full-res image in Details modal (200-400 MB spikes); P-R1 memoize `HistoryRow` + `VirtualList` offsets; stop a11y-permission poll once granted.
**Android** (`perf-android.md`): P-A1 **migrate the SharedPreferences blob store → SQLite/SQLCipher** (root bottleneck; also fixes the fragile pipe-delimited blob) [L]; P-A2 stop `pruneByAge()` on every `getItems()`; P-A3 cap `parseCache` with `LruCache(200)` [S]; P-A4 adaptive backoff for the 3 s P2P dial + 1 Hz responder poll; P-A5 cache `grantUriToAll` package list (enumerates all packages per copy).
**Backend** (`perf-backend.md`): P-B1 connect the existing `r2d2` pool / drop the single global DB mutex; P-B2 `prepare_cached` for the 3 hot page queries; P-B3 single batched FTS `DELETE … WHERE id IN (…)`; P-B4 kill double-base64 on relay; P-B5 broadcast item-id not full blob (H8 peers.json cache also here).

## 5 · UX / UI (`ux-ui-review.md`)

Top: U1 **Android auto-save** (= H5); U2 remove mandatory **QR blur-reveal click**; U3 **popup `role="listbox/option"`** (screen readers can't use history at all); U4 SAS "Match/Doesn't match" needs explanatory copy for a security action; U5 **44px hit targets** (`IconActionBtn` is 20×20); U6 cross-platform parity: settings-tab count (6 vs 5), "MB" vs "MiB", "This Mac" hardcoded, Material vs Lucide icons, no LAN discovery section on Android, KindChip 9px < 10.5px floor; U7 remote device shows raw UUID not name; U8 destructive actions (clear-all / revoke-all) need stronger confirmation; U9 macOS onboarding/first-run; U10 dev-speak microcopy ("re-provision remaining devices", raw MIME/bundle IDs).

## 6 · Code health / architecture

- **God modules:** `daemon/ipc.rs` 12 056 lines / one 4 820-line match arm; `ui/HistoryView.tsx` 2348; `ui/DevicesView.tsx` 1593 (`SasPairingModal` ~370 embedded); `android/HistoryActivity.kt` 2472; `ClipboardRepository.kt` 1812; `ClipboardService` companion ~870. → split into modules; move business logic out of Composables/Views.
- **Dead/dup:** `storeDecryptedItem` dead (C4); duplicated 3-path sync dispatch (FgsSyncLoop/PollWorker/RealtimeClient); inline `contentType=="image"` vs `contentTypeIsImage()`; HKDF salt-constant naming hazard (`HKDF_SALT_V2` vs `_V2_BASE`); `device_id` vs `origin_device_id` naming drift.
- **Group-key architecture:** N>2 devices need a shared group key, not per-pair keys (root of C2).
- **Android storage architecture:** SharedPreferences-as-DB (P-A1) is the root of both perf and the fragile blob format.

## 7 · Do-first — Top 15 (impact ÷ effort)

1. **C3** macOS file `btoa` RangeError — S, file feature broken today
2. **C6** sensitive auto-wipe predicate — S, silent data loss
3. **C5** Android `runBlocking` ANR — S
4. **C1** `delete_all` tombstone+broadcast — M, "deletes resurrect"
5. **H2** `triggerCatchUpPoll` actually store rows — S
6. **H8/H9** peers.json cache + collapse duplicate sink fields — S
7. **C8** verify/fix `wall_time` clamp — S
8. **H1** wire FTS search to the UI — M
9. **F1** per-paste plain-text (`Option+Enter`) — S–M
10. **F11** undo-delete toast — S
11. **U2/U3** remove QR blur-click + popup a11y roles — S
12. **H5/U1** Android Settings auto-save — M
13. **C4** Android WorkManager file branch (revive/replace `storeDecryptedItem`) — M
14. **H4** QR full-provisioning (relay/Supabase creds) — M
15. **C2** group-key for >2 devices — L (highest strategic value)

> Strategic bets (separate track, all **L**): F4 iOS app, F2 snippets, F3 rich-text, P-A1 Android→SQLite, F6 collections, F7 OCR, H6 hosted sync sign-in.

## 8 · Source map
Every item above cites its origin doc under `docs/product/`, `docs/review/`, `docs/perf/`, `docs/ux/`. Persona wishlists in full live in `docs/product/persona-*.md`. Competitive matrix in `docs/product/competitive-gap-analysis.md`.
