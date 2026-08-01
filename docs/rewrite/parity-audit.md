# Parity audit — what v1 did that v2 does not

**Question asked:** *were all the capabilities carried over from v1?*

**Short answer:** no, and the shortfall splits into three unequal piles. Most of
the reduction is the point of the rewrite and is recorded as a decision. A second
pile is declared missing in `README.md` / `SECURITY.md` and is therefore known,
if in places understated. A third pile — **19 capabilities** — is neither
recorded nor declared. Those are the finding.

---

## 0. Scope, method, and honesty about what I could not check

### What I compared

| | v1 (`archive/v0.4.1-pre-rewrite`, `d36c5676`) | v2 (`v2-main`, `c53be35b`) |
|---|---|---|
| Rust files / crates | 528 files, 12 crates | 114 files, 7 crates |
| Rust lines | ~150k (README figure; not recounted) | 26,147 |
| Kotlin files | 304 (a full Android app) | 0 |
| TS/TSX files | 157 | 17 |
| Rust tests | not counted (see below) | 523 |
| IPC operations | 61 addressable | 13 |
| CLI subcommands (top level) | 25 | 13 |
| UI screens | 5 + popup + tray | 1 + tray |

### Sources read

* `docs/rewrite/port-manifest/` — all seven manifests (9,582 lines) and the
  `README.md` that scopes them.
* `CLAUDE.md`, `README.md`, `SECURITY.md`, `docs/adr/0001`, `docs/adr/0002`,
  `docs/rewrite/target-architecture.md`.
* v1 via `git show` / `git ls-tree` only — the branch was never checked out.
  `README.md`, `ARCHITECTURE.md`, `docs/adr/ADR-014`, the crate and module
  trees, `AppConfig`, the CLI command tree, the UniFFI surface.
* v2 source in full for `copypaste-core`, `copypaste-ipc`, `copypaste-daemon`,
  `copypaste-p2p` (merge/transport/peers), the Tauri bridge and the React app;
  by targeted read for `copypaste-cloud` and `copypaste-cli`.

### Anchor commit, and why it matters

**This audit describes `HEAD` = `c53be35b`.** Six agents are editing the tree
concurrently, and the working tree already diverges from `HEAD` in ways that
close some of the gaps below. Untracked-but-landing at the time of writing:

`crates/copypaste-daemon/src/cloud/`, `crates/copypaste-daemon/src/merge.rs`,
`crates/copypaste-ui/src-tauri/src/shell/` (tray, hotkey, autostart),
`crates/copypaste-ui/src-tauri/src/commands/peers.rs`,
`crates/copypaste-ui/src-tauri/src/backend/embedded.rs` (the Android in-process
backend), `tauri.android.conf.json`, `tauri.macos.conf.json`, `supabase/`
(four SQL migrations including a `pg_cron` retention job), `.github/`,
`scripts/release/`.

Where a gap below is being closed right now I mark it **[in flight]**. A gap
marked in flight is still a gap at `HEAD`; it is marked so nobody re-does work
that is already underway.

### What I could not check, and why

1. **I did not run anything.** No `cargo test`, no `npm test`. Every test
   verdict below is "a test with this assertion exists in the tree", not "it
   passes". The counts are from static extraction of `#[test]` / `#[tokio::test]`
   attributes.
2. **I did not count v1's tests.** The README's "~500 acceptance tests" is a
   manifest figure, and the manifests are what I audited against — not v1's
   actual test files. Comparing 508 manifest acceptance tests to 523 v2 Rust
   tests would be meaningless and I have not done it.
3. **Anything macOS-only is unverifiable here and I did not try.** NSPasteboard,
   Keychain, the window server, TCC. Where v2's code looks right I say the code
   looks right; I never say it works.
4. **Android.** v1 shipped a 304-file Kotlin app. v2 has no Android app at
   `HEAD`. I audited the *capability*, not the port quality of something that
   does not exist.
5. **`copypaste-cloud` I read selectively** — the sync driver, pull/push, the
   REST client and the auth module. I did not read `realtime/` line by line.
6. **The v1 → v2 line-count ratio is not evidence of anything** and I have not
   used it as such.

### Verdict vocabulary

| Verdict | Means |
|---|---|
| **Ported & tested** | The behaviour exists in v2 *and* a test asserts it. |
| **Ported, untested** | The behaviour exists in v2; no test would catch its removal. |
| **Solved differently** | v2 does not do what v1 did, but achieves the same user-visible outcome by another mechanism. Named explicitly so it is not miscounted as loss. |
| **Deliberately dropped** | Absent, and the absence is recorded somewhere I can cite. |
| **⚠ Silently missing** | Absent, and I found no record of the decision in `CLAUDE.md`, the ADRs, the manifests, `README.md`, `SECURITY.md`, or a source comment. |

---

## 1. Headline findings

### 1.1 The nineteen silently-missing capabilities, ranked by what a user loses

Ranked by user impact, not by implementation cost. "Where I looked" is given for
each, because a false alarm here costs real time.

