# Edge Case & Test Coverage Audit — CopyPaste v0.1.0-alpha.1
**Auditor:** tester
**Date:** 2026-05-23
**Commit:** 7a577f7f9906c3504b789b394383ac9ebf1588b1
**Total findings:** 38 (Critical: 4, High: 11, Medium: 13, Low: 7, Info: 3)
**Existing test count:** 403 tests in workspace
- copypaste-core: 97 | copypaste-daemon: 60 | copypaste-relay: 70
- copypaste-ui: 41 | copypaste-cli: 43 | copypaste-p2p: 31
- copypaste-supabase: 32 | copypaste-sync: 29 | copypaste-android: 0

## Findings (sorted by severity)

| # | Severity | Category | Scenario | Current Coverage | Recommendation |
|---|----------|----------|----------|------------------|----------------|
| 1 | CRITICAL | IPC | `BufReader::lines()` in `ipc.rs:147` is unbounded — a client can write a 4 GB line and OOM the daemon | NONE (no MAX_REQUEST_BYTES) | Add `take(MAX_REQUEST_BYTES)` wrapper + test that sends >limit and asserts disconnect, not crash. Mirror `copypaste-sync::engine::MAX_FRAME_SIZE` pattern. |
| 2 | CRITICAL | Storage | Schema downgrade: user installs older alpha, DB has `user_version=2`, code constant is `1` → `apply_migrations` short-circuits with `Ok`, queries then fail at runtime referencing unknown columns | NONE (`schema.rs:21` only checks `>=`) | Add test `downgrade_returns_explicit_error()` opening a DB with `user_version > SCHEMA_VERSION` and asserting structured `SchemaError::Downgrade`. Currently swallowed silently. |
| 3 | CRITICAL | Sync | `LamportClock::tick` (`clock.rs:33`) uses `value += 1` — overflow at u64::MAX panics in debug, wraps to 0 in release, breaking LWW total order forever | NONE | Switch to `saturating_add`+log warning, or panic with explicit error. Add test `tick_saturates_at_u64_max()` and `observe_saturates_at_u64_max()`. |
| 4 | CRITICAL | Storage | Concurrent SQLite writes: daemon holds `Arc<Mutex<Database>>` (`daemon.rs:38`) but cloud-sync (`cloud.rs`) and p2p-sync (`p2p.rs`) clone the same handle. WAL helps, but no test exercises simultaneous insert from all 3 writers + verifies no lost updates / ordering invariants | NONE | Add `tests/concurrent_writers.rs`: spawn 3 tokio tasks (clipboard-monitor, sync-engine, cloud-push), each insert 1000 items, assert count = 3000 and Lamport monotonicity preserved. |
| 5 | HIGH | Clipboard | No test for rapid clipboard changes (poll loop drops events between cycles). At 60 fps a copy-paste-copy in <16ms is invisible | NONE | Add property test: simulate `changeCount` jumping by 5 between two `poll()` calls — current code only ever returns the latest, document this is intentional or fix. |
| 6 | HIGH | Clipboard | Mixed types on macOS: text + image present simultaneously. Code (`clipboard.rs:71`) silently drops image when text exists. No test confirms intent | NONE | Add unit test with mocked pasteboard providing both; assert text wins, log warning, or expose `ClipboardContent::Mixed`. |
| 7 | HIGH | Clipboard | Binary clipboard data that is not text and not a valid PNG/TIFF (e.g., custom Adobe formats, RTF, files, URLs) → silently dropped, no telemetry, no error | NONE | Add test for `public.rtf` / `public.file-url` and assert either accepted or logged-as-unsupported, never silent. |
| 8 | HIGH | Storage | SQLite full-disk: no `try_insert` path that handles `SQLITE_FULL`. Daemon will likely loop-spam errors | NONE | Add test using `tmpfs` with 1 MB quota or fault-injection wrapper; assert daemon backs off and surfaces to UI. |
| 9 | HIGH | Storage | SQLCipher with corrupted/missing key in Keychain: `encrypted_db_rejects_wrong_key` exists but no end-to-end test for "Keychain entry was deleted by user/macOS" path — daemon currently panics on `Keychain::get_or_generate` failure | partial (db.rs:251) | Add test mocking `keychain::get_or_generate` returning `Err`, assert daemon enters degraded mode with user-visible notification, does not crash-loop. |
| 10 | HIGH | IPC | Client disconnects mid-request: `read_line` will return `Ok(0)` but no test verifies the handler doesn't try to write to a closed socket and panic | NONE | Add test in `integration_ipc.rs`: open stream, write half a line, drop. Server task must not panic and must release resources. |
| 11 | HIGH | IPC | Multiple clients connecting simultaneously: existing tests all use single client. No test for N=10 concurrent connections doing `list` + `delete_all` interleaved | NONE | Add tokio test spawning 10 client tasks; assert all succeed and DB state is consistent (transaction isolation). |
| 12 | HIGH | P2P | Rogue mDNS announcement: peer with valid `_copypaste._tcp` record but unpaired fingerprint must NOT initiate sync. Current `transport.rs` uses cert fingerprint allowlist but no test simulates rogue announce + connect attempt | partial | Add test in `copypaste-p2p/tests/` that registers a rogue peer in discovery and asserts `TlsVerifier::verify_fingerprint` rejects with `UnknownPeer`. |
| 13 | HIGH | P2P | TLS handshake timeout: no `tokio::time::timeout` wrapping handshake. A malicious peer can hold the connection forever | NONE (transport.rs) | Wrap `connect()`/`accept()` in `timeout(Duration::from_secs(10), ...)`; add test that hangs accept side for 30s and asserts client times out at 10s. |
| 14 | HIGH | Crypto | Truncated/missing chunk in middle of multi-chunk stream (e.g. chunks 1,2,4): `truncated_stream_fails_decryption` only covers truncation at the end | partial (chunks.rs:174) | Add `gap_in_middle_fails_decryption()` covering N-1 truncations: should fail at the gap, not silently produce partial plaintext. |
| 15 | HIGH | Sensitive | False positive: `generic_password_kv` matches `"password: hello world"` in a clipboard quote/forum post → user content silently redacted on sync. No false-positive corpus test | NONE | Add `tests/false_positive_corpus.rs` with 50+ benign strings (Lorem Ipsum with the word "password", chat snippets, code samples). Track FP rate; baseline ≤5%. |
| 16 | MEDIUM | Sync | Two peers with identical Lamport + identical wall_time + identical device_id (clone of a stolen image) → LWW falls through device_id tiebreak with equal IDs → undefined behavior in `merge.rs` | partial (`equal_lamport_equal_wall_larger_device_id_wins`) | Add `identical_everything_is_idempotent()` asserting merge of two byte-identical items is no-op. |
| 17 | MEDIUM | Sync | Network partition + heal: A and B both write items 100-110 while disconnected. On reconnect, items must merge without duplicates and Lamport clocks converge | NONE (existing tests are single-shot) | Add scenario test: `(A↔B sync, partition, both write 10 items, heal, resync) → assert both DBs identical and clock = 121`. |
| 18 | MEDIUM | P2P | NAT traversal failure: discovery sees peer but `transport.connect` cannot reach. No test covers "peer visible but unreachable" — should not block UI | NONE | Add `discovery_advertises_unreachable_peer_does_not_panic()`; assert `connect()` returns explicit error after timeout. |
| 19 | MEDIUM | Cloud | Supabase WS disconnect during outbound push: reconnect logic exists in `realtime.rs` but no test asserts items queued during disconnect are flushed on reconnect (could lose items) | partial (backoff tested) | Add test injecting WS close mid-push; assert push retries and item lands in Supabase. |
| 20 | MEDIUM | Cloud | Auth token expiry mid-sync: `spawn_auto_refresh` exists but no test for "token expires *between* request build and request send" (race window) | NONE | Add test mocking `expires_at = now+1s`, sleep 2s, attempt push, assert retry-with-refresh path. |
| 21 | MEDIUM | Cloud | Server returns 429: no `Retry-After` handling visible in `cloud.rs` | NONE | Add test mocking 429 response; assert exponential backoff honors header. |
| 22 | MEDIUM | Cloud | Server schema mismatch (Supabase row has extra/missing JSON field after server upgrade): deserialization will fail and skip the entire batch | NONE | Add test with `serde_json::Value` payload containing unknown field; assert graceful skip + log, not abort. |
| 23 | MEDIUM | IPC | Method called before daemon `Database` is initialized (race at startup): IPC listener bound before `db.lock().await` resolves | NONE | Add test confirming early calls return `IPC_NOT_READY` not panic. |
| 24 | MEDIUM | IPC | Very large response (history with 100k items, single `list` call) → Vec materialization OOM + line longer than client buffer | NONE | Add test inserting 100k items and asserting `history_page` enforces server-side pagination cap. |
| 25 | MEDIUM | UI | Slint `HistoryWindow` opened before daemon socket exists (cold boot): `IpcClient::connect` returns `Err`. No test for "is the error surfaced or does Slint panic" | partial (`ipc_client_connect_fails_when_no_socket`) | Add windows-level test asserting empty-state UI shown, not crash. |
| 26 | MEDIUM | UI | 10k+ items in history: Slint `ListView` without virtual scrolling will allocate every row → slow open / OOM | NONE | Add benchmark + UX test: assert initial render <500 ms for 10k items, or paginate. |
| 27 | MEDIUM | UI | PairWindow with no peers discovered (mDNS off): empty state visible? Or perpetual "searching" spinner? | NONE | Add test that `discovery_service` returns no peers and assert `PairWindow` shows actionable empty state with troubleshooting hint. |
| 28 | MEDIUM | Sensitive | Unicode-encoded secrets: AWS key with full-width digits (`ＡＫＩＡ...`) or zero-width-joiner inside JWT will bypass regex | NONE | Add corpus test for NFKC-normalised inputs; either normalise before detect or document as out of scope. |
| 29 | LOW | Process | Daemon crash + launchd KeepAlive restart: no integration test verifies a SIGKILL'd daemon restarts and recovers state (Lamport clock persisted? socket re-bound?) | NONE | Add `tests/lifecycle.rs` that kills the daemon via SIGKILL, asserts launchd respawn (or simulates), and Lamport clock continues from persisted value. |
| 30 | LOW | Process | SIGTERM mid-sync: graceful shutdown handler not visible. Sync session in flight should commit or rollback cleanly | NONE | Add test sending SIGTERM during `engine.run_session`; assert no half-written items in DB. |
| 31 | LOW | Process | System sleep/wake: WS connections + UnixSocket file descriptors typically broken after >30 min sleep. Daemon must re-establish | NONE | Add documentation + test stub `wake_from_sleep_reconnects_supabase_ws()`. |
| 32 | LOW | Sensitive | Multi-line credit-card numbers split across lines (`4111-1111-\n1111-1111`): pattern (none currently) won't match | NONE (no credit_card pattern) | Add `credit_card` regex and multi-line test. Currently a coverage gap, not a regression. |
| 33 | LOW | Crypto | Nonce collision after 2^96 messages: irrelevant in practice but no doc-test asserts XChaCha20 is used (24-byte nonce) vs ChaCha20 (12-byte) | partial | Add doctest in `encrypt.rs` confirming nonce size = 24 with comment on safe ceiling. |
| 34 | LOW | P2P | Lamport clock overflow on receive (`observe(u64::MAX)` → `+1` → panic/wrap). Covered by #3 but also needs to be tested via the wire protocol path | NONE | Add `engine.rs` test sending a `WireItem { lamport: u64::MAX, .. }` and assert clock saturates rather than panics. |
| 35 | LOW | Storage | DB corruption mid-write (power loss): WAL should auto-recover, but no test corrupts the `-wal` file and verifies graceful open or actionable error | NONE | Add `tests/corruption.rs` truncating the `-wal` file and asserting `Database::open` returns `DbError::Corrupt` not silent data loss. |
| 36 | INFO | Platform | macOS image format variation (TIFF on Sequoia vs PNG on Sonoma): code reads both but no parametrised test fixture | NONE | Add fixture files `tests/fixtures/clipboard_tiff.bin`, `clipboard_png.bin`; ensure both decode via `image.rs`. |
| 37 | INFO | Platform | Windows clipboard formats: scaffolding for `clipboard.rs` non-macos returns `Ok(None)` always; no test guards regression when Windows impl lands | NONE | Add `#[cfg(target_os="windows")] fn poll_returns_none_today()` placeholder so future PR fails the test instead of silently succeeding. |
| 38 | INFO | Android | `copypaste-android` crate has 0 tests — entire foreground service path untested | NONE | At minimum: unit-test `NotificationHelper`, `SharedPreferences` settings serialisation, and IPC bridge protocol. |

