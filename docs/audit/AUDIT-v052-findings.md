# v0.5.2 Full Compliance Audit — Findings & Triage

Base: HEAD 7450053 (post merge-train, pre restyle-merge). 12 read-only auditors + cross-cutting sweep. Status: collecting (3 of 12 reported).

Legend: 🔴 BLOCKER (fix before release) · 🟠 fix-wave (this release) · 🟡 defer (v0.6/v0.5.3) · ✅ confirmed-clean

---

## CRYPTO (crates/copypaste-core) — done
- 🟠 HIGH db.rs:162-170/469/487 — SQLCipher key rendered as plain hex `String`, never zeroized (heap-dump leak). Fix: `Zeroizing<String>` on all 3 hex bufs.
- 🟠 HIGH chunks.rs:84 — `encrypt_chunks` panics on `chunk_size==0` (pub fn, `slice.chunks(0)`). Fix: guard → Err.
- 🟡 MED chunks.rs:86,129 — `len() as u32` truncation >4B chunks (not reachable today). try_from guard.
- 🟠 MED sync_key.rs:208 — `derive_sync_key` accepts 0/1-char passphrase → trivial brute-force. Fix: min length 8, surface error.
- 🟡 MED keys.rs:115,224 — `derive_v2`/`derive_enc_key` return raw `[u8;32]` not `Zeroizing` (precedent exists). Wrap.
- 🟡 LOW sync_key.rs:114 cloud AAD no key-epoch; keys.rs:60 pair_id in salt+info (doc it); pairing_qr.rs:224 raw field access; encrypt.rs deprecated empty-AAD still pub.
- ✅ nonces fresh OsRng everywhere; AAD item_id|schema|key_version enforced; Argon2id m=19456/t=2/p=1; subtle CT-eq on tokens; ZeroizeOnDrop on keypair/synckey/token; UnknownKeyVersion no-panic.

## ANDROID KOTLIN — done
- 🔴 CRIT ClipboardRepository.kt:183-203 `deleteItems()` — no `onItemsDeleted()` → notif counter over-reports (USER-REPORTED BUG). Fix: call onItemsDeleted(ctx, n).
- 🔴 CRIT ClipboardRepository.kt:230-249 `clearUnpinned()` — same, no decrement.
- 🔴 CRIT ClipboardRepository.kt:209-224 `clearAll()` — same, counter stuck.
- 🔴 CRIT ClipboardRepository.kt:488-523 `pruneToLimits()` — eviction inflates counter, no decrement.
  → ALL FOUR = the user's "каунтер в notification не скидається" — single fix-wave task (route all delete/evict paths through onItemsDeleted).
- 🟠 HIGH ClipboardRepository.kt:562 `plaintextLen` stores char-count not bytes (non-ASCII wrong). Pass plaintextBytes.size.
- 🟠 HIGH ClipboardRepository.kt:309 dedup state not reset on Settings.clear() → first re-copy dropped after clear. resetDedupState().
- 🟠 HIGH MainActivity.kt:154 `handleClipboardChange` skips counter/sound/notif/sync — foreground-captured items uncounted+unsynced. Route via ClipboardService.captureClip.
- 🟠 HIGH SettingsActivity.kt:555 SettingsNumberField commits on EVERY keystroke (1→15→150 MB coercion, cursor jump). Commit on focus-lost/IME. (NOTE: design v2 replaces these with stepped sliders → moot, but fix or replace.)
- 🟠 HIGH Settings.kt:208 deviceId getter TOCTOU race → two UUIDs at first boot. synchronized.
- 🟠 HIGH LogcatCaptureService.kt:108 Handler.post after scope.cancel → coroutine on cancelled scope. Guard isActive.
- 🟠 MED ClipboardRepository.kt:556 local lamportTs=0 → cloud row always wins LWW, local re-capture overwritten. Store currentTimeMillis.
- 🟡 MED FgsSyncLoop dial-after-delay; SyncManager concurrent sign-in; ClipboardService lastCopyNotifMs debounce broken (use AtomicLong); storeItemWithLww encrypt-outside-lock ghost blob; SupabaseClient errorStream null crash; OnboardingActivity battery remember(ctx) stale; recordSourceId apply() vs commit().
- 🟡 LOW strings.xml:57 STALE hint "default 1/25/500" (real 15/64/10240) — USER-FLAGGED stale strings; HistoryActivity:600 onCopy copies 140-char SNIPPET not full plaintext (real bug — truncated copy!); NotificationHelper dup id 1001; LogcatCapture START_STICKY non-FGS; logcatCaptureWorking not reset on disable.
  → reclassify HistoryActivity:600 (copies truncated snippet) as 🟠 HIGH — that's data loss on copy.

