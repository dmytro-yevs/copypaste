# v0.5.2 Pre-Tag Verification Checklist (meta-analysis — what static audit can't see)

From opus think-agent on HEAD 7e4735b. The static audit found bugs INSIDE modules; this covers the seams BETWEEN them + runtime + CI blind spots.

META-CONCLUSION: CI compiles + unit-passes on macOS, but NEVER runs the daemon cloud path with tests, NEVER runs Kotlin in CI, NEVER exercises two devices syncing. The cross-language crypto conformance harness is the right pattern — extend it (cloud round-trip + emulator) and make it a blocking gate, plus one real-hardware dual-sync sign-off.

## TOP 10 BEFORE TAG
1. [BLOCKER] Mac→Android AND Android→Mac via Supabase, real devices, same passphrase — core feature, untested by any gate.
2. [BLOCKER] Add `cargo test -p copypaste-daemon --features cloud-sync` to CI — is_synced + keyset watermark fixes run NOWHERE today.
3. [BLOCKER] SkippedBatch fix: rapid multi-copy doesn't lose latest clip + regression test.
4. [BLOCKER] FTS rows deleted with items on prune/ttl; assert count parity (ghost search + unbounded growth on size-prune path).
5. [BLOCKER] Mac↔Android LAMPORT SEMANTICS (wall-millis vs logical clock) — verify LWW picks intended winner both directions. NEW — not in module audit.
6. [BLOCKER] Per-setting live-apply audit UI→daemon: {reaches-live / reaches-after-restart} table; p2p_enabled + passphrase + limits actually take effect (A-SET-1).
7. [BLOCKER] Android: foreground-copied item counted AND synced; history-copy = FULL plaintext not 140-char snippet.
8. [BLOCKER] Run Kotlin CryptoConformanceTest on emulator once; confirm both golden_vectors.json byte-identical.
9. [BLOCKER] Cross-device bytea payload_ct: Mac-written row decrypts on Android + vice-versa; add paired Rust↔Kotlin hex-codec golden.
10. [BLOCKER] Revoke-peer blocks WITHOUT restart; passphrase-mismatch = visible skip not silent void; sensitive wipes cross-device (pinned-sensitive survives).

## NEW FINDINGS (not in per-module audit) → add to fix-wave
- 🔴 LAMPORT DIVERGENCE (§1.3): Android SyncManager.kt:311 lamportTs=currentTimeMillis vs Mac logical clock. Decide ONE semantics. Likely: Android should use a logical Lamport counter (persisted, max(local,incoming)+1) OR both agree to wall-millis. Must produce same LWW winner for same causal order. FIX + cross-device test.
- 🟠 bytea payload_ct codec (§1.2): two hand-rolled hex codecs (Rust encode_payload_ct_hex ↔ Kotlin encodePayloadCt) with NO conformance binding. Add golden vector.
- 🟠 cloud-sync tests not in CI (§6.1/7.1): add `cargo test -p copypaste-daemon --features cloud-sync` job (mocked HTTP).
- 🟠 Kotlin tests not in CI (§6.3/7.2): add `./gradlew testDebugUnitTest` + emulator connectedAndroidTest for conformance.
- 🟠 android-uniffi-live clippy gate (§7.4); DMG frontend-freshness grep (§7.5); notarization/Gatekeeper on clean Mac (§7.6).
- 🟢 empty-plaintext dedup (§4.7); non-ASCII byte-len eviction (§4.8 — overlaps Android plaintextLen fix); device_id stability (§1.6).

## HOW TO VERIFY (runtime — needs hardware, USER sign-off per release gate)
- scripts/e2e.sh has Supabase support but is wired into NO workflow — use it for the dual-sync E2E.
- scripts/acceptance.sh = local smoke + 2-process loopback P2P only.
- Conformance harness: crates/copypaste-android/tests/conformance_vectors.rs (Rust gen) + android/app/src/androidTest/.../CryptoConformanceTest.kt (Kotlin consumer) + golden_vectors.json in BOTH tests/fixtures/ and androidTest/assets/ — DIFF them.

Full reasoning: think-agent output in session transcript. Ties [[project-v052-audit-findings]] [[project-dualsync-rootcause]] [[feedback-release-gate]] [[feedback-platform-parity]].
