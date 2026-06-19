# CopyPaste — Reopened Issues (post-fix verification)

Date: 2026-06-19 · Tracking: CopyPaste-d6th. These closures failed independent verification and have been set back to `open` in bd (or filed new). Each has code/test/gate evidence — no "looks fixed" acceptances.

---

## R1 — iqkm: clippy `-D warnings` RED (25 errors)
- **bd:** CopyPaste-iqkm (reopened) · **Severity:** P2 (blocks the `-D warnings` gate / CI / merge)
- **What still broken:** The Zeroizing wrapping is functionally correct, but the author added explicit `&*` derefs that clippy rejects. `cargo clippy --workspace --all-targets --all-features -- -D warnings` = **exit 101**, 25 errors: 24× `explicit_auto_deref` + 1× `empty_line_after_doc_comments`.
- **What the fix missed:** running clippy at all (the prior round had no toolchain).
- **Affected files:** `crates/copypaste-daemon/src/ipc.rs` (lines 4002,4004,4006,4089,4091,4093,4175,4330,4332,4334,7842,7843,7981,8134,8137,8138,8232,8234,8236,8290,8292,8294), `src/daemon.rs:3875`, `src/relay.rs:1963` (blank line after doc comment).
- **Fix:** replace each `&*x` with `&x`; remove the blank line at relay.rs:1963.
- **Test plan:** `cargo clippy --workspace --all-targets --all-features -- -D warnings` must exit 0.
- **Owner:** daemon.

## R2 — xxsw: stale test fails under `--all-features`
- **bd:** CopyPaste-xxsw (reopened) · **Severity:** P1 (a test in the suite fails)
- **What still broken:** `copypaste-android::tests::live_calls_reuse_cached_connection` panics at `lib.rs:3120` — `"db_(path,key) must be cached after first live call"`. The fix re-keyed `DB_BY_PATH` to `SHA-256(key)`, but the test still asserts `db_by_path().contains_key(&(path, raw_key_arr))` using the raw key.
- **What the fix missed:** updating the pre-existing live test to the new cache-key contract (3 NEW tests were added; the OLD one was not touched).
- **Affected files:** `crates/copypaste-android/src/lib.rs:3108-3124` (the test).
- **Fix:** look up via `key_cache_hash(&key)` (or assert the handle→side-map contract).
- **Test plan:** `cargo test -p copypaste-android --all-features` exits 0.
- **Owner:** android.

## R3 — kmcr (PG-2): relay PoP only 1/3 wired
- **bd:** CopyPaste-kmcr (reopened) · **Severity:** P1
- **What still broken:** Android relay registration still sends no `pop_b64` → relay may reject (depending on relay version). The Rust `relay_registration_pop` + UDL declaration exist, but the **generated Kotlin bindings were never regenerated** (`copypaste_android.kt` lacks the fn), there is **no `CopypasteBindings.kt` wrapper**, and **`RelayClient.registerDevice()` does not call it**.
- **What the fix missed:** `./scripts/generate-android-bindings.sh` + Kotlin call-site.
- **Affected files:** `crates/copypaste-android/src/lib.rs` (fn present), `uniffi/copypaste_android.udl` (declared), `android/.../generated/.../copypaste_android.kt` (NOT regenerated), `CopypasteBindings.kt` (no wrapper), `RelayClient.kt` (`registerDevice` body).
- **Fix:** regenerate bindings; wrapper in `CopypasteBindings.kt`; `pop_b64 = base64(relay_registration_pop(syncKey, inboxId))` in `registerDevice`.
- **Test plan:** register against a mock relay; assert non-null `pop_b64` accepted; byte-equality with the daemon derivation for the same (sync_key, device_id).
- **Owner:** android (needs cargo-ndk + Gradle).

## R4 — 8qcm (PG-12): revoke+rotate only 1/3 wired (SECURITY)
- **bd:** CopyPaste-8qcm (reopened) · **Severity:** P1 (security — revoked peer retains the sync key)
- **What still broken:** Android revoke dialog (`DevicesActivity.kt:658-687`) calls only `revokeDeviceAudit()`. No passphrase field, no rotation. Dialog text literally tells the user to rotate manually in Settings. A revoked peer can still decrypt cloud/relay blobs.
- **What the fix missed:** Kotlin bindings regen + dialog wiring to `revoke_device_and_rotate_key`.
- **Affected files:** `crates/copypaste-android/src/lib.rs:694-735` (Rust present), `uniffi/copypaste_android.udl` (declared), `android/.../DevicesActivity.kt` (dialog unchanged), generated bindings (not regenerated).
- **Fix:** regenerate bindings; add passphrase `OutlinedTextField` + "Revoke & rotate" path calling `revoke_device_and_rotate_key`; store returned key in AndroidKeystore, zero the `ByteArray`; re-derive relay inbox id + PoP and re-register.
- **Test plan:** revoke → rotate → old key rejected by relay poll; revoked device cannot read new cloud blobs.
- **Owner:** android.