## CLI / IPC / TELEMETRY / ANDROID-FFI — done
- 🔴 CRIT cli/commands/cloud.rs:102 — Supabase password stored PLAINTEXT in config.json (should be Keychain on macOS). Fix: Keychain + zeroize CLI string.
- 🔴 CRIT android/lib.rs:303 — `runtime()` `.expect()` panics across FFI if tokio build fails, permanent unrecoverable. Return Result.
- 🟠 HIGH cli/ipc.rs:100 Response.id String vs shared u64 → lost correlation id (arch-2). Align or doc.
- 🟠 HIGH cli/cloud.rs:68 password prompt echoes cleartext (no rpassword). 
- 🟠 HIGH cli/paths.rs:88 set_var in parallel tests unsound (1.80 deprecated). serial/Mutex.
- 🟠 HIGH android/lib.rs:683 legacy-frame warn via eprintln → logcat black hole (invisible diag).
- 🟠 HIGH cli/backup.rs:111 backup_path forwarded as arg unvalidated (--flag injection to script). Validate is-path.
- 🟡 MED ipc id u64 vs String deser break risk; cli/watch.rs:61 no backoff/forever; telemetry init() ignores consent → silent Noop; android with_cached_db poison partial map; cli/status.rs:164 id="status" inconsistent; telemetry IPv6 regex over-match; android lib.rs:537 fresh device_id per sync (origin_device_id unstable).
- 🟡 LOW export.rs umask 0644; backup current_exe symlink; android decrypt-fail silent continue (count it).
- ✅ ipc wire types tested; telemetry Disabled short-circuits; FFI panic_boundary comprehensive; ABI check hard-reject; cli socket timeout+max-bytes; exit_on_err propagation.

---

## P2P / SYNC — done (0 CRIT)
- 🟡 HIGH sync/engine.rs:195+ — HAVE/WANT keyed on row-UUID `id` not stable `item_id` → never converges (union not replace). = CRDT-identity-plan (already approved separate work).
- 🟠 HIGH sync/engine.rs:148 run_session no wall-clock timeout; daemon/p2p.rs:708 run_peer_connection_framed no idle read-timeout → leaked tasks/stale sinks.
- 🟡 MED HAVE sends ALL items (no delta/cursor); contains() O(N²); discovery.rs:241 byte-slice device_id panic risk; bootstrap binds 0.0.0.0.
- ✅ LWW merge tie-break correct+tested; lamport clock; PAKE/OPAQUE; TLS channel-binding (relay MitM test); cert-pinning CT; frame bounds 16MiB; mDNS rate-limit per-device; dial backoff.

## CLOUD + SUPABASE — done (0 CRIT)
- 🟠 HIGH cloud.rs:657/1277 — `is_synced` never set to 1 after push → every restart re-pushes ALL history (409-flood, wastes budget). Fix: UPDATE is_synced=1 after push.
- 🟠 HIGH cloud.rs:1277 — wall-time-ONLY cursor loses items sharing same wall_time in burst (same-ms multi-device) — silently skipped forever. Fix: keyset (wall_time,id). [matches dual-sync burst-loss root cause — verify the merged fix actually covers daemon cloud path]
- 🟠 HIGH realtime.rs:384 empty join payload (no JWT → RLS unauth, no events); :304 backoff never reset after long session; cloud.rs:491 Notify misses shutdown if parked elsewhere (use CancellationToken).
- 🟠 MED cloud.rs:1693 downloaded items hardcode is_sensitive=false → cross-device sensitive items skip auto-wipe TTL. Run detect on download.
- 🟡 MED content_type silent "text" fallback; key_arr manual-zero (use zeroize); poll limit=20/10s magic; auth auto-refresh infinite loop (latent).
- ✅ HTTPS-only; fail-closed auth; CloudHandle Drop shutdown; 401 single-refresh guard; retry-queue cap; RunningGuard RAII; PII redact in logs.