| # | Capability | What the user loses | Where I looked |
|---|---|---|---|
| **1** | **Any UI for pairing, peers, or sync** | **Closed** — `crates/copypaste-ui/src/components/devices/`. Peer sync exists, is tested, and has a CLI — and there is no way to reach it from the app. This is precisely the failure CLAUDE.md rule 6 was written from, reproduced. | `crates/copypaste-ui/src/**` (10 source files, one screen); `src-tauri/src/lib.rs` at `HEAD` exposes 6 commands, none of them peer-related. **[in flight]** — `commands/peers.rs` is untracked in the working tree. |
| **2** | **Daemon lifecycle ownership (v1 ADR-014)** | **Closed** — ADR-0004; `src-tauri/src/service/`; `components/shell/ServiceOffline.tsx`. Opening the app does not start the daemon; quitting does not stop it; after an upgrade a stale daemon keeps the socket. v1 fixed all three deliberately. v2's app tells the user to run `copypaste-daemon` in a terminal. | `git show archive/…:docs/adr/ADR-014-app-owned-daemon-lifecycle.md`; v2 `docs/adr/` (2 ADRs, neither about this); `MSG_UNREACHABLE` in `src-tauri/src/lib.rs`; no `packaging/`, no launchd plist, no `copypaste daemon install` verb. |
| **3** | **Sensitive-item auto-wipe (TTL)** | **Closed** — `core/src/sensitive/wipe.rs`, swept from the poll loop (`daemon/src/capture.rs`). The default TTL is an open product question under CLAUDE.md rule 4. v1 deleted a detected secret from history after 30 s by default, with `0` as an explicit "disabled" sentinel. v2 computes `Severity::HighConfidence` and then does nothing with it: a copied AWS key stays in history forever. | `storage/schema.rs` has no `expires_at` column; `storage/retention.rs` has cap + age eviction only; `grep -rn "expires\|ttl"` across core+daemon returns only the confidence-floor constants. README's "Not built" says *"age-based retention"*, which is a different feature (manifest 01 §3.17, manifest 07 §6.2). |
| **4** | **Re-copy does not bump an item to the top** | **Closed** — `Store::insert_or_bump` (`core/src/storage/items.rs`); the dedup probe is unbounded again (`retention.rs`). Copying something you copied last week creates a second row instead of promoting the first. Outside the 60-second dedup window, history accumulates duplicates. This is table-stakes clipboard-manager behaviour. | `capture.rs:117-140` — `Ingested::Duplicate` is logged and discarded, never bumped; `retention.rs` `DEDUP_WINDOW_MS = 60_000`. Manifest 01 T-36/T-37/T-39 (I-23), manifest 03 D9. CLAUDE.md rule 3 records the *bucket-width* fix, not the loss of unbounded dedup. |
| **5** | **Export / import** | **Closed** — `Method::Export` / `Import`, CLI `export` / `import`. No way to get your history out of the app, and no way to bring it in. v1 had `copypaste export`/`import` and IPC verbs, with `export.skipped_non_text` and `include_sensitive` defaulting false. | v2 `Method` enum (13 variants) and CLI subcommands (13) — neither has export or import. Not in README "Not built". |
| **6** | **Database backup / restore** | **Closed** — `Method::Backup` / `Restore`, `daemon/src/server/dbadmin.rs`. `reset_database` and `vacuum` still have no verb. v1 had `db_backup` / `db_restore` with a VALIDATE-then-SWAP against the real Keychain key, plus `copypaste backup`/`restore`. The only durable copy of an encrypted history has no supported backup path. | Same two enumerations. Manifest 04 §4.11 and `CopyPaste-crh3.6/crh3.2/8wbt`. |
| **7** | **Device revocation and sync-key rotation** | **Partly closed** — `PeerStore::revoke` / `revoke_all` / `revoked` exist, are persisted and refuse a revoked pairing on every read path; no IPC verb, CLI verb or UI reaches them, and rotation is unbuilt. `unpair` is local-only and documented as such ("its half of the pairing keeps working until it also unpairs"). v1 had `revoke_peer`, `revoke_all_peers`, `revoke_and_rotate`, `rotate_sync_key` and a `revoked_devices` audit table. A lost device cannot be cut off. | `Method::Unpair` doc comment in `copypaste-ipc/src/lib.rs:71-73`; `peers/store.rs` has `remove` and no revocation list; no rekey anywhere in `copypaste-core`. Manifest 04 §4.8/§4.16, `CopyPaste-gbo`. |
| **8** | **Pin state does not sync** | **Closed as a decision** — manifest 05 §3.6 amended 2026-07-30; pin stays local and a remote delete of a pinned row is refused. v2 keeps `pinned`/`pin_order` local on purpose. Manifest 05 §3.6 says the opposite, in the binding half, with a bug behind it. Pinning a note on the laptop leaves it unpinned on the phone. | `crates/copypaste-daemon/src/meta/write.rs:33-58` — the code states the divergence clearly. The manifest was **not** changed in the same commit, which CLAUDE.md rule 2 requires. |
| **9** | **User-facing settings of any kind** | **Closed** — `ipc/src/config.rs`, `daemon/src/settings.rs`, `Method::GetConfig` / `SetConfig`, CLI `config show` / `config set`, and `components/settings/ServiceTab.tsx` over `hooks/useServiceConfig.ts` for the eleven fields, driven by `e2e/tests/daemon-config.e2e.test.ts`. Credentials, passphrase, wifi-only and auto-apply remain absent. v1's `AppConfig` had 21 fields (poll interval, size caps, storage quota, sensitive TTL, exclusion list, sound/notify on copy, LAN visibility, sync toggles, `paste_as_plain_text`, …), reachable over `get_config`/`set_config` and a five-tab Settings screen. v2 has no config file, no config IPC, no Settings UI; every constant is compiled in. | `git show archive/…:crates/copypaste-core/src/config/mod.rs`; v2 has no `config` module and `daemon/src/main.rs` takes four CLI flags. Not in README "Not built". |
| **10** | **Keyset pagination and load-more** | v2's list is `LIMIT/OFFSET` with a single fixed page and no load-more in the UI. A row inserted above the window shifts everything down, so a second page duplicates or skips rows — the exact bug `CopyPaste-8ebg.57` fixed. | `storage/items.rs:99-110` (offset only; the order *is* total, which is the hard half); `hooks/useClipboard.ts:66` calls `listItems(PAGE_SIZE, 0)` and nothing else. Manifest 03 §3.12 (binding), manifest 06 INV-4 / AT-2 / AT-7. |
| **11** | **Quick-Paste popup and global hotkey** | **Closed** — `src-tauri/src/shell/{hotkey,window}.rs`. The primary interaction model of a clipboard manager on macOS: hotkey → popup → `↑↓` → `Enter` → pasted into the app you were in. v1 had `⌘1`–`⌘9`, plain-text paste, prior-app restore, blur-to-close. | `crates/copypaste-ui/src/**` has no popup route; `tauri.conf.json` at `HEAD` declares one window. Manifest 06 §3.5, INV-23/25/26. **[in flight]** — `src-tauri/src/shell/` adds a hotkey. |
| **12** | **Stale-socket bind is TOCTOU-racy again** | **Closed** — `BindLock`, an exclusive `flock(2)` held over probe→remove→bind (`server/listener.rs`). Two daemons on one database is a data-loss shape. v1 fixed this with `flock(2)` around probe→remove→bind (`CopyPaste-ah1m`). v2 does probe→remove→bind unguarded, so a second starter can unlink a freshly-bound socket and bind its own. | `server/listener.rs:61-72` — `clear_stale_socket`. There is a test (`a_stale_socket_file_is_replaced`) but it tests the single-starter path. |
| **13** | **IPC connection cap and read/write timeouts** | **Closed** — `MAX_CONCURRENT_CONNECTIONS` 64 via `try_acquire_owned`, `READ_TIMEOUT` 30 s, `WRITE_TIMEOUT` 10 s, plus a `MAX_WATCHERS` 8 sub-cap (`server/listener.rs`). v1 capped concurrent connections at 64 with non-blocking `try_acquire`, and bounded every read (30 s) and write (10 s), because a same-UID client that never drains otherwise pins a permit and the DB mutex forever. v2's accept loop spawns without limit and has no timeout. | `server/listener.rs:79-93`; `grep -n "timeout\|Semaphore"` across `server/` returns nothing. `CopyPaste-6ot5`, `CopyPaste-cce1`, `CopyPaste-c4q2.24`. |
| **14** | **Pairing codes never expire and are not single-use** | **Closed** — `PAIRING_CODE_TTL` 300 s (`p2p/src/peers/`); an unredeemed pairing carries a deadline and the first completed session burns it. `pair_create` mints a 256-bit PSK, stores the peer immediately, and returns the code. Nothing burns or ages it, so a code written on a sticky note stays a working credential indefinitely. v1 had `QR_PAIRING_TTL_SECS` and INV-28's single-use semantics. | `p2p/transport/token.rs`, `p2p/peers/store.rs` (no expiry field, no consume). Manifest 06 INV-28, manifest 04 `CopyPaste-8ebg.59/.65`. |
| **15** | **Push/streaming updates (`watch_subscribe`)** | **Closed** — `Method::Watch`, `daemon/src/server/watch.rs`, CLI `watch`, `ui/src/hooks/usePush.ts`. v1 clients could subscribe to a change stream. v2's app polls every 3 s and the CLI has no `watch`. Poll-only means up to a 3 s lag and constant IPC traffic. | v2 `Method` enum; `hooks/useClipboard.ts` `POLL_ACTIVE_MS = 3000`. `CopyPaste-44rq.19`. |
| **16** | **Discovery is not reachable from any client** | **Closed** — `Method::Discovered` / `Rescan`, CLI `discover`. `copypaste-p2p::discovery` exists and is tested, and `PeerInfo.online` uses it — but there is no `list_discovered` / `rescan_discovered`, so an unpaired device on the LAN is invisible. Pairing always requires typing an address. | v2 `Method` enum; `p2p/handlers.rs` exposes five operations. Manifest 04 §4.12. |
| **17** | **Undecryptable rows are skipped but not counted** | **Closed** — `ItemPage::skipped_undecryptable` on the wire, `components/history/SkippedNotice.tsx` in the app. v1's `decrypt_page` returned `DecryptedPage::skipped` so the UI could say "3 items could not be read". v2 logs and silently shortens the page — the user sees fewer items with no explanation. | `daemon/src/server/items.rs:184-194` (`decrypt_rows`). `CopyPaste-00zz`, manifest 03 Q10. |
| **18** | **Notifications and sound on copy** | **Half closed** — `daemon/src/notify.rs`, called from the capture tick, plays the system alert sound on macOS behind `sound_on_copy` and suppresses itself whenever the clipboard backend is a fake (every `cargo test`, every demo, every non-macOS host). The notification is not built: posting it needs `UNUserNotificationCenter` and therefore an application bundle, so it has to come from the app, which reads `notify_on_copy` back out of `get_config`. v1 fired both for background captures. Without them a background capture is invisible. | `daemon/src/notify.rs`; `daemon/src/capture.rs`. Manifest 06 §3.6, manifest 01 §3.23. |
| **19** | **Bulk actions, filtering, sorting, drag-to-reorder pins** | **Partly closed** — bulk actions, filter and sort landed (`components/history/BulkBar.tsx`, `lib/view.ts`). Drag-to-reorder now has every layer *below* the screen: `Store::reorder_pinned`, `Method::ReorderPinned`, the daemon's dispatch, the CLI's `reorder`, and a `useReorderPinned` hook. There is no drag affordance in the list, and both Tauri backends still refuse the call on reasons their own comments got wrong. v1's History had multi-select with bulk pin/delete, filter by kind and by origin device, sort options, and drag-to-reorder pinned items (`pin_order`, `reorder_pinned`). v2 has one flat list. `pin_order` *is* maintained — `set_pinned` appends a new pin at `MAX(pin_order) + 1` and clears it on unpin — so pins have a stable order; there is simply no way for the user to change it. | `storage/items.rs:172-203` (`set_pinned`), `storage/schema.rs` (`pin_order REAL`), `components/HistoryList.tsx`; no `reorder` in the `Method` enum or the CLI. Manifest 06 §3.1.6/§3.1.7/§3.1.9. |

### 1.2 Where the record is thinner than the code

Three places where a decision *was* taken but is recorded only in a source
comment, which CLAUDE.md rule 2 and the ADR convention both suggest is not
enough:

* **Pin is local, not LWW** — `daemon/src/meta/write.rs`. Contradicts a binding
  manifest section that was not amended. (Finding 8.)
* **No span/redaction API and no password-manager bundle list** —
  `core/src/sensitive/mod.rs:33-36`. Good rationale, correctly reasoned, in a
  module doc comment rather than an ADR.
* **Dedup is windowed rather than unbounded** — `core/src/storage/retention.rs`.
  The comment justifies the *window width*; it does not acknowledge that v1's
  dedup had no window at all. (Finding 4.)

### 1.3 Where `README.md`'s "Not built" section is accurate — and where it is not

The section was worth checking rather than trusting. Result: **accurate on
everything it lists**, with two verified-correct entries that could have been
wrong, and incomplete by nine items.

Verified correct:

* *"`evict_older_than` exists, no loop calls it"* — true; `grep` finds only test
  callers. And `evict_over_cap` **is** called (`capture.rs:216`), so the
  claim is precise rather than sweeping.
* *"`governor` is declared in the workspace manifest and unused"* — was true;
  the declaration was removed on 2026-07-30 rather than left standing as a claim
  the code did not honour. Rate limiting is still unbuilt, which is now visible
  as an absence instead of as a dependency.
* *"cloud sync … not wired into the daemon or the CLI"* — true at `HEAD`: the
  `Method` enum has no cloud variants and `dispatch` is exhaustive without them.
  **[in flight]**.

Understated or absent:

| README says | Reality |
|---|---|
| "frontmost-app attribution" | It is not only attribution. Manifest 07 §5.8 / I-6 makes the frontmost bundle id an **independent sensitivity signal** — the one that catches a password with no detectable shape. Losing it is a security capability loss, not a metadata one. |
| "age-based retention" | Distinct from the sensitive TTL auto-wipe, which is also missing and is not mentioned. (Finding 3.) |
| — | Export/import, backup/restore, revocation/rotation, config & settings, keyset pagination, streaming, discovery listing, notifications, daemon lifecycle. (Findings 2, 5, 6, 7, 9, 10, 15, 16, 18.) |

`README.md` also contains an internal contradiction: the header says
`crates/copypaste-ui` "is the product surface, not a placeholder", while the
Works table calls the same crate an "Interim history window … Temporary". One
of those should go.

`SECURITY.md`'s "Not implemented" list (age-based retention, private mode,
exclusion list, rate limiting, telemetry) is more disciplined than the README's
but has the same three omissions: the sensitive TTL, revocation, and the fact
that a pairing code never expires.

---

## 2. Capability inventory and verdicts, per subsystem

Organised by the seven port manifests, plus a cross-cutting section for what no
manifest owns.

### 2.1 Clipboard capture (manifest 01)

