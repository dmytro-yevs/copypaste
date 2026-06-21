# CopyPaste — Full Repository Audit Report

**Date:** 2026-06-20/21
**Auditor:** Orchestrated multi-agent audit (11 parallel senior/lead auditor streams + orchestrator synthesis)
**Scope:** Entire workspace — 12 Rust crates, Tauri/React desktop UI, Android/Kotlin/Compose app, relay, CI, scripts, docs.
**Method:** Read-only code inspection (cited `file:line`), plus executed quality gates. No source files were modified during the audit.

---

## 1. Executive Summary

CopyPaste is a **mature, security-conscious codebase**, not a prototype. The hard architectural and cryptographic contracts hold up under inspection:

- **Architecture boundaries are clean (PASS).** `copypaste-cli` and `copypaste-ui/src-tauri` have **zero** `copypaste-core` linkage — they speak Unix-socket IPC only. `copypaste-relay` is standalone (no internal crate deps, no SQLCipher). The daemon is the sole owner of clipboard polling, encryption, DB writes, and Keychain.
- **Crypto core is sound (PASS).** XChaCha20-Poly1305 with AEAD AAD binding `(item_id, schema_version, key_version)`, purpose-separated HKDF-SHA256 info strings, PAKE (OPAQUE + RFC-5705 channel binding + SAS) with **mandatory** confirm tags enforced via `subtle::ct_eq` at all four bootstrap call sites, derived sync keys never placed in QR and redacted in `Debug`.
- **Relay is ciphertext-only (PASS, verified).** Stores only ciphertext/nonce/metadata/sender/lamport_ts; no plaintext, keys, or pairing secrets. Bearer token is 16 bytes from `OsRng` (stronger than the documented `SHA-256(pubkey)[..32]`) compared in constant time. Fan-out, dedup, replay, oversized-payload, invalid-base64, and auth-failure paths are handled and the sensitive-exclusion filter is enforced and tested.
- **Quality gates are green:** `cargo check --all-features`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` all pass. The token-parity scripts pass 53/53 and 21/21.

The findings are therefore **not** "the foundation is broken." They are **completeness, reliability, UX-safety, and cross-platform parity** gaps — concentrated in: destructive-action UX (missing confirmations/undo), Android↔macOS feature drift, error-message hygiene (raw exceptions/paths leaking into UI), a handful of non-atomic DB writes, secret-zeroization edge cases, and CI coverage holes.

## 2. Overall Verdict & Risk Level

| | |
|---|---|
| **Build/test health** | 🟢 Green (all primary gates pass) |
| **Architecture integrity** | 🟢 Strong — contracts verified, no P0/P1 violations |
| **Cryptography** | 🟢 Strong — no broken crypto found |
| **Data-loss safety** | 🟡 Medium — non-atomic FTS/revoke writes, restore without confirm |
| **UX safety (destructive actions)** | 🔴 Needs work — bulk delete / revoke-all / clear fire without real confirmation or undo on both platforms |
| **Cross-platform parity** | 🟡 Token layer is in parity; **feature/behavior drift remains** (Android export/undo/P2P/recovery; light-first default contradicted on both) |
| **Production-ready?** | **Not yet.** No release-blocking P0, but a cluster of P1 reliability/UX/parity issues should be fixed first. |

**Overall risk: MEDIUM.** No critical (P0) issue and no broken encryption, but enough P1 reliability/UX/parity defects that shipping as-is risks data-loss surprises (no-confirm deletes), user-trust confusion (stale "connected" status, misleading sync warning), and silent Android crypto failure modes (non-fatal ABI mismatch, ProGuard stripping FFI).

## 3. What Was Verified vs. Not Verified

**Verified in code (high confidence):** crate dependency graph & boundary contracts; AEAD/AAD/HKDF/PAKE crypto; relay ciphertext-only storage + auth + fan-out; SQLCipher keying & WAL; IPC framing/error-code contract; daemon lifecycle & polling; CLI command→IPC mapping; Tauri command bridging; UniFFI ABI version match (Rust 18 ↔ Kotlin 18); token parity (scripts green). All quality gates executed and observed.

**Could NOT be fully verified (documented):**
- `cargo audit` — environment blocked (corrupt cached advisory-db + network egress blocked). Advisory coverage is still satisfied via `cargo deny check` ("advisories ok"). See VERIFICATION_REPORT.md.
- Runtime behavior on real devices (actual macOS↔Android sync, camera QR scan, Keychain ACL on a live login keychain, launchd/foreground-service lifecycle) — requires manual QA; not executable in this environment.
- Exhaustive Android Compose control-by-control trace — sampled, not 100% (flagged by the completeness auditor).
- Whether the live ingest path calls `WireItem::clamp_timestamps` — not resolvable from the four sync crates alone.

## 4. Quality Gates — Commands & Results

| Command | Result |
|---|---|
| `cargo check --workspace --all-features` | ✅ exit 0 |
| `cargo fmt --all --check` | ✅ exit 0 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | ✅ exit 0 |
| `cargo test --workspace --all-features` | ✅ exit 0 |
| `cargo deny check` | ✅ advisories ok, bans ok, licenses ok, sources ok |
| `cargo audit` | ⚠️ ENV-blocked (corrupt advisory-db, no network) — covered by `cargo deny` |
| `node scripts/parity-check.mjs` | ✅ 53/53 token comparisons within ±5 |
| `node scripts/check-skin-parity.mjs` | ✅ web SKINS ↔ android skinTokens 21/21 |
| Frontend `pnpm lint/typecheck/test/build` | ⏳ not run this pass (no ESLint config exists — see bd / H-F01) |
| Android `./gradlew test/lint/assembleDebug` | ⏳ not run this pass (cross-compile OOM guard; deferred to fix stage) |

## 5. Findings Tally (after dedup)

| Severity | Count | Filed to bd |
|---|---|---|
| **P0 (critical)** | **0** | — |
| **P1 (high)** | 23 | ✅ |
| **P2 (medium)** | 66 | ✅ |
| **P3 (low)** | 45 | ✅ |
| **P4 (nice-to-have)** | 3 (umbrella) | ✅ |
| **Total filed** | **137** | all tagged `[AUDIT-0620 src:...]` |

Raw stream output was ~205 findings; deduplicated to 137 canonical issues. **5 findings were re-verified and refuted** (false positives — see §7). Per-stream detail lives in `.audit/A..I, P1..P3.md`. The full canonical table is in `AUDIT_FINDINGS.md`.

## 6. Top Risks (fix first)

1. **Destructive actions without confirmation/undo** (P1 ×7) — macOS bulk delete `fjvz`, revoke-all `uw45`, clear-history `w6xc`; Android single delete `2ifa`, Clear-All swallows errors `yel4`. Data-loss UX on both platforms.
2. **Android silent crypto-failure modes** (P1 ×2) — ABI mismatch is non-fatal `fkx7` (silently corrupts crypto data); ProGuard may strip UniFFI entry points so a release build silently runs in no-crypto **stub mode** `hh3w`.
3. **Full-history plaintext export over IPC** (P1) — `phit`: `export` with `include_sensitive=true` decrypts every row (incl. credentials) over the socket / to CLI stdout with no warning, confirm, or audit beyond a count.
4. **Non-atomic DB writes** (P1 ×2) — `upsert_fts` `j9pv` (orphaned/unsearchable items) and `revoke_device` `d7um` (lost revocation audit record → device looks un-revoked) are DELETE+INSERT without a transaction.
5. **Misleading runtime state** (P1/P2) — theme defaults to dark on both platforms while spec says light-first `ei27`; SyncStatusChip shows stale "connected" for ~10s after going offline (P2); "Enable sync" shows a stale "requires daemon update" warning even though the daemon honors it (P2).
6. **Cross-platform feature drift** (P1/P2) — Android lacks export/import/backup `8jx8`, delete-undo `kaf6`, P2P LAN sync, and degraded-DB recovery.
7. **Privacy leaks in error UI** (P2) — socket paths + macOS username rendered into the DOM/toasts on both platforms.
8. **Secret-zeroization gaps** (P1/P2) — `process::exit` paths skip `Zeroizing` drops `liaz`; `derive_storage_key_v1` and `ecdh` return un-zeroized key copies.

## 7. Refuted Findings (do NOT re-file)

Re-verified as correct in source: macOS *does* have clear-all (Settings→Storage); web peer cards *do* show Verified badge + fingerprint grid; Android FTS indexing sensitive items is *consistent* with the daemon (not drift); relay *does* enforce sensitive-exclusion on push (tested); all reported design-token drifts are already fixed (scripts green 53/53 + 21/21). The `sync_enabled` "no-op stub" claim was also refuted — the daemon fully gates P2P/relay/cloud on it.

## 8. bd Tracking

Every canonical finding is a bd issue (Ukrainian title, English body, `[AUDIT-0620 src:<stream-code>]` trace tag). Query them:

```bash
bd list --priority=1 --status=open      # the 23 P1s
bd list --status=open | rg "AUDIT"      # full audit set (descriptions carry the tag)
```

bd state after filing: **Open 138, Closed 609, Total 752, In-progress 0.**

## 9. Recommended Next Steps

This audit is **Stage 1 (discovery)**. The remaining stages are gated on it:

- **Stage 2 — Fix P1 (security/data-loss/reliability):** the 23 P1 issues, starting with destructive-UX confirmations, Android ABI/ProGuard hardening, the two non-atomic writes, and export-warning. Each fix ships with the regression test named in its bd issue.
- **Stage 3 — Parity fixes:** Android export/undo/recovery, light-first default, sync-status recency, error-message hygiene.
- **Stage 4 — CI/test hardening:** add ESLint+frontend CI, `--all-features` test job, PR-time Android lint, make fuzz/machete blocking, gitignore generated keystores.
- **Stage 5 — Final verification + manual QA:** real-device macOS↔Android sync, pairing, private/sensitive, offline transitions.

Companion documents: `AUDIT_FINDINGS.md` (master table), `AUDIT_FIX_PLAN.md` (phased plan), `VERIFICATION_REPORT.md` (gate results), `PARITY_MATRIX.md` + `FEATURE_INVENTORY.md` (agent-generated), and raw per-stream detail under `.audit/`.