## Coverage Gap Summary by Crate

- **copypaste-core (97 tests)**: solid happy-path coverage of crypto/storage/sensitive. Gaps: schema downgrade (#2), concurrent writers (#4), sensitive false-positive corpus (#15), Unicode bypass (#28), multi-line patterns (#32), DB corruption (#35).
- **copypaste-daemon (60 tests)**: IPC method coverage strong, but no client-side disconnect (#10), no concurrent clients (#11), no large-request DoS (#1), no init-race (#23), no large-response cap (#24), no lifecycle (#29-31).
- **copypaste-sync (29 tests)**: merge LWW well-tested. Gaps: Lamport overflow (#3, #34), identical-everything idempotency (#16), network-partition-heal (#17).
- **copypaste-p2p (31 tests)**: TLS verifier + discovery covered. Gaps: rogue mDNS (#12), TLS handshake timeout (#13), NAT failure (#18).
- **copypaste-supabase (32 tests)**: realtime backoff + auth tested. Gaps: 429+Retry-After (#21), schema drift (#22), refresh race (#20), push-during-disconnect (#19).
- **copypaste-ui (41 tests)**: ipc_client + fingerprint formatting covered. Gaps: cold-boot UX (#25), virtual scrolling (#26), empty PairWindow (#27).
- **copypaste-relay (70 tests)**: HTTP handlers strong; out of release-blocker scope.
- **copypaste-cli (43 tests)**: command parsing solid; out of release-blocker scope.
- **copypaste-android (0 tests)**: complete coverage gap (#38). Acceptable for alpha if marked "preview only".

## Top 5 must-have tests before alpha ship
1. **#1** IPC request size limit + DoS test (trivial OOM exploit by local process).
2. **#2** Schema downgrade explicit error (silent failure → data loss when user reinstalls).
3. **#3 + #34** Lamport saturation tests (release-mode silent wrap breaks sync).
4. **#4** Concurrent writers stress test (cloud + p2p + daemon writing simultaneously — the core ship promise).
5. **#9** Keychain missing/corrupted-key degraded-mode test (most likely real-user crash path on macOS).

## Blocker for alpha release?
**YES** — findings #1, #2, #3, #4 are all silent-failure paths in code that runs on every install. #1 is a local DoS exploit. #2 makes downgrade scenarios irrecoverable. #3 corrupts sync ordering permanently in release builds. #4 is the entire reason this product exists (multi-source sync) and has zero integration coverage.

Mitigation: write the **top 5 tests** above (estimated 1 day of focused work). All other findings are acceptable as documented `known-issues` for an alpha.