| Capability (v1) | Verdict |
|---|---|
| `changeCount`-driven change detection, cursor advanced on every drop path | **Ported & tested** — `clipboard/change.rs`, 8 tests. The state machine was extracted from the platform binding precisely so it is testable on Linux; that is the right call and it worked. |
| Burst handling — surviving item captured, intermediates counted, never a burst-only result | **Ported & tested** — `burst_does_not_eat_the_survivor`, `burst_threshold_boundary`, `first_observation_is_not_a_burst`. The highest-value rule in the manifest, and it is guarded. |
| Two-sided self-write sentinel with conditional post-stamp | **Ported & tested** — 4 tests, including `third_party_write_during_ours_is_still_captured` (`CopyPaste-8yzf`). |
| `org.nspasteboard.*` markers, all three, probed before any read | **Ported, untested** — `clipboard/macos.rs:150-159` is correct by inspection. No seam exists on the fake to exercise it, so nothing would catch its removal. |
| Text representation, size gate, UTF-8 lossy conversion | **Ported & tested** (`oversized_content_is_rejected_and_counted`). |
| Cocoa string constants hoisted out of the tick (`CopyPaste-pbre`) | **Ported, untested** (`thread_local! UTIS`). |
| Autorelease pool around the whole tick (I-17) | **Ported, untested**. |
| Image capture (PNG/TIFF), thumbnails, decode-bomb budget | **Deliberately dropped** — README "Not built". |
| File capture, `NSFilenamesPboardType` binary plist, `public.file-url` percent-decoding, MIME derivation | **Deliberately dropped** — same. |
| Rich text | **Deliberately dropped** — same. |
| Representation priority (text > image > file) | **N/A** while text-only; the macOS module's comment reserves the slot. |
| Frontmost-app resolution + 750 ms cache, fail-closed | **Deliberately dropped** — README "Not built", though understated (§1.3). |
| App exclusion list, fail-closed when non-empty | **Deliberately dropped** — README + SECURITY.md. |
| Private mode | **Deliberately dropped** — README + SECURITY.md. |
| Sensitive TTL cleanup, `0` sentinel, startup purge before socket bind | **⚠ Silently missing** (finding 3). |
| Poll interval / size caps hot-reload | **⚠ Silently missing** as a consequence of finding 9 (no config to reload). |
| Broadcast channel absorbing bursts | **N/A** — no subscriber exists (finding 15). |
| Sound on copy, suppressed in test envs | **⚠ Silently missing** (finding 18). |
| Non-macOS is a silent no-op | **Solved differently, tested** — v2 substitutes a drivable fake (`clipboard/fake.rs`, 9 tests) rather than a no-op. Strictly better: the pipeline is demonstrable off a mac. |

### 2.2 Crypto (manifest 02)

The strongest subsystem in the audit. Everything the port-manifest README lists
as binding is present, and most of it is tested.

| Capability | Verdict |
|---|---|
| XChaCha20-Poly1305, 24-byte OsRng nonce | **Ported & tested** — `nonces_are_unique_and_never_all_zero`, `two_encryptions_of_the_same_plaintext_use_different_nonces`. |
| AAD binds item identity; fail closed on wrong key / wrong AAD | **Ported & tested** — `wrong_item_id_fails_closed`, `item_aad_is_injective_across_delimiter_abuse`, `tampered_ciphertext_fails_closed_in_every_byte`, `swapped_nonces_both_fail`. The delimiter-abuse injectivity test is better than v1's. |
| HKDF-SHA256 derivation, domain separation | **Ported & tested** — `hkdf_info_strings_are_the_documented_ones`, `db_key_and_item_key_are_domain_separated`. |
| Zeroization, constant-time compare, no key material in `Debug` | **Ported & tested** — `assert_zeroize_on_drop`, `ct_eq_agrees_with_byte_equality`, six `debug_*_never_prints_key_material` tests. |
| Keychain service/account naming frozen | **Ported & tested** — `keychain_identifiers_are_frozen`. |
| macOS Keychain store | **Ported, unverified.** *Was* "behind `macos-keychain`". The feature is gone: `security-framework` is target-gated and the backend is selected by `target_os` alone, so no build can be configured into the file store. Compiled and linted on `macos-14` in CI; still never executed. |
| Android Keystore store | **Landed after this anchor** — `crypto/keystore/android.rs`, target-gated the same way. An AES-GCM key that never leaves the Keystore wraps the device secret; the wrapped blob sits in app-private storage. Never compiled — no NDK. |
| `0600` file key store as the non-macOS fallback | **Ported & tested** — `the_file_is_owner_only`, `file_backed_store_round_trips_and_rejects_a_wrong_key`. Correctly described in README as a development posture. |
| Argon2id cloud sync-key derivation, per-account salt | **Ported & tested** — `argon2_parameters_are_the_documented_ones`, `per_account_salt_is_deterministic_and_unique`, `another_accounts_key_fails_closed`. |
| Chunked AEAD (`CHUNK_FORMAT_V1` → `aead::stream`) | **N/A** — chunking existed for large image/file blobs. Text-only makes it moot; correctly not built. |
| `key_version` dispatch, rotation sweep, repair pass | **Deliberately dropped** — CLAUDE.md rule 3, README "No upgrade path". |
| Verbatim HKDF info strings, AAD byte layouts, CPPAIR envelopes | **Deliberately dropped** — port-manifest README puts these in the reference column. |
| OPAQUE / PAKE bootstrap | **Solved differently** — Noise `NNpsk0`, where possession of the pairing token *is* the authentication. Manifest 02 §6.3 recommends exactly this. |
| Database rekey (`Database::rekey`) | **⚠ Silently missing** — a corollary of finding 7. Manifest 03 §3.6, E5/E6. |

### 2.3 Storage (manifest 03)