## R5 — jbao (P1-1): doc regression + tautological test
- **bd:** CopyPaste-jbao (reopened) · **Severity:** P1 (privacy doc contradiction) + P2 (test gap)
- **What still broken:** (a) **`docs/relay-api.md:254-255` says "Sensitive items do sync through the relay (encrypted)"** — the exact opposite of the shipped fix; a user reading the doc would believe their secrets sync. (b) The guard test `relay.rs:1977-2020 push_loop_skips_sensitive_items` asserts `item.is_sensitive==true` twice and never invokes `push_loop` — it passes even if the guard is deleted. The cloud push/backlog and P2P catch-up guards have **no test at all**.
- **What the fix missed:** the doc author retained the pre-fix description; the test author wrote a predicate assertion instead of a behavior test.
- **Affected files:** `docs/relay-api.md:254-255`; `crates/copypaste-daemon/src/relay.rs:1977-2020`; cloud.rs / sync_orch.rs guard paths (untested).
- **Fix:** correct relay-api.md to "sensitive items are never uploaded"; add a test that drives a sensitive item through `push_loop` (mock sink) and asserts zero outbound, or asserts `build_content_b64` is the only filter.
- **Test plan:** mock-relay test: sensitive item → no POST; non-sensitive → POST.
- **Owner:** daemon + docs.

## R6 — 17lj: relay-api.md rewrite introduced the regression
- **bd:** CopyPaste-17lj (reopened) · **Severity:** P1 (same doc as R5)
- **What still broken:** the relay-api.md rewrite is otherwise accurate to the wire protocol but lines 254-255 contradict the sensitive-exclusion guarantee.
- **Fix/Test:** see R5; one correction closes both.
- **Owner:** docs.

## R7 — 7d8x (PG-1): Unpair handler untested
- **bd:** CopyPaste-7d8x (reopened) · **Severity:** P2 (code correct; no regression guard)
- **What still broken:** `p2p_listener.rs` correctly parses `PeerFrame` and evicts on `Control(Unpair)`, but there is **no test**. The bd close reason itself said "NEEDS instrumented test: Unpair reaches listener, reconnect refused, no items post-Unpair."
- **Affected files:** `crates/copypaste-android/src/p2p_listener.rs:332-411` (handler), test suite `:748-961` (no Unpair case).
- **Fix:** extend the `dial_and_send` harness to send a serialized `PeerFrame::Control(ControlMsg::Unpair)`; assert peer evicted from `PeerState::peers`/`allowed`, added to `revoked`, connection closed.
- **Owner:** android.

---

## New issues filed during verification

### NEW-1 — e5oe: FTS orphan on cloud-sync overwrite (P2)
- `sync_common::replace_cloud_item_by_item_id` (`sync_common.rs:527-569`, every cloud/relay inbound LWW) and `sync_orch::sweep_poison_rows` (`sync_orch.rs:1947`) DELETE from `clipboard_items` by `item_id` but never delete the matching `clipboard_fts` row → orphaned searchable plaintext accumulates on every cloud-synced item overwrite. The original audit verified only the `items.rs` delete paths.
- **Fix:** `delete_fts_for_ids` in the same transaction. **Test:** cloud-overwrite an item; assert old content not returned by FTS search.

### NEW-2 — h7v8: Kotlin Panicked not propagated (P3)
- `CopypasteBindings.kt` adds `class Panicked` but the 24 hand-wrapper catch blocks catch generic `CopypasteException` and rethrow as `EncryptionFailed`/`DatabaseError`. Callers via wrappers never see `Panicked` (lose the reason).
- **Fix:** check `is CopypasteException.Panicked` before the generic catch in each wrapper.

---

## NEEDS-QA / design (not reopened, but must not be re-closed without resolution)
- **nq39** — residual plaintext Supabase password in IPC `set_config` body (0600 socket; local only).
- **PG-6 (zfqa)** — `protocolMismatchHandler` never assigned in `App.tsx`; no UI banner (console.warn only). Stale comment `ipc.ts:83`.
- **PG-9 (wb6s)** — DESIGN: full-hex fingerprint (spec wanted truncate+tap-to-copy) and reverses the prior intentional removal (CopyPaste-n).
- **PG-11 (71cf) / PG-10** — shared `SyncBadgeState` enum NOT adopted; PG-10 offline-signal divergence (IPC-unreachable vs OS-network) still open → badges can still disagree under a daemon crash.
- **PG-16 (mxoq)** — classifier FFI added but `TextKind.kt` Kotlin swap not done.
- **PG-24** — Android WorkManager TTL prune not addressed (deferred).
- **tj9s/PG-5** — export sensitivity filter has no test.