## UI REACT/TS — done (0 CRIT)
- 🟠 HIGH Popup.tsx:42-61 double IPC load on every popup show (mount + focus both refresh). Guard with hasMounted.
- 🟠 HIGH event_tap.rs:219 `.expect()` on bg thread → tap dies silently, TapActive stays true → global shortcut permanently dead, no UI feedback. Explicit error + fallback.
- 🟠 HIGH SettingsView.tsx:251 private-mode toggle desync with tray (loaded once; tray toggles independently → stale Save reverts tray). Poll/event re-read.
- 🟠 MED AboutView.tsx:35 version HARDCODED "0.4.1" (real 0.5.2) — always wrong. Use app_version cmd. [my tail-polish agent rewrote AboutView — verify version source after restyle merge]
- 🟡 MED HistoryView interval double-load on loadState change; 6 SettingsView timers + DevicesView timer not cleared on unmount; execCommand copy fake-success; ImageThumbnail no unmount guard.
- 🟡 LOW lib.rs:680 tray 2× blocking IPC on main thread (20s startup hang if slow daemon); do_call no write/connect timeout; ipc.ts unchecked cast.
- ✅ masking.ts unicode; store.ts; ErrorBoundary; HistoryView kbd nav + error toasts; DevicesView QR/revoke surfaced errors.