| Capability | Verdict |
|---|---|
| SQLCipher at rest, raw key, WAL | **Ported & tested** — `database_is_encrypted_on_disk_and_in_wal_mode` covers E7 + E8 + P1 in one assertion. (It does not check `-wal`/`-shm` separately, as v1 did.) |
| Wrong key rejected, never a plaintext fallback | **Ported & tested**. |
| Sensitive never in FTS — all three layers | **Ported & tested, exemplary.** `storage/search.rs` names the three layers in its module doc and implements all three; `sensitive_items_never_reach_the_search_index`, `search_never_returns_a_sensitive_item`, plus a server-side read-time test. The *migration* is genuinely N/A under one schema — but a purge **pass** landed after this anchor (`sensitive/purge.rs`, run at daemon start), for the different reason that a rule added later never revisits rows captured before it. |
| Tombstones: content and nonce wiped, FTS row removed in the same transaction, hash retained | **Ported & tested** — `delete_tombstones_the_row_and_clears_the_index`. |
| Pinning: pinned sort first, never evicted by cap or TTL | **Ported & tested** — `list_is_pinned_first_then_newest_first`, `eviction_respects_pins`, `ttl_eviction_respects_pins`, `delete_all_leaves_pinned_items_intact`. |
| `pin_order` maintained on pin/unpin | **Ported, untested** — `set_pinned` appends at `MAX(pin_order) + 1`, clears on unpin, and `delete` clears it too. No test asserts the ordering, only `set_pinned_toggles_and_reports_existence`. |
| Drag-to-reorder (`reorder_pinned`) | **Closed below the UI** (finding 19) — `Store::reorder_pinned`, `Method::ReorderPinned`, CLI `reorder`. No drag affordance, and both Tauri backends refuse. |
| Dedup with a UNIQUE-index TOCTOU backstop | **Ported & tested, improved** — the schema comment explains why the index excludes tombstones so a re-copy after delete is a fresh row (v1's D4). Tests: `dedup_index_makes_a_same_bucket_reinsert_idempotent`, `a_collision_with_the_dedup_index_is_refused_not_an_error`. |
| Unbounded dedup + recency bump | **⚠ Silently missing** (finding 4). |
| Cap eviction, oldest unpinned first, newest exempt | **Ported & tested** and wired (`capture.rs:216`). |
| Age-based eviction | **Deliberately dropped** (unwired) — README, explicitly and precisely. |
| Sensitive `expires_at` TTL | **⚠ Silently missing** (finding 3). |
| Keyset pagination contract | **⚠ Silently missing** (finding 10). The *total order* is present and documented; only the seek predicate is not. |
| Connection pool (r2d2) for reads | **Ported, thinly tested** — `in_memory_pool_shares_one_database`, `concurrent_upserts_all_land`, `a_torn_write_cannot_be_observed`. No P5-equivalent (pool refuses an unmigrated file) and no pool-stress test. |
| FTS5 query sanitiser (hyphens, quotes, specials) | **Ported & tested** — `fts5_query_sanitizer`, and the module comments each rule with the bug it came from. |
| Full-schema migration ladder v1→v15, `migration_state`, v4 sweep, purge of dead v1 rows | **Deliberately dropped** — CLAUDE.md rule 3. `rusqlite_migration` replaces the hand-rolled runner exactly as manifest §6.1 recommended, and `a_future_schema_version_is_refused` preserves the downgrade guard (M7). |
| Distinct v2 filename so a v1 DB is never touched | **Ported & tested** — `default_file_name_is_not_v1s`. This is half of CLAUDE.md rule 3's one obligation. The other half — saying so plainly rather than reading as corruption — is **not** met: `storage/legacy.rs` landed after this anchor and nothing on the startup path calls it against a v0.4 filename. B-4. |
| `db_backup` / `db_restore` / `vacuum` / `reset_database` | **⚠ Silently missing** (finding 6). |

### 2.4 IPC and CLI (manifest 04)

The port-manifest README makes the method catalogue binding **as a feature
inventory**. Measured that way: 61 v1 operations → 13. The reduction breaks down
as follows.

| v1 operations | Count | Disposition |
|---|---|---|
| Covered by a v2 method | 13 | `status`, `history_page`→`List`, `search`, `copy_item`→`Copy`, `delete_item`→`Delete`, `delete_all`, `pin_item`→`Pin`, `list_peers`→`Peers`, `unpair_peer`→`Unpair`, + `Add`, `PairCreate`, `PairAccept`, `SyncNow` |
| Legacy/deprecated verbs kept only for wire compat | 6 | **Deliberately dropped** — port-manifest README retires wire compatibility |
| Media / image / file / icon | 5 | **Deliberately dropped** — text-only |
| Config | 2 | **⚠ Silently missing** (finding 9) |
| Cloud & sync keys | 8 | Split: 4 exist as unwired IPC variants **[in flight]**; `rotate_sync_key`, `revoke_and_rotate`, `set_sync_passphrase`, `cloud_test_connection` are **⚠ silently missing** (finding 7) |
| Private mode | 2 | **Deliberately dropped** |
| DB admin | 5 | **⚠ Silently missing** (finding 6) |
| Peers & discovery beyond `list_peers` | 3 | **⚠ Silently missing** (finding 16) |
| Pairing: SAS flow (5), QR (2), password/PAKE (3), revoke (3) | 13 | SAS and QR: see below. Revocation: **⚠ silently missing** (finding 7). Password/PAKE: **deliberately dropped** (v1 itself disabled two of the three) |
| Import / export | 2 | **⚠ Silently missing** (finding 5) |
| `watch_subscribe` | 1 | **⚠ Silently missing** (finding 15) |
| `count`, `stats` | 2 | **Solved differently** — `StatusData.item_count` |

Binding non-method items:

| Capability | Verdict |
|---|---|
| Error-code taxonomy | **Ported, reduced.** 5 codes at `HEAD` (`not_found`, `invalid_request`, `protocol_mismatch`, `not_ready`, `internal`) + `auth_failed` **[in flight]**. `rate_limited` and `not_implemented` have no producer, correctly. `request_too_large` is folded into `invalid_request` — a small loss of client precision (v1's `CopyPaste-c4q2.27` wanted "payload too large" to be distinguishable). |
| Errors never leak paths | **Ported & tested, thoroughly.** `copypaste-ipc/src/redact.rs` with 6 tests including `no_username_survives_a_realistic_socket_error`; `error_messages_contain_no_paths` appears **six** times across crates; the frontend has its own `a_daemon_supplied_path_never_reaches_the_frontend`. This is CLAUDE.md rule 4 and it is well guarded. |
| Readiness / degraded mode, `status` exempt from the gate | **Ported & tested** — `requires_ready` has no `_` arm (so a new method must be classified), `only_status_answers_before_readiness`, `not_ready_reads_as_still_starting_up`. |
| Client retry on `not_ready` — bounded, reconnecting | **Ported & tested** — `not_ready_is_retried_on_a_fresh_connection`, `not_ready_gives_up_rather_than_retrying_forever`. |
| Protocol-version gate | **Ported & tested** — `mismatched_protocol_version_is_rejected`, `status_warns_on_a_protocol_difference`. |
| One typed wire contract shared by daemon, CLI and bridge | **Solved much better than v1.** v1 modelled it three times and the CLI imported none of them; v2 has one `Method` enum and both dispatchers are exhaustive over it, so an unhandled method is a compile error. This is the single clearest win in the rewrite. |
| `0600` socket, stale-socket self-healing | **Ported & tested** (`the_socket_is_owner_only`, `a_stale_socket_file_is_replaced`) but the `flock` guard is gone — finding 12. |
| Framing size cap | **Ported** via `LinesCodec::new_with_max_length`; the two-pass method-aware cap is correctly not needed without bulk verbs. |
| Connection cap, read/write timeouts | **⚠ Silently missing** (finding 13). |
| CLI `--json` for scripting | **Ported & tested** — `json_is_accepted_before_and_after_the_subcommand`. |
| CLI `daemon start/stop/restart/install/uninstall` | **⚠ Silently missing** (part of finding 2). |
| CLI `watch` | **⚠ Silently missing** (finding 15). |

### 2.5 Peer sync (manifest 05, P2P half)

| Capability | Verdict |
|---|---|
| Deterministic, symmetric, total merge order | **Ported & tested, and better specified than v1.** One comparator, four keys, and `merge.rs`'s module doc explains why `deleted` sits at key 3 — reasoning v1 did not have written down. Tests: `merge_is_symmetric_across_the_whole_decision_space`, `merge_is_exactly_the_four_key_lexicographic_order`, `summary_comparator_agrees_with_the_full_one` (the `CopyPaste-ayvs` guard), `arrival_order_does_not_change_the_outcome`, `two_divergent_devices_converge`, `a_third_session_after_three_way_sync_is_a_fixed_point`. |
| Lamport clock | **Solved differently** — v2 orders on `created_at → content_hash → deleted → origin_device_id` and refuses implausibly-future stamps instead of maintaining a logical clock. The port-manifest README sanctions exactly this ("monotonicity is replaced by refusing implausibly-future stamps"). Tests: `a_version_beyond_the_skew_ceiling_is_skipped`, `a_row_stamped_far_in_the_future_is_refused_and_does_not_move_the_cursor`. |
| Lower-bound clamp at the decode boundary (`CopyPaste-psx7`, R-CLK-1) | **Ported & tested** — `negative_timestamps_are_clamped_on_decode`, `negative_timestamps_are_clamped_inside_items_too`, `a_negative_stamp_is_clamped_at_the_boundary`. Validated at the decode boundary, as the rule requires. |
| Delete-wins; a tombstone carries no ciphertext | **Ported & tested** — `a_newer_tombstone_beats_an_older_live_version`, `an_older_live_version_cannot_resurrect_a_newer_tombstone` (×2 crates), `a_constructed_tombstone_carries_no_payload`, `a_tombstone_carrying_ciphertext_is_refused_before_anything_is_sent`. |
| Delete-before-create (`CopyPaste-bfiu`) | **Ported & tested** — `a_delete_for_an_item_the_peer_never_saw_is_stored`, `a_tombstone_reaches_the_store_even_for_an_unknown_item`. |
| Idempotency / replay safety | **Ported & tested** — `replaying_a_whole_session_changes_nothing`, `applying_the_same_item_twice_stores_it_once`, `applying_the_same_version_twice_is_a_no_op`. |
| `is_sensitive` recomputed on the receiver (`CopyPaste-kcf`) | **Ported & tested** — `an_incoming_secret_is_flagged_by_this_devices_detector`. |
| Sensitive items never leave their origin (`CopyPaste-20yw`) | **Ported & tested** — 4 tests across p2p and cloud, including `a_sensitive_item_is_withheld_even_when_it_is_the_only_one`. |
| `origin_device_id` preserved, never restamped | **Ported & tested** — `a_recorded_origin_is_never_restamped`. |
| `pinned` / `pin_order` travel with a version | **⚠ Silently missing / manifest conflict** (finding 8). |
| mTLS + rcgen + pinning verifier + two hand-written DER parsers | **Solved differently** — Noise `NNpsk0` over TCP. Fewer moving parts, one crypto stack, and tested (`round_trip_over_loopback_in_both_directions`, `wrong_psk_fails_the_handshake_on_both_sides`, `tampered_frame_fails_authentication_and_poisons_the_session`). |
| 6-digit SAS confirmation after a PAKE | **Solved differently** — with `NNpsk0` the transferred code *is* the mutual authenticator, so a separate short-authentication-string comparison is redundant rather than dropped. The *UI* for it, however, is gone (finding 1) and manifest 06 lists the SAS flow as binding UI behaviour. |
| QR pairing that provisions every transport in one scan | **⚠ Partially missing** — no QR anywhere. The multi-transport provisioning is moot (one transport at `HEAD`), but "scan a code" vs "read out 52 base32 characters" is a real usability regression on a phone. Folded into finding 1. |
| mDNS-SD discovery | **Ported & tested** (`copypaste-p2p/src/discovery/`, incl. hostile-TXT bounding, flood eviction, `start_degrades_without_multicast`) but not reachable by a client (finding 16). Multicast is unavailable here, so runtime behaviour is unverified — declared. |
| Pairing-code TTL / single use | **⚠ Silently missing** (finding 14). |
| Device revocation + rotate | **⚠ Silently missing** (finding 7). |

### 2.6 Cloud and relay (manifest 05, backend half)

At `HEAD`, `copypaste-cloud` is 7,519 lines and 157 tests wired to nothing. That
is the largest concentration of test effort in the tree, on the one crate no
user can reach.

| Capability | Verdict |
|---|---|
| The relay server (~12k lines, Axum + SQLite + quota + TTL + PoP + metrics) | **Deliberately dropped** — manifest 05 §5.1 row 17 and §5.4 endorse it. |
| Shared-account fan-out | **Solved differently** — one table + RLS on `user_id`, and upsert-on-`item_id` removes the duplicate-row class. Manifest agrees this is better. |
| Independent per-device credentials | **Solved differently** — one GoTrue session per device. |
| Keyset cursor pagination | **Ported & tested** — `fetch_since_asks_for_an_inclusive_bound_and_a_total_order`, `a_page_is_put_into_cursor_order_before_it_is_applied`, `pull_drains_more_rows_than_one_page`. |
| **The polling backstop** (§5.4 obligation 1 — "the only item that can silently reintroduce data loss") | **Ported & tested, and correctly framed.** `sync/cadence.rs` treats Realtime as an accelerator: `a_realtime_event_can_wake_the_poll_loop`, `any_change_resets_the_idle_interval`, `the_idle_interval_grows_and_is_bounded`. This was the highest-risk item in the manifest and it was handled. |
| Server-side quota and TTL (§5.4 obligation 2) | **Missing at `HEAD`** — declared in README ("quota/TTL job"). **[in flight]**: `supabase/migrations/…_retention.sql` lands a `pg_cron` job that orders on the server-assigned `inserted_at`, honouring rule 4a (`CopyPaste-1uqb`) explicitly. |
| Threat-model change recorded (§5.4 obligation 3) | **Closed** — `docs/cloud-privacy.md` is the page the obligation asks for: the column-by-column disclosure, the two-secrets split (account password gates the rows, sync passphrase decrypts them), the metadata regression against v1's account-less relay, and what the row signature does and does not stop. `SECURITY.md`'s cloud section carries the summary and links to it. |
| Signed LWW metadata (§5.3 mitigation) | **Closed** — `cloud/src/crypto/sign.rs`. Every row carries an HMAC over the ordering fields plus the ciphertext and nonce, under a second key from the sync passphrase; `CloudSync::pull` verifies before the row reaches the merge and refuses what does not verify. The round counts its refusals in `SyncStats::skipped_forged`, and that count does not yet reach `CloudStatusData`. |
| 401 → refresh → retry once; second 401 is hard | **Ported & tested** — `a_401_refreshes_once_and_retries_once`, `a_second_401_is_a_hard_error_not_a_loop`, on both read and write paths. |
| `invalid_grant` disambiguation (password vs refresh grant) | **Ported & tested** — `invalid_grant_on_the_password_grant_is_bad_credentials`, `the_same_body_on_the_refresh_grant_is_an_expired_session`, `the_disambiguation_holds_for_422_and_401_too`. A subtle v1 lesson, carried faithfully. |
| 429 + `Retry-After`, clamped, single-shot | **Ported & tested** — 6 tests. |
| Non-JSON error body preserved as a truncated snippet | **Ported & tested** — `a_gateway_html_page_is_truncated_rather_than_lost`. |
| `expires_at` saturation on a hostile `expires_in` | **Ported & tested** — `a_hostile_expires_in_saturates_instead_of_wrapping`. |
| Token/email redaction in `Debug` | **Ported & tested** — `debug_for_a_session_shows_no_token`, `emails_are_masked_for_logs`, `a_session_zeroizes_on_drop`. |
| Phoenix wire format, join payload, join gating, reconnect backoff | **Ported & tested** — `a_five_element_frame_parses`, `a_numeric_ref_is_absent_not_empty`, `the_join_frame_carries_the_jwt_the_wildcard_and_the_filter`, `only_an_ok_reply_on_our_topic_confirms_the_join`, `the_reconnect_schedule_grows_and_is_bounded`. |
| Cursor advances past unusable rows; undecryptable row never deletes the local copy | **Ported & tested** — `an_undecryptable_row_is_skipped_and_never_deletes_the_local_copy`, `cursor_advances_after_the_delta_is_computed`. |
| Watermark never moves backwards | **Ported & tested** — `the_watermark_only_moves_forward`, `push_does_not_move_the_download_watermark`, and the fake source *asserts* it. |
| Watermark persists across restart (AT-26) | **Not verifiable at `HEAD`** — the `SyncSource` trait defines `watermark`/`set_watermark` and only the in-memory fake implements it. There is no daemon-side implementation to persist. Folds into "cloud not wired". |
| RLS policy static audit (AT-51) | **Missing at `HEAD`** — the policies are documented in `rest/mod.rs`'s module doc but there was no SQL to assert against. **[in flight]** with `supabase/migrations/`. |
| Mutations survive a backend outage (AT-33, `CopyPaste-1t38`) | **⚠ Not found.** No broadcast ring, no outbound mutation queue. A pin/delete made while the backend is down relies on the next full push round to notice. Not a data-loss shape given LWW, but the v1 test does not have an analogue. |
| Pre-passphrase backlog sweep (AT-32, "BUG C2") | **⚠ Not found.** No test asserts that setting a passphrase after capturing items uploads the backlog. |

### 2.7 UI behaviour (manifest 06)

**Superseded by [`ui-parity-audit.md`](ui-parity-audit.md)**: Settings, Devices,
pairing, bulk actions, filter, sort, load-more, tray and hotkey landed after this
anchor. The port-manifest README is unusually explicit here: the behaviour and
accessibility half is binding **in full**, and "the toolkit does not get to drop
the behaviour". Against that bar:

| Capability | Verdict |
|---|---|
| History list: search, virtualised rows, copy / pin / delete | **Ported** — `App.tsx` + 5 components. Search is debounced at the manifest's 250 ms. |
| INV-1 scroll anchoring to content | **Ported, untested** — `hooks/useScrollAnchor.ts` (112 lines) implements it. No test. AT-1/AT-3 have no analogue. |
| INV-6 shrink clamp | **Ported, untested** — same hook. AT-4 has no analogue. |
| INV-5 row heights over-reserved, never estimated by character count | **Ported & tested** — `row height reserves the full preview cap` (2 assertions), including "does not vary with content length". One of the three rules the port-manifest README singles out, and it is guarded. |
| INV-2 / INV-3 / INV-33 identical data → identical reference, mutations invalidate, late responses lose | **Solved differently** — React Query's `structuralSharing`, `invalidateQueries` and per-key last-write-wins replace ~1,000 lines of hand-rolled polling. `useClipboard.ts` maps each invariant to the mechanism that satisfies it. Manifest §9.1 asks for exactly this substitution. **Untested** — no AT-5/AT-6 analogue. |
| INV-27 visibility-gated polling | **Solved differently** (`refetchIntervalInBackground: false`), untested. |
| INV-8 rows are `listitem`, never `option` | **Ported** and reasoned in a comment; enforced only by inspection. |
| INV-10 masked content never reaches the a11y tree | **Ported & tested** — `keeps the plaintext out of the DOM entirely`, `labels the row without quoting the secret`. The manifest's stronger form — sensitive content *absent from the view* rather than obscured over the top — is what v2 implements. |
| INV-11 revealed secrets re-hide automatically | **Ported, untested** — `hooks/useReveal.ts`. |
| INV-12 raw errors never rendered | **Ported & tested** — `every error kind has non-empty copy that names no path`, plus `lib/errors.ts` classification. |
| INV-4 load-more merges | **⚠ Silently missing** — there is no load-more (finding 10). |
| INV-9 keyboard selection announced (A11Y-2) | **Ported, untested** — `aria-live` announcer is a sibling of the list, which also satisfies A11Y-14 (`CopyPaste-wrfn`). |
| A11Y-1 list semantics, A11Y-3 masked content, A11Y-9 icon names | **Ported, untested** — present in the markup. |
| A11Y-4 dialogs, A11Y-6 tablist, A11Y-7 disclosures, A11Y-8 toggles, A11Y-13 shortcut control | **N/A** — no dialogs, tabs, disclosures or shortcut control exist. |
| A11Y-10 contrast, A11Y-11 reduced motion, A11Y-12 reduced transparency, A11Y-15 720×460 minimum | **⚠ Not found.** These are behaviour, not palette, so the "visual is reference only" carve-out does not cover them. Reduced-motion and reduced-transparency in particular are token-layer rules that survive a redesign. |
| INV-17/18/19 banner priority queue, single modal, ref-counted scroll lock | **N/A** — no banners or modals. |
| INV-21/22 per-field prefs defaulting, first paint carries appearance | **N/A** — no prefs. |
| INV-23/24 physical-key shortcut capture, failed registration must not crash startup | **N/A** at `HEAD`. **[in flight]** — `shell/hotkey.rs` logs a warning on failure, which satisfies INV-24. |
| INV-25/26 hide hands focus to the prior app; copy-then-hide | **N/A** — no popup (finding 11). |
| INV-29 optimistic writes revert on failure | **Ported, untested** — `useDeferredDelete` with an undo window. |
| INV-31 pinned items must not jump to the top on copy | **Ported by construction** — v2 does not bump on copy at all (which is finding 4 seen from the other side). |
| INV-32 selection tracked by id | **Ported** — `activeId` state. |
| INV-35 screen-capture protection on by default | **⚠ Silently missing.** `tauri.conf.json` sets no content protection. A screen recorder captures the history window. |
| INV-36 closing the window hides it; only tray → Quit exits | **Ported** — the tray menu comments cite INV-36. Untested. |
| INV-38 tray checkmark reflects daemon truth | **N/A** — no private mode to reflect. |
| Devices screen, Settings (5 tabs), Pairing modals, Quick-Paste popup, toasts, sync-status chip | **⚠ Silently missing** (findings 1, 9, 11). |
| v1's palette, spacing, type, elevation, translucency scales | **Deliberately dropped** — port-manifest README, ADR-0002, README. `design/` holds v1's values as declared placeholders, which is the correct posture (the README warns specifically against quietly re-deriving them). |

### 2.8 Secret detection (manifest 07)

Alongside crypto, the best-carried subsystem — and the one where v2 most clearly
improved on v1 rather than merely reproducing it.

| Capability | Verdict |
|---|---|
| The ruleset | **Ported & tested, improved.** v1 had 40 regex rules + an off-table credit-card check; v2 has 42 first-class rules, with the card rule promoted into the table exactly as manifest §3.3 demanded ("v2 must make the card rule a first-class rule"). `rule_count_parity`, `rule_names_are_unique`, `ruleset_compiles`, `manifest_true_positives_are_detected`, `manifest_hard_negatives_produce_no_detection`. |
| gitleaks as the source (CLAUDE.md rule 1) | **Ported** — rules cite their gitleaks origin and say where the manifest's stricter version was kept instead (e.g. the `generic-api-key` entropy gate rejected the manifest's own fixtures, so the variety gate was kept). Exactly the reasoning rule 1 asks for. |
| Detection and deletion kept separate (I2) | **Ported & tested** — `Detector::is_sensitive` gates the index; `Severity` gates deletion. `inert_band_is_detected_but_never_auto_wipes`. v1 collapsed these three separate times. |
| 0.70 auto-wipe floor; nothing sits exactly on it | **Ported & tested** — `no_rule_sits_exactly_on_the_floor` (the `CopyPaste-8ys1` guard), `fp_risk_patterns_below_autowipe_floor`, `high_confidence_true_positives_are_above_the_floor`. |
| NFKC normalisation and the bypasses it closes | **Ported & tested** — `nfkc_bypass_closed_for_full_width_digits_and_cards`, `normalise_is_the_identity_on_ascii`, and `zwj_bypass_is_documented_and_still_open` — a test that *documents a known open gap* rather than hiding it. |
| Luhn validation, single implementation | **Ported & tested** — `negative_card_fixtures_are_actually_negative`, `ssn_structure_rejects_impossible_groups`. v1 had two Luhn implementations (§7.3); v2 has one. |
| Value-strength gate, extended to `dotenv_secret` | **Ported & tested, improved** — v1 left `dotenv_secret` at 0.80 with no validator, so `API_KEY=changeme` auto-wiped. v2 adds a capture group and shares the gate. `value_strength_follows_the_manifest_criteria`, `dotenv_secret_is_value_gated`, `multibyte_value_gated_on_chars_not_bytes`. |
| Structural / context anchoring | **Ported & tested** — `context_anchored_rules_do_not_match_without_their_anchor`, `word_anchors_reject_glued_tokens`, `prefixed_token_rules_are_word_anchored`, `openai_legacy_does_not_double_fire_on_proj_keys`. |
| Highest-confidence wins, not lowest declaration index (§7.2) | **Ported & tested** — `scan_ranks_by_confidence_not_declaration_order`. A real v1 bug, fixed. |
| False-positive corpus budget | **Ported & tested** — `benign_corpus_has_zero_false_positives` (v2 asserts zero; v1's budget was ≤5 % with a floor of 2). Stricter. |
| Performance on large input | **Ported & tested** — `large_benign_input_completes_quickly`, `secret_embedded_in_a_large_benign_document_is_still_found`. |
| I4 sensitive never indexed | **Ported & tested** — see §2.3. |
| I5 sensitive items carry no thumbnail | **N/A** — no thumbnails. |
| I6 password-manager bundle-ID signal | **Deliberately dropped** — recorded in `sensitive/mod.rs`'s module doc with a reason ("a capture-time signal about the source app, carried as configuration"). Correctly reasoned; understated in README (§1.3). |
| I9 span-merging, UTF-8-safe redaction | **Deliberately dropped** — same module doc: no consumer, and an unused API recreates §7.4's three dead entry points. Sound. |
| §6.2 sensitive TTL | **⚠ Silently missing** (finding 3) — the one part of manifest 07 that is neither ported nor recorded. |
| §7.5 the telemetry scrubber as a second regex engine | **Deliberately dropped** — module doc, and the manifest asked for it. |

### 2.9 Cross-cutting (no manifest owns these)

| Capability | Verdict |
|---|---|
| Telemetry crate | **Deliberately dropped** — README; v1's own docs said it was never wired to a caller. |
| Benchmarks (`copypaste-bench`, Criterion) | **⚠ Not found.** No benchmark crate, no perf regression guard. Low user impact; worth noting because manifest 06 §5.4 carries budget numbers nothing now measures. |
| Fuzz targets (5 in v1: AEAD decrypt, image decode, IPC parse, snapshot parse, sync event decode) | **⚠ Not found.** Three of the five parse boundaries still exist in v2 (AEAD, IPC line, sync frames) and are unfuzzed. There *are* good hand-written hostile-input tests (`junk_base64_decodes_to_malformed_rather_than_panicking`, `an_oversized_frame_is_refused_without_parsing`, `truncated_ciphertext_fails_without_panicking`), which covers much of the value. |
| CI: 15 workflows incl. acceptance, coverage, fuzz-smoke, SBOM, audit, visual regression | **Landed, reduced** — `.github/workflows/` now carries CI (Linux + `macos-14`), supply-chain (`cargo deny`, `cargo audit`) and release. Coverage, fuzz-smoke and visual regression are not among them; fuzzing is B-28. |
| Homebrew cask / release packaging | **Deliberately dropped for now** — README + ADR-0001. **Landed since** — `scripts/release/`, `Casks/`, and an Android APK step (ADR-0006). Never run on a Mac or against an SDK. |
| Localisation (v1 shipped `values-uk`) | **Closed** — `crates/copypaste-ui/src/i18n/`, one catalogue behind i18next with `catalogue.test.ts` over it. English is the only locale shipped, but no user-facing string is hard-coded in a component any more, which is what a second locale needs. |
| Android app | **Landed, bounded surface** — `backend/embedded/mod.rs` is the in-process backend; the capture ladder, Keystore backend, shared ingest/import/export paths and release APK build are present. The intentionally unavailable operations are persistent service configuration, backup/restore, pinned reorder and change-stream watch; Android hides Service and Storage controls for the unsupported persistence/backup operations. |
| UniFFI 55-function surface | **Solved differently** — ADR-0002 replaces FFI-to-Kotlin with one Tauri app embedding the core. |
| Windows | **Deliberately dropped** — CLAUDE.md rule 7 (and v1 had frozen it already). |
| Linux desktop | **Deliberately dropped** — CLAUDE.md rule 7. Note the daemon still runs on Linux for development. |

---

## 3. Acceptance-test coverage

### 3.1 The population

| Manifest | Acceptance tests | Form |
|---|---|---|
| 01 clipboard-capture | 84 | `T-1`…`T-84`, given/when/then |
| 02 crypto | 18 | tables, 5 groups |
| 03 storage | 92 | tables, `M1-16`, `S1-8`, `E1-9`, `P1-8`, `D1-9`, `T1-7`, `X1-7`, `Q1-11`, `K1-8` |
| 04 ipc-protocol | 50 | numbered list, 5 groups |
| 05 sync-and-backend | 62 | tables, `AT-1`…`AT-56` |
| 06 ui-behaviour | 73 | `AT-1`…`AT-73`, given/when/then |
| 07 secret-detection | 129 | fixture tables (input → expected) |
| **Total** | **508** | |

### 3.2 Sampling method

Two strata, both drawn from the population above. **I did not compare 508 to
523.** Those numbers measure different things and the comparison would be
meaningless.

**Stratum A — census, not sample.** Every acceptance test whose own text cites a
recovered `CopyPaste-*` bug ID. This is a complete enumeration of that
sub-population, not a sample of it: **n = 29** (01: 9, 02: 1, 03: 1, 04: 0,
05: 11, 06: 0, 07: 7). Manifests 04 and 06 attach their bug IDs in separate
ledgers (04 §8, 06 Appendix A) rather than inside the test text; those are
handled in §4 instead.

**Stratum B — systematic sample of the remainder.** Every 11th acceptance test
in document order within each manifest, giving **n = 44** proportional to
manifest size (01: 8, 02: 2, 03: 8, 04: 5, 05: 5, 06: 7, 07: 9).

**Total sample: 73 of 508 = 14.4 %.**

**Verification procedure per sampled test.** (a) Locate the v2 module that owns
the behaviour. (b) Search its test module for an assertion with the same
*meaning* — not the same name; v2 renamed everything to sentences. (c) If none,
search the whole workspace for the behaviour's identifiers. (d) Classify. A test
is only "equivalent present" if I read the v2 assertion.

**Limits of the procedure.** It is a static read. It cannot tell a test that
asserts the right thing from one that asserts it vacuously, and I did not run
the suite. Where the behaviour does not exist in v2 at all, "no equivalent" is a
statement about the capability, not about test discipline.

### 3.3 Results

| Outcome | Stratum A (n=29) | Stratum B (n=44) | Total (n=73) |
|---|---|---|---|
| Equivalent assertion found | 14 (48 %) | 19 (43 %) | **33 (45 %)** |
| N/A — capability deliberately dropped, so the test is moot | 6 (21 %) | 12 (27 %) | **18 (25 %)** |
| Behaviour present in v2, **no test** | 4 (14 %) | 6 (14 %) | **10 (14 %)** |
| **Behaviour absent and undeclared** | 5 (17 %) | 7 (16 %) | **12 (16 %)** |

Read as: of the 55 sampled tests that are still *in scope* after deliberate
drops, **60 % have an equivalent assertion**, 18 % describe behaviour that
exists untested, and 22 % describe behaviour that is gone.

### 3.4 Stratum A in full (the bug-attached census)

| Test | Bug | v2 equivalent | Outcome |
|---|---|---|---|
| 01 T-11 third-party write during ours | `8yzf` | `third_party_write_during_ours_is_still_captured` | ✅ found |
| 01 T-22 binary-plist filenames parse | `q5ab` | — | N/A (no file capture) |
| 01 T-28 stat+read atomic under the cap | `b5iz` | — | N/A (no file capture) |
| 01 T-32 oversized image never copied to the heap | `1f5c` | `clipboard/macos.rs:172-184` implements the length-before-`to_vec` check for text | present, untested (macOS-only) |
| 01 T-47 classification independent of the exclusion list | `44rq.43` | — | ❌ absent (no app signal at all) |
| 01 T-54 frontmost staleness bound ≤ 2 ticks | `8ebg.57` | — | N/A (declared dropped) |
| 01 T-62 identical images converge and bump | `8ebg.57` | — | N/A (no images) |
| 01 T-73 no sensitive rows ⇒ no scan | `98ja` | — | ❌ absent (no TTL sweep) |
| 01 T-77 poll-interval hot reload | `at2m` | — | ❌ absent (no config) |
| 02 §3.2.2 length-prefixed HKDF inputs | `lkmy` | `hkdf_info_strings_are_the_documented_ones`, `derivation_diverges_when_either_input_changes` | ✅ found (property, not bytes — correct under rule 3) |
| 03 T6 newer pin beats older recopy | `ojhe` | `equal_timestamp_and_hash_break_on_delete_then_origin` covers the delete half | partial → counted ❌ (pin does not participate; finding 8) |
| 05 AT-9 comparator equivalence over the decision space | `ayvs` | `summary_comparator_agrees_with_the_full_one`, `merge_is_symmetric_across_the_whole_decision_space` | ✅ found |
| 05 AT-11 unified lamport space (pin/delete outranks recopy) | `ojhe` | `merge.rs` module doc reasons it; `an_older_live_version_cannot_resurrect_a_newer_tombstone` | ✅ found (delete half) |
| 05 AT-16 upsert does not resurrect; `deleted` always explicit | `kgs7` | `an_upsert_body_is_an_array_that_always_states_deleted` | ✅ found |
| 05 AT-17 hostile timestamps clamped at deserialization | `psx7` | `negative_timestamps_are_clamped_on_decode`, `…_inside_items_too`, `a_negative_stamp_is_clamped_at_the_boundary` | ✅ found (three, incl. the nested case) |
| 05 AT-20 delete-before-create, per transport | `bfiu` | `a_delete_for_an_item_the_peer_never_saw_is_stored` (p2p), `a_tombstone_reaches_the_store_even_for_an_unknown_item` (meta) | ✅ found |
| 05 AT-33 mutations survive an outage | `1t38` | — | ❌ absent |
| 05 AT-54 undecryptable item skipped, not stored | `jww`/`5y4` | `an_undecryptable_row_is_skipped_and_never_deletes_the_local_copy` | ✅ found |
| 05 AT-55 sensitive re-detection on receive | `kcf` | `an_incoming_secret_is_flagged_by_this_devices_detector` | ✅ found |
| 05 AT-56 sensitive items never leave the device | `20yw` | 4 tests across p2p + cloud | ✅ found |
| 07 §9.1 `access_token` / `refresh_token` in `generic_password_kv` | `2eet` | rule 23's pattern carries them; `value_strength_follows_the_manifest_criteria` | ✅ found |
| 07 §9.1 `ip_with_port` must not auto-wipe | `8ys1` | `no_rule_sits_exactly_on_the_floor`, `fp_risk_patterns_below_autowipe_floor` | ✅ found |
| 07 §9.2 password-manager app fixtures | `44rq.43` | — | ❌ absent |
| 07 §9.4 app signal independent of every other list | `44rq.62` | — | N/A (declared dropped) |
| 07 §9.4 app-list matching is substring, case-insensitive | `44rq.64` | — | N/A (declared dropped) |
| 07 §9.6 promote-on-copy recomputes `expires_at` | `8ebg.2` | — | ❌ absent (finding 3) |
| 07 §9.3 rule-table structural invariants | `3e7y` | `rule_count_parity`, `rule_names_are_unique`, `ruleset_compiles` | ✅ found |
| 01 T-3 cursor advances on every drop path | (I-3) | `cursor_advances_after_the_delta_is_computed`, and `oversized_content_is_rejected_and_counted` asserts the re-offer case | ✅ found (for the paths that exist) |
| 03 S3 stale FTS row never surfaces | (ADR-015) | `sensitive_items_never_reach_the_search_index`, `search_never_returns_a_sensitive_item` | ✅ found |

### 3.5 Stratum B — notable results

Rather than list all 44, the ones that changed my view:

**Found, and better than v1's version:**
`merge_is_symmetric_across_the_whole_decision_space` (property over the full
space, where v1 asserted points); `a_wrong_shape_is_reported_rather_than_silently_defaulted`
(three copies — the exact failure mode of v1's 128 untyped `.as_*()` calls);
`no_username_survives_a_realistic_socket_error`; `zwj_bypass_is_documented_and_still_open`.

**Found, weaker than v1's version:**
`database_is_encrypted_on_disk_and_in_wal_mode` folds E7/E8/P1 into one test and
skips the `-wal`/`-shm` files v1 checked separately. Pool coverage is three
tests where v1 had eight plus a stress test.

**Behaviour present, no test (the 10):** scroll anchoring (INV-1), the shrink
clamp (INV-6), reveal auto-hide (INV-11), `role="listitem"` (INV-8), the a11y
announcer (INV-9), the `org.nspasteboard.*` probe, the hoisted UTI constants,
the autorelease pool, the length-before-copy image gate, and window-hide-on-close
(INV-36). Note the shape: **every one is either in the untestable macOS binding
or in the untested UI.** That is a coverage boundary, not scattered neglect.

**Absent and undeclared (the 12):** frontmost/app-signal tests ×2, sensitive TTL
tests ×3, hot-reload ×1, pin LWW ×1, keyset pagination (03 Q11 — which the
manifest itself flags as "add this") ×1, outage-survival ×1, backlog sweep ×1,
reduced-motion/contrast ×1, screen-capture protection ×1.

---

## 4. Bug-regression risk

The manifests record **217 unique recovered bug IDs**. Every one is
re-introducible in a clean-slate rewrite. Of those, **30 are cited somewhere in
v2's tree** (source comments or tests). Citation is not the only evidence — many
v2 tests encode a rule without naming its bug — so the analysis below is by
behaviour, not by grep count.

Ranked by severity in the order CLAUDE.md rule 4 sets: **data loss, then
security, then correctness, then cosmetics.**

### 4.1 Data loss

| Rank | Bug / rule | Would v2 catch a reintroduction? | Notes |
|---|---|---|---|
| **D1** | `CopyPaste-ah1m` — TOCTOU on stale-socket bind; two daemons on one database | **No.** `a_stale_socket_file_is_replaced` covers the single-starter path only. | The `flock` guard is gone (finding 12). Two daemons writing one SQLCipher file is the worst shape in the tree. |
| **D2** | Burst handling must not advance the cursor without capturing | **Yes** — `burst_does_not_eat_the_survivor`, `burst_threshold_boundary`, `first_observation_is_not_a_burst`. | One of the three rules the port-manifest README singles out. Well guarded. |
| **D3** | `CopyPaste-bfiu` — delete-before-create resurrects an item | **Yes** — two tests, in both the p2p session and the meta writer. | |
| **D4** | `CopyPaste-kgs7` — omitting `deleted` on upsert resurrects a tombstone | **Yes** — `an_upsert_body_is_an_array_that_always_states_deleted`. | |
| **D5** | `CopyPaste-psx7` — negative timestamp becomes `u64::MAX` and wins forever | **Yes** — three tests, clamped at the decode boundary as R-CLK-1 requires. | |
| **D6** | `CopyPaste-8ebg.57` — OFFSET pagination duplicates or skips rows | **No.** No keyset predicate, no test. Manifest 03 Q11 explicitly asks for this test and it does not exist. | Finding 10. Currently masked because the UI fetches only page 1 — it becomes live the moment load-more lands. |
| **D7** | `CopyPaste-1uqb` — eviction ordered on a client-supplied clock lets an attacker displace items | **N/A at `HEAD`** (no server retention). **[in flight]** — the new migration orders on `inserted_at` and says why. | |
| **D8** | `CopyPaste-1t38` — mutations lost to broadcast-ring overflow during an outage | **No.** No equivalent mechanism and no test. | LWW limits the blast radius; still unguarded. |
| **D9** | AT-24 — ≥ limit rows sharing one `wall_time` (the manifest calls this "the worst silent-data-loss bug in the codebase") | **Yes** — `fetch_since_asks_for_an_inclusive_bound_and_a_total_order`, `pull_drains_more_rows_than_one_page`, `a_page_is_put_into_cursor_order_before_it_is_applied`. | The compound keyset is present on the cloud side. Note the asymmetry with D6: cloud has it, local does not. |
| **D10** | AT-29 — a full page yielding no cursor progress spins forever | **Yes** — `no_recovery_path_can_spin`, `the_watermark_only_moves_forward`. | |
| **D11** | `CopyPaste-00zz` — one bad row surfacing ~629 errors, or blanking a page | **Partly.** `decrypt_rows` skips and logs; no test asserts the page survives, and the skip count is not returned (finding 17). | |
| **D12** | Sensitive auto-wipe with the `0` sentinel — misreading `0` as "expire now" deletes every sensitive item immediately | **N/A** — the feature is gone (finding 3). If it is built later, this is the first test to write. | |

### 4.2 Security

| Rank | Bug / rule | Would v2 catch a reintroduction? | Notes |
|---|---|---|---|
| **S1** | ADR-015 / `CopyPaste-i6pp` — plaintext passwords in the FTS index | **Yes, three times.** Write guard, in-transaction re-read, read predicate; each with a test. | The best-defended rule in the tree. |
| **S2** | Frontmost-app lookup skipped when the exclusion list is empty (`CopyPaste-44rq.43`) — password-manager content never flagged | **No.** The signal does not exist. | The port-manifest README lists this as one of three load-bearing rules. It is the one that did not survive. Declared, but as "attribution" (§1.3). |
| **S3** | `CopyPaste-8ys1` — a rule sitting exactly on the auto-wipe floor silently deleted RFC1918 addresses | **Yes** — `no_rule_sits_exactly_on_the_floor` asserts strict inequality for every rule. | Better than v1: v1 fixed one rule, v2 pins the invariant. |
| **S4** | Socket path in a user-facing error discloses the local username | **Yes, repeatedly** — 6 `error_messages_contain_no_paths` tests, `no_username_survives_a_realistic_socket_error`, a frontend test, and a shared `scrub_paths` with 6 tests of its own. | |
| **S5** | Fail-closed on crypto — wrong key, wrong AAD, wrong version must never fall back | **Yes** — `wrong_item_id_fails_closed`, `a_version_mismatch_fails_closed_on_both_halves`, `tampered_ciphertext_fails_closed_in_every_byte`, `a_wrong_key_fails_closed_rather_than_reading`. | |
| **S6** | `CopyPaste-crh3.89`/`crh3.12` — registration oracle via non-constant-time PoP compare | **Solved differently** — GoTrue replaces PoP. `ct_eq_agrees_with_byte_equality` and `psk_comparison_is_available_and_correct` keep constant-time comparison where secrets remain. | |
| **S7** | Pairing credential with no TTL and no single-use burn | **No.** Nothing ages or consumes a pairing code (finding 14). | v1 had `QR_PAIRING_TTL_SECS` and INV-28. |
| **S8** | `CopyPaste-6ot5` / `cce1` / `c4q2.24` — same-UID client pins a connection permit and the DB mutex | **No.** No cap, no timeouts (finding 13). | Same-UID only; the socket is `0600`. |
| **S9** | `CopyPaste-5lm` — `list_peers` leaking `password_file_enc` | **Yes by construction** — `PeerInfo` has five non-secret fields; `peer_debug_shows_no_key_material`, `advertisement_has_no_secret_material`, `the_serialised_file_holds_the_psk_as_hex_and_nothing_else_secret`. | |
| **S10** | INV-13 — the pairing QR payload string entering the DOM | **N/A** — no QR, no pairing UI. Re-read this before building finding 1. | |
| **S11** | INV-35 — screen-capture protection off | **No.** Not configured; no test. | The history window is capturable by any screen recorder. |
| **S12** | Forged cloud metadata outranking a real version (manifest 05 §5.3) | **No** — signed LWW metadata is declared not built, and the threat-model note is not written down. | |
| **S13** | `CopyPaste-kcf` — a synced password bypassing the receiver's own detection | **Yes** — `an_incoming_secret_is_flagged_by_this_devices_detector`. | |

### 4.3 Correctness

| Rank | Bug / rule | Would v2 catch a reintroduction? |
|---|---|---|
| **C1** | `CopyPaste-8yzf` — post-stamping the sentinel unconditionally suppresses a third party's copy | **Yes** — `third_party_write_during_ours_is_still_captured`, `failed_write_clears_the_sentinel`. |
| **C2** | `CopyPaste-ayvs` — two comparators for one total order disagree, peers never converge | **Yes** — `summary_comparator_agrees_with_the_full_one`; v2 also structurally prevents it by placing `deleted` above `origin_device_id` so the summary view is sufficient. Reasoning is written down. |
| **C3** | `CopyPaste-ojhe` — a delete tying its own live version resurrects on device-id order | **Yes for delete** (`equal_timestamp_and_hash_break_on_delete_then_origin`); **no for pin**, which no longer participates (finding 8). |
| **C4** | §7.2 — `detect()` returning the lowest index rather than the highest confidence | **Yes** — `scan_ranks_by_confidence_not_declaration_order`. |
| **C5** | Audit MED #6 — a credit card embedded in ordinary text is a silent miss | **Yes** — `"Customer card: 4111 1111 1111 1111 — expires 12/26"` is a fixture. |
| **C6** | Audit MED #5 — `-----BEGIN ENCRYPTED PRIVATE KEY-----` and PuTTY `.ppk` missed | **Yes** — one broadened `private_key` rule plus `putty_private_key`, both commented with the miss. |
| **C7** | `openai_legacy` double-firing on `sk-proj-` keys | **Yes** — `openai_legacy_does_not_double_fire_on_proj_keys`. |
| **C8** | FTS5 `-bar` read as a column filter → "no such column" | **Yes** — `fts5_query_sanitizer`, with each rule commented as the bug it came from. |
| **C9** | INV-5 — the "smarter" character-count row-height estimate (the estimate *was* the bug) | **Yes** — `does not vary with content length`. Third of the three README-highlighted rules. |
| **C10** | INV-1 / INV-6 — scroll anchoring and the shrink clamp | **No test.** Implemented in `useScrollAnchor.ts`; nothing would catch its removal. |
| **C11** | INV-2 / INV-3 / INV-33 — identity churn, stale dedup signature, late responses clobbering | **No test.** Delegated to React Query, which is the right call; the *delegation* is untested. |
| **C12** | `CopyPaste-crh3.10` / `ro0r` — client retry policy | **Yes for `not_ready`**; `migration_in_progress` has no producer and correctly does not exist. |
| **C13** | I-23 / T-36-39 — dedup must find and bump an old row | **No** — the behaviour is gone (finding 4). |
| **C14** | AT-32 "BUG C2" — history captured before a passphrase is set never uploads | **No test.** |

### 4.4 Cosmetics and diagnostics

| Bug / rule | Caught? |
|---|---|
| `CopyPaste-8ebg.38` — toasts rendering on top of each other | N/A — no toasts |
| `CopyPaste-8ebg.55` — auto-dismiss not pausing on hover | N/A |
| `CopyPaste-7w060.2` — toast stack colliding with the sidebar footer | N/A |
| `CopyPaste-8ebg.37` — "Nothing copied yet" flashing on every popup open | N/A — no popup |
| `CopyPaste-8ebg.53` — screen readers announcing `⌘⇧V` instead of `CmdOrCtrl+Shift+V` | N/A — no shortcut control |
| `CopyPaste-wrfn` — a live region inside `role="list"` failing `aria-required-children` | **Yes by construction** — the announcer is a sibling |
| `CopyPaste-93yr` — export silently dropping non-text items | N/A — no export |
| Relative-time rendering, plural forms, truncation on char boundaries | **Yes** — `relative_time_buckets`, `plurals_read_correctly`, `truncation_lands_on_a_char_boundary`, `one_line_truncates_on_char_boundaries` |

### 4.5 Summary

Of the 217 recovered bugs, I assessed **47 in depth** (the 29 bug-attached
acceptance tests plus 18 drawn from the 04 §8 and 06 Appendix A ledgers).

| | Count |
|---|---|
| Guarded by an equivalent v2 test | 22 |
| Moot — the surrounding capability was deliberately dropped | 12 |
| **Re-introducible with nothing to catch it** | **13** |

The 13, in severity order: **D1** (socket TOCTOU), **D6** (OFFSET pagination),
**D8** (mutations during an outage), **D11** (undecryptable-row count),
**S2** (app-origin sensitivity), **S7** (pairing-code lifetime), **S8**
(connection cap / timeouts), **S11** (screen-capture protection), **S12**
(forged cloud metadata), **C3-pin** (pin does not sync), **C10** (scroll
anchoring untested), **C11** (React Query delegation untested), **C13** (dedup
bump).

Note the pattern: **eleven of the thirteen sit in exactly two places** — the
three untested boundaries (macOS binding, UI, IPC transport) and the capabilities
that were dropped without a record. Neither is a discipline problem in the code
that exists; the tested Rust core is well guarded.

---

## 5. Where v2 solved the problem differently

Stated explicitly so none of it is miscounted as loss.

| v1 | v2 | Sanctioned by |
|---|---|---|
| 6 retry/backoff implementations + an unused `BackoffScheduler` | one `cadence`/`retry` path in `copypaste-cloud` | CLAUDE.md rule 1 |
| 3 rate limiters | none yet, and no dependency pretending otherwise | README |
| 3 models of the wire contract, CLI importing none | one `Method` enum, both dispatchers exhaustive over it | manifest 04 §10.1 |
| 4 Lamport-ordering implementations + a `LamportClock` with no production caller | one comparator on `created_at → content_hash → deleted → origin_device_id`, plus a future-stamp ceiling | port-manifest README; manifest 05 §7.2/§7.3 |
| structural CRDT would be the "proper" answer | LWW over metadata, because items are opaque ciphertext at rest | CLAUDE.md rule 1 exemption 2, stated in `merge.rs`'s module doc |
| 2 hand-written ASN.1/DER parsers, rustls + rcgen + a pinning verifier + OPAQUE | Noise `NNpsk0` over TCP | manifest 02 §6.3 |
| ~12k-line Axum relay with its own DB, quota, TTL, PoP, metrics | Supabase + RLS, upsert-on-`item_id` | manifest 05 §5.4 |
| `CHUNK_FORMAT_V1` bespoke framing | N/A while text-only; `aead::stream` when needed | manifest 02 §6.2 |
| hand-rolled `user_version` migration runner | `rusqlite_migration` | manifest 03 §6.1 |
| hand-rolled byte-scanning frame codec | `tokio_util::codec::LinesCodec` | manifest 04 §10.3 |
| ~1,000 lines of per-screen polling, in-flight guards, sequence tags, signature hashes | React Query (`structuralSharing`, `invalidateQueries`, visibility gating, last-write-wins) | manifest 06 §9.1 |
| 2 regex secret engines (detector + telemetry scrubber) | one, gitleaks-sourced | manifest 07 §7.5 |
| 2 Luhn implementations | one | manifest 07 §7.3 |
| credit card as an off-table special case with no spans | a first-class rule | manifest 07 §3.3, which demanded exactly this |
| `detect()` returning the lowest declaration index | highest confidence, ties on longer match | manifest 07 §7.2 |
| 38-field daemon context with 13 `Arc<Mutex<Option<T>>>` slots | one `AppState`, every field always present | — |
| UniFFI (55 functions) + a separate Compose app | one Tauri app, core embedded in-process on Android | ADR-0002 |
| a non-macOS no-op clipboard | a drivable fake, so the pipeline is demonstrable and testable off a mac | — |

---

## 6. Recommendations

Not asked for, but the ranking above implies an order. Grouped by what each buys.

**Stops a data-loss or security shape (do these first):**

1. Restore the `flock` guard around probe→remove→bind (**D1**, finding 12). Small,
   local, and the failure mode is two daemons on one database.
2. Decide the sensitive-TTL question (**finding 3**) — build it, or state in
   `SECURITY.md` that a detected secret is retained indefinitely. Silence is the
   one option that is wrong, because the detector's severity band currently
   implies an action that never happens.
3. Turn on content protection for the history window (**S11**), and add a
   pairing-code lifetime (**S7**).
4. Write down the cloud threat-model change (manifest 05 §5.4 obligation 3)
   before the cloud wiring lands.

**Closes a manifest conflict:**

5. Resolve the pin-LWW divergence (**finding 8**) — either make pin travel, or
   amend manifest 05 §3.6 and say why, which is what CLAUDE.md rule 2 asks.

**Makes the README true:**

6. Add the nine missing entries to "Not built", restate frontmost-app as a
   sensitivity signal rather than attribution, and resolve the
   product-surface/interim-window contradiction.

**Buys back the most user-visible capability per unit of work:**

7. The pairing/peers UI (**finding 1**) — the feature is built, tested, and
   unreachable. This is CLAUDE.md rule 6's own example. **[in flight]**
8. Daemon lifecycle ownership (**finding 2**) — v1 wrote an ADR about why this
   matters; the reasoning still applies.
9. Re-copy bumps to the top (**finding 4**) — a few lines in `capture.rs`, and
   the difference between a clipboard manager and a log.
10. Export (**finding 5**) — the cheapest half of findings 5 and 6, and the one
    that makes "no upgrade path" survivable next time.

**Before the UI grows:**

11. Keyset pagination (**finding 10, D6**) — cheaper to add now than after
    load-more ships against OFFSET. Manifest 03 Q11 already specifies the test.
12. Tests for `useScrollAnchor` (**C10**) and for the React Query delegation
    (**C11**). The delegation is the right design; the invariants it satisfies
    are still ours to prove.

---

## 7. What this audit does not establish

* That any v2 test passes. Nothing was run.
* That the macOS or Android paths work. Neither was built.
* That the 33 "equivalent assertion found" verdicts are strong assertions rather
  than weak ones. I read each one, but a static read cannot measure rigour.
* That the 12 "absent and undeclared" sampled tests are the only such tests. The
  sample is 14.4 % and the estimate that follows from it — roughly 60-70 further
  undeclared absences across the full 508 — is an extrapolation, not a count.
* Anything about `copypaste-cloud`'s `realtime/` module beyond its tests' names
  and its public shape.
* Anything about the working tree. This is `c53be35b`.