## DAEMON IPC + KEYCHAIN — done (2 CRIT)
- 🔴 CRIT ipc.rs:100-135 — DUAL CONFIG-PATH SPLIT: set_config writes config.json (~/Library/Preferences), daemon reads config.toml (~/Library/Application Support). p2p_enabled WRITTEN BUT NEVER READ (daemon uses env only). = A-SET-1 "settings don't persist/apply". Fix: unify config path + wire fields to daemon read-path.
- 🔴 CRIT ipc.rs:2366/2461 — revoke_peer/revoke_all_peers don't remove peer from live PairedPeers allowlist (no remove method) → revoked peer's mTLS accepted until restart. Add PairedPeers::remove + call it.
- 🟠 HIGH ipc.rs:2013 get_config/set_config/get_sync_status blocking fs I/O on async worker (no spawn_blocking); :2275 pair_peer doesn't register_live_peer (manual pairing needs restart); :1532 stats sensitive_items counts only first 1000 rows (undercount). 
- 🟠 HIGH ipc.rs:2703/2973 PAKE SessionKey discarded — no TLS channel binding (TODO-S3, known; LAN MitM). [= same S3 gap; p2p auditor says bootstrap path HAS binding but ipc pairing path doesn't — reconcile]
- 🟡 MED dual load_peers (raw-json drops sync_key_b64/address from list_peers); save_peers no 0600 chmod (secret leak); keychain read_secret not Zeroizing; test_cloud email in error msg.
- 🟡 LOW p2p_enabled field; device_name HOSTNAME unreliable→always "CopyPaste"; write_config TOCTOU world-readable window; fingerprint case-sensitive dup; keychain Err(_) swallows non-notfound → orphans DB.
- ✅ stale-socket cleanup; MAX_REQUEST_BYTES guard; UUID validation; redact_config_secrets; PAKE TTL+cap; socket 0600; CancellationToken shutdown.

## CORE DETECTION + IMAGE — done (0 CRIT)
- 🟠 HIGH image.rs:206 encode_image no size check after re-encode → compressed input → PNG 3-4× bigger than budget → 150-200MB blob to SQLite. Check png_bytes.len() after encode.
- 🟠 HIGH (UX, data-loss) sensitive detector FALSE POSITIVES auto-wipe user data silently: phone_us (any 10-digit), passport (any LtrLtr+6-9digit = SKU/order-id), openai sk-* (any 48-char). detect() has NO confidence threshold → daemon.rs:1177 marks is_sensitive → 30s auto-wipe. Fix: confidence floor ≥0.70 for is_sensitive TTL gate; exclude phone/email/passport from auto-wipe.
- 🟡 MED image.rs:206 encode_image ignores user max_decoded_image_mb (uses compile default 50, not config); CARD_RUN_RE .expect() hot-path; sk- / vault / iban / aws patterns under-anchored (FP).
- ✅ decode-bomb Limits on all 3 paths; chunks_from_blob cap; thumbnail zero-guard; NFKC; no ReDoS (Rust regex NFA); config atomic save; clamp_values.

## DAEMON CLIPBOARD + LIFECYCLE — done (1 CRIT)
- 🔴 CRIT clipboard.rs:256-268 — SkippedBatch advances last_change_count to count BUT discards already-read text/image → next poll sees count==last → None → LATEST CLIPBOARD CONTENT PERMANENTLY LOST on big batches. Fix: return read content, or set last=count-1.
- 🟠 HIGH sync_orch.rs:234 — merge_incoming_with_crypto holds tokio Mutex db.lock().await across sync rusqlite I/O + shared_sync_key disk read → blocks executor unbounded per P2P batch. spawn_blocking.
- 🟠 HIGH ipc.rs:51/daemon.rs:310 — p2p_enabled dead field (env-only) [CONFIRMS daemon-ipc CRIT #1].
- 🟠 MED daemon.rs:1213-1229 — after dedup stored_id!=item.id, broadcasts item with REJECTED uuid → subscribers look up nonexistent row. Broadcast stored row or None.
- 🟠 MED sync_orch.rs:88 shared_sync_key reads peers.json per outbound item (cache it); daemon.rs:51 core::AppConfig loaded once never refreshed (= A-SET-1 hot-reload; daemon-ipc says Arc<RwLock> exists for SOME fields — reconcile which are live).
- 🟡 LOW degraded loop 250ms busy-poll; peers.rs:76 save_peers non-atomic (corrupt on crash → lose sync key); clamp missing sensitive_ttl_secs floor (=0 wipes all).
- NOTE: org.nspasteboard skip IS present (clipboard.rs:170) — auditor's "missing" claim is FALSE (stale base). Verify Concealed+AutoGenerated also covered (saw TransientType).
- ✅ no unwrap in prod; atomic orderings; signal wiring; prune pinned=0 guard.

## CORE STORAGE + CONFIG — done (0 CRIT)
- 🟠 HIGH items.rs:475 delete_sensitive_expired MISSING `AND pinned=0` (doc promises exclusion) → pinned sensitive item wiped by 30s sweep. Add guard.
- 🟠 HIGH items.rs:474 threshold underflow if ttl>now (saturating_sub); items.rs:60,91 SystemTime::now().unwrap() panics daemon if clock<epoch (unwrap_or_default).
- 🔴→🟠 HIGH daemon.rs:1342 prune_history deletes clipboard_items WITHOUT clipboard_fts → orphan FTS rows grow forever + ghost search; :841 run_ttl_cleanup same. Fix: delete FTS in same tx. [size-prune is the path the user cares about — must clean FTS]
- 🟠 MED sanitize_fts5_query leading-quote → unclosed phrase → SQL error to client; limit/offset as i64 lossy (negative=no-limit); key_version i64 as u8 truncates>255; purge_dead_v1 two non-tx DELETEs.
- 🟡 MED sqlite_cache_mb=8 config field DEAD (schema hardcodes 8MB, pool gets default 2MB); INLINE_THRESHOLD_BYTES dead const; cache_size pragma not in CONNECTION_PRAGMAS.
- 🟡 LOW clamp_values doesn't floor history_limit/sizes (=0 wipes/divzero); user_version unwrap_or(0) re-runs migrations on lock failure.
- ✅ migrations v1→v7 monotonic+tx-rollback; insert_item_with_fts atomic; pin/delete pinned-guard; rekey checkpoint+fsync crash-safe; clamp_preview UTF-8.

## RELAY — done (0 CRIT)
- 🟠 HIGH items.rs:68 pre-auth base64 decode ~13MB BEFORE verify_token → unauth memory DoS. Move auth before decode.
- 🟠 HIGH state.rs:464 timing side-channel device-id enumeration (unknown device no ct_eq vs known ct_eq). Dummy ct_eq on not-found.
- 🟠 HIGH ARCHITECTURE.md:59-60 STALE/WRONG security claims (token=SHA256(pubkey) — actually OsRng; fan-out to all inboxes — actually auth-gated). Doc fix (misleads integrators into insecure assumptions).
- 🟡 MED GET /devices unauth enumeration oracle (device names = PII); no global memory cap; no TLS (document reverse-proxy req); no replay/dedup; reg_attempts soft cap; README port drift 7777 vs 8080.
- ✅ ct_eq token; OsRng; body limit; inbox 500 cap; checked_add; TTL server-time; composite cursor; no path injection. [relay is OUT of v0.5.2 scope per CLAUDE.md — findings = v0.6 relay hardening backlog]

## PENDING (1 auditor): cross-cutting-sweep.

## EARLY BLOCKER SHORTLIST (must fix before release)
1. Android notif-counter (4 CRIT, 1 fix) — USER-REPORTED.
2. Android HistoryActivity copies truncated snippet (data loss).
3. CLI Supabase password plaintext in config → Keychain.
4. Android FFI runtime() panic → Result.
5. Crypto: SQLCipher key zeroize + encrypt_chunks panic guard + min passphrase len.
