# CopyPaste Threat Model — v0.3.0

- **Version:** 0.3.0
- **Date:** 2026-05-23
- **Status:** Living document — re-review on every minor release
- **Methodology:** STRIDE per asset + trust-boundary analysis
- **Related ADRs:** ADR-001 (XChaCha20), ADR-002 (Unix socket IPC), ADR-003
  (SQLCipher), ADR-004 (SQLite WAL), ADR-013 (Tauri UI), ADR-007 (IPC
  versioning), ADR-008 (PAKE), ADR-009 (relay storage), ADR-010 (codesigning)

---

## 1. Scope & Assumptions

### 1.1 In scope

This threat model covers the **v0.3.0** surface of CopyPaste:

- The `copypaste-daemon` process (clipboard capture, storage, sync).
- The local on-disk **SQLCipher database** (`clipboard.db` / clipboard store).
- The OS **Keychain / Secret Service** entries holding the daemon's
  per-installation secrets (`db_key`, mTLS private key, device identity key).
- The **Unix-domain socket IPC** between the Tauri UI and daemon.
- The **mDNS + mTLS LAN sync path** between paired peers.
- The **`copypaste-relay` HTTPS endpoint** used as a store-and-forward inbox
  when peers are not simultaneously online.
- The **device-pairing PAKE handshake** (see ADR-008).
- The OPAQUE-KE session keys derived from a successful pairing.
- The Android foreground-service variant (clipboard read + push only — same
  threat surface as desktop for relay/peers; storage is Android Keystore +
  EncryptedSharedPreferences instead of Keychain).

### 1.2 Out of scope (explicit)

The following are acknowledged risks but are **not** mitigated by CopyPaste
itself; users must rely on platform mechanisms:

- **Host OS compromise.** Root/Administrator on the host can read process
  memory, the Keychain, and the UI's pixel buffer. No mitigation
  possible at the application layer.
- **Physical access to an unlocked device.** A user who hands over an
  unlocked machine cannot expect application-layer secrecy. Lock-screen and
  full-disk encryption (FileVault / dm-crypt / BitLocker) are the user's
  responsibility.
- **Compromise of the OS Keychain backing store.** We treat Keychain /
  Secret Service / Android Keystore as trusted hardware-backed KDF roots.
- **Kernel-level exploits, hypervisor escape, evil-maid firmware attacks.**
- **Supply-chain compromise of the Rust toolchain or crates.io** — partially
  addressed by `cargo audit` in CI but not modeled here.
- **TLS root-store compromise.** If a global CA root used by the relay's
  HTTPS certificate is mis-issued, relay traffic confidentiality is
  weakened (mTLS still binds peer ↔ peer, but the relay can be MITM'd by a
  state-level adversary). Out of scope for application code.
- **Side-channel attacks** (cache, timing, electromagnetic) against the
  XChaCha20-Poly1305 or PAKE implementations. We trust the upstream
  `chacha20poly1305` and `opaque-ke` crates.

### 1.3 Trust assumptions

- The OS Keychain / Secret Service / Android Keystore is **trusted**.
- The user's choice of pairing password (during PAKE) is **trusted as
  short-lived** — only required to survive long enough to complete one
  handshake; afterwards the derived OPAQUE-KE session keys take over.
- Once two devices complete pairing, each is **trusted not to be malicious
  toward the other** (TOFU model). Revocation is an open issue (§7).
- The relay is **trusted to be honest-but-curious** at most — it can drop,
  reorder, or delay items, but it cannot read plaintext, and tampering with
  ciphertext is detected by AAD-bound authenticated encryption (ADR-001).

---

## 2. Assets

Assets are ordered by sensitivity (highest first). Each asset is referenced
by ID in the STRIDE matrix (§4).

| ID  | Asset                                | Lives in                                  | Sensitivity                  |
| --- | ------------------------------------ | ----------------------------------------- | ---------------------------- |
| A1  | Plaintext clipboard items            | Daemon RAM, briefly in UI process         | **CRITICAL** — user content  |
| A2  | SQLCipher DB master key (`db_key`)   | Keychain / Secret Service / Keystore      | **CRITICAL** — unlocks A1    |
| A3  | Device identity private key (mTLS)   | Keychain                                  | **CRITICAL** — peer trust    |
| A4  | OPAQUE-KE session keys (post-pair)   | Daemon RAM, persisted encrypted in A6     | **HIGH** — sync confidentiality |
| A5  | Pairing PAKE password (ephemeral)    | User's brain → both devices' RAM, ~60 s   | **HIGH** — bootstraps A4     |
| A6  | Encrypted clipboard items on relay   | Relay process RAM (HashMap, ADR-009)      | **MEDIUM** — already AEAD'd  |
| A7  | Device identity (UniFFI device id)   | Daemon DB + relay registry                | **LOW** — public-ish        |
| A8  | Clipboard metadata (timestamps, len) | Daemon DB (A1 encrypted, metadata in clear inside SQLCipher) | **LOW–MEDIUM** — pattern leaks |
| A9  | mDNS discovery records on LAN        | Broadcast on local network                | **LOW** — presence only      |

Notes:

- **A1** is the prize. Everything else exists to protect A1.
- **A2** in the Keychain is the root of trust. Compromise → all historical
  A1 readable from disk.
- **A8** (timestamps, item length, type) leaks even when A1 is encrypted —
  see §7 for the side-channel limitation.

---

## 3. Trust Boundaries

```
                          USER
                            │
                            │  (clipboard via OS APIs)
                            ▼
   ┌──────────────────────────────────────────────────────┐
   │  Host OS process boundary                             │
   │                                                       │
   │   ┌──────────────┐  Unix socket  ┌──────────────┐    │
   │   │  Tauri UI    │ ◄──IPC──────► │   Daemon     │    │
   │   │  process     │   (boundary)  │  process     │    │
   │   └──────────────┘   ADR-002     └──────┬───────┘    │
   │                                          │            │
   │                          ┌───────────────┼────────┐  │
   │                          │               │        │  │
   │                          ▼               ▼        ▼  │
   │              ┌─────────────────┐  ┌──────────┐  ┌──────────────┐
   │              │ Keychain /      │  │ SQLCipher│  │  Network     │
   │              │ Secret Service  │  │   DB     │  │  egress      │
   │              │  (A2,A3)        │  │  (A1*)   │  │              │
   │              └─────────────────┘  └──────────┘  └──────┬───────┘
   │                                                         │
   └─────────────────────────────────────────────────────────┼─┘
                                                             │
                              ┌──────────────────────────────┼────────┐
                              │  Trust boundary: network     │        │
                              ▼                              ▼        │
                       ┌─────────────┐                ┌─────────────┐ │
                       │ Peer device │  ◄── mTLS ──►  │   Relay     │ │
                       │ (daemon)    │     (LAN)      │  (HTTPS)    │ │
                       └─────────────┘                └─────────────┘ │
                                                             ▲        │
                                                             │ HTTPS  │
                                                             └────────┘
```

### 3.1 Boundary inventory

| #   | Boundary                          | Authentication                  | Confidentiality                |
| --- | --------------------------------- | ------------------------------- | ------------------------------ |
| TB1 | User → UI (window + input)        | OS session                      | Pixel buffer (OS-managed)      |
| TB2 | UI ↔ Daemon (Unix socket)         | filesystem perms `chmod 0600`   | Local-only, no network         |
| TB3 | Daemon ↔ Keychain                 | OS access-control list          | Kernel-mediated                |
| TB4 | Daemon ↔ SQLCipher file           | filesystem perms + AES-256-CBC  | At-rest AES-256 (ADR-003)      |
| TB5 | Daemon ↔ Peer daemon (LAN)        | mDNS discovery + mTLS           | XChaCha20-Poly1305 envelope    |
| TB6 | Daemon ↔ Relay (WAN)              | bearer token + HTTPS            | TLS + XChaCha20-Poly1305       |
| TB7 | Pairing handshake (PAKE)          | shared short password           | OPAQUE-KE (ADR-008)            |

---

## 4. Threats (STRIDE per Asset)

### 4.1 A1 — Plaintext clipboard items

| #     | STRIDE | Threat                                                      | Mitigation                                                                      |
| ----- | ------ | ----------------------------------------------------------- | ------------------------------------------------------------------------------- |
| T1.S  | S      | UI process spoofed by another local process over Unix sock  | Socket `chmod 0600`, daemon checks peer UID via `SO_PEERCRED` (ADR-002)         |
| T1.T  | T      | Relay or MITM swaps a stored ciphertext for another         | AAD binds `(device_id, item_id, content_hash, schema_version)`; AEAD detects   |
| T1.R  | R      | Cannot tell which device pushed an item ("who pasted that?")| `origin_device_id` is part of merge tiebreak and stored alongside ciphertext   |
| T1.I  | I      | Clipboard read by another local process via OS clipboard API| Out of scope — OS-level concern; we recommend OS clipboard managers be denied  |
| T1.I2 | I      | DB file copied off disk by another local user               | SQLCipher AES-256-CBC w/ key from Keychain (ADR-003)                            |
| T1.D  | D      | Daemon memory exhaustion via giant clipboard items          | Item size cap enforced in capture pipeline; reject > MAX_ITEM_BYTES             |
| T1.E  | E      | UI process gains daemon-level perms via socket commands     | IPC schema-versioned (ADR-007); commands restricted to read/push/delete only    |

### 4.2 A2 — SQLCipher DB master key (`db_key`)

| #     | STRIDE | Threat                                                              | Mitigation                                                                |
| ----- | ------ | ------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| T2.S  | S      | Another app pretends to be daemon, asks Keychain for `db_key`       | Keychain entry scoped to daemon's code-signed identity (macOS) / app UID  |
| T2.T  | T      | Key replaced on disk-resident Keychain backup (macOS `login.keychain`) | Keychain ACL on entry; **see open issue OI-4 below**                       |
| T2.R  | R      | Key rotated without audit log                                       | Key-rotation events logged to daemon log (no plaintext key, just rotation event id) |
| T2.I  | I      | Key dumped from Keychain by malware running as user                 | Out of scope — OS compromise                                              |
| T2.D  | D      | Keychain unavailable → daemon can't open DB                         | Daemon refuses to start without `db_key`; user re-paired flow             |
| T2.E  | E      | Non-daemon process reads key via Keychain prompt                    | macOS: code-signing identity binding (ADR-010)                            |

### 4.3 A3 — Device identity private key (mTLS)

| #     | STRIDE | Threat                                                | Mitigation                                                                  |
| ----- | ------ | ----------------------------------------------------- | --------------------------------------------------------------------------- |
| T3.S  | S      | Attacker impersonates this device to a paired peer    | Peer's TLS stack verifies cert against pinned `device_id`; rejects on mismatch |
| T3.T  | T      | Cert swap during mDNS discovery                       | mTLS terminates *before* trusting discovery; SAN must match                 |
| T3.R  | R      | Device id reused across reinstalls without revocation | Re-pair required after reinstall; old cert stays in peer's trust store     |
| T3.I  | I      | Private key extracted from Keychain                   | Same as T2.I — out of scope                                                 |
| T3.D  | D      | Cert expired → no sync                                 | Long-lived cert (5 yr); warning at 6 mo prior to expiry                    |
| T3.E  | E      | Cert used outside mTLS (e.g., signing clipboard items)| Cert scoped via EKU = clientAuth only                                       |

### 4.4 A4 — OPAQUE-KE session keys (post-pair)

| #     | STRIDE | Threat                                                       | Mitigation                                                              |
| ----- | ------ | ------------------------------------------------------------ | ----------------------------------------------------------------------- |
| T4.S  | S      | Attacker injects derived session key from offline crack       | OPAQUE-KE is augmented PAKE — no offline brute possible (ADR-008)       |
| T4.T  | T      | Session key downgraded by relay                              | Keys are local-only; relay never sees them                              |
| T4.R  | R      | Cannot trace which pairing produced which session key        | Session keys tagged with `pairing_id`; logged                           |
| T4.I  | I      | Session key persisted to disk in plaintext                   | Persisted only inside SQLCipher DB (A2 protects)                        |
| T4.D  | D      | Session key rotated mid-sync, peer can't decrypt             | Rotation handled by re-handshake; AEAD failure triggers re-handshake     |
| T4.E  | E      | Use of session key for non-sync purposes                     | Domain-separated KDF labels per usage                                   |

### 4.5 A5 — Pairing PAKE password (ephemeral)

| #     | STRIDE | Threat                                                          | Mitigation                                                                |
| ----- | ------ | --------------------------------------------------------------- | ------------------------------------------------------------------------- |
| T5.S  | S      | Attacker observes user typing 6-digit code, impersonates peer   | Code expires in ≤60s and is single-use; OPAQUE-KE binds to a specific peer cert at completion |
| T5.T  | T      | Active MITM swaps password during transmission                  | Password is never transmitted (OPAQUE-KE) — only blinded values           |
| T5.R  | R      | User claims they didn't initiate pairing                        | Pairing UI requires explicit confirmation on **both** devices             |
| T5.I  | I      | Password shoulder-surfed                                        | User responsibility; we recommend `4-word` passphrase                     |
| T5.D  | D      | Adversary spams pairing attempts → user fatigue                 | Rate-limit pairing attempts; lock-out after 5 failures / 1 min            |
| T5.E  | E      | Code reused → cross-account compromise                          | Single-use; nonce derived from code burned after one attempt              |

### 4.6 A6 — Encrypted clipboard items on relay

| #     | STRIDE | Threat                                                  | Mitigation                                                                              |
| ----- | ------ | ------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| T6.S  | S      | Attacker pretends to be device, retrieves inbox         | Bearer token per device (ADR-009); rotated on re-pair                                   |
| T6.T  | T      | Relay swaps ciphertext between devices' inboxes         | AAD binds destination `device_id`; recipient AEAD-fails on mismatched AAD               |
| T6.R  | R      | Sender denies pushing item                              | Relay logs `(push_ts, sender_token_id, item_id)`; sender-bound bearer token             |
| T6.I  | I      | Operator reads inbox plaintext                          | XChaCha20-Poly1305; relay never has keys                                                 |
| T6.D  | D      | Inbox flooded → memory exhaustion                       | Per-device inbox size cap + TTL eviction (ADR-009); rate-limit pushes per token         |
| T6.E  | E      | Relay endpoint used for arbitrary storage               | Push/pull endpoints validate AAD shape; reject malformed                                |

### 4.7 A7 — Device identity (UniFFI device id)

| #     | STRIDE | Threat                                                       | Mitigation                                                |
| ----- | ------ | ------------------------------------------------------------ | --------------------------------------------------------- |
| T7.S  | S      | Two devices collide on `device_id`                           | UUIDv4; collision probability negligible                  |
| T7.T  | T      | Device id changed mid-sync                                   | `device_id` immutable; rotation = re-pair                 |
| T7.I  | I      | Device id leaked → relay tracks user                         | User accepts this trade-off when using relay              |
| T7.D  | D      | —                                                            | —                                                         |
| T7.E  | E      | —                                                            | —                                                         |

### 4.8 A8 — Clipboard metadata (timestamps, lengths, types)

| #     | STRIDE | Threat                                                        | Mitigation                                                          |
| ----- | ------ | ------------------------------------------------------------- | ------------------------------------------------------------------- |
| T8.I  | I      | Relay infers usage patterns from push frequency / sizes       | **Not fully mitigated** — see open issue OI-5 (traffic-analysis)    |
| T8.I2 | I      | DB columns leak metadata if SQLCipher key compromised         | Metadata co-located with ciphertext; protected by A2                |
| T8.T  | T      | Timestamp manipulated to reorder clipboard history            | Merge tiebreak uses `(timestamp, origin_device_id)`; deterministic  |

### 4.9 A9 — mDNS discovery records

| #     | STRIDE | Threat                                                        | Mitigation                                                            |
| ----- | ------ | ------------------------------------------------------------- | --------------------------------------------------------------------- |
| T9.S  | S      | Rogue device advertises matching service name                 | mTLS handshake fails — discovery is only a hint, never a trust source |
| T9.D  | D      | mDNS flood from LAN attacker                                  | **Open issue OI-3** — no rate-limit yet                               |
| T9.I  | I      | Adversary on LAN learns CopyPaste is running                  | Accepted — feature requires advertising presence                      |

---

## 5. Mitigations — Summary Table

| Threat ID(s)         | Mitigation                                              | Reference                                |
| -------------------- | ------------------------------------------------------- | ---------------------------------------- |
| T1.S, T1.E           | Unix socket `chmod 0600`, peer UID check                | ADR-002, `daemon/src/ipc.rs`             |
| T1.T, T6.T           | AAD binds `(device_id, item_id, content_hash, schema_version)` | ADR-001 + AAD work in beta-w3            |
| T1.R                 | `origin_device_id` in merge tiebreak + push record       | `core/src/merge.rs` (tiebreak fix)       |
| T1.I2, T2.*          | SQLCipher AES-256-CBC, key from Keychain                | ADR-003                                  |
| T1.D, T6.D           | Item/inbox size caps, push rate-limit                   | ADR-009, `relay/src/limits.rs`           |
| T3.*, TB5            | mTLS with pinned `device_id` SAN                        | `core/src/sync/mtls.rs`                  |
| T4.*, T5.*, TB7      | OPAQUE-KE pairing handshake                             | ADR-008, `core/src/pairing/`             |
| T5.D                 | Pairing rate-limit + lockout                            | `daemon/src/pairing.rs`                  |
| T6.*                 | Bearer-token per device, AAD-bound ciphertext, TTL evict| ADR-009, `relay/src/storage.rs`          |
| T8.T                 | Deterministic merge w/ `origin_device_id` tiebreak      | `core/src/merge.rs`                      |
| T9.S                 | mDNS treated as hint; mTLS is sole trust anchor          | `core/src/discovery/`                    |
| Code integrity       | Ad-hoc code signing (macOS) / signed APK (Android)      | ADR-010                                  |
| Local data integrity | WAL + checksums                                         | ADR-004                                  |
| UI tampering         | Tauri assets compiled-in, no remote code loading        | ADR-013                                  |
| IPC schema drift     | Versioned IPC envelope, server rejects unknown ops      | ADR-007                                  |

---

## 6. Out of Scope (Explicit Reaffirmation)

The following threats are **acknowledged but unaddressed** at the application
layer. Users must rely on platform mitigations:

1. **Full-disk compromise while unlocked.** If a user mounts the disk on
   another OS with FileVault/BitLocker unlocked, SQLCipher still protects A1
   (separate key in Keychain), but A2 is in the Keychain backup. Rely on
   Keychain ACLs and OS account separation.
2. **Malicious peer post-pairing.** Once two devices complete OPAQUE-KE,
   they are mutually trusted (TOFU). A compromised peer can exfiltrate
   *future* clipboard items. **Revocation flow is a TODO** (see OI-2).
3. **Kernel exploits.** Out of scope.
4. **Memory dump of running daemon.** Plaintext A1 is briefly in daemon RAM
   during capture/relay encode. No mitigation beyond OS process isolation.
5. **Side-channel attacks** on `chacha20poly1305` / `opaque-ke` crates.
6. **Social engineering** of the user during pairing.
7. **Compromised TLS root store** affecting relay HTTPS.
8. **Adversary controls user's display.** Pairing-code confirmation
   assumes the user can read both screens.

---

## 7. Open Issues (Severity-Tagged)

| ID    | Severity   | Description                                                                                                 | Tracking                          |
| ----- | ---------- | ----------------------------------------------------------------------------------------------------------- | --------------------------------- |
| OI-1  | **LOW**    | `SCHEMA_VERSION` constant not `pub` — purely an API-cleanup item, no security impact, in flight.            | Beta-w3 cleanup branch            |
| OI-2  | **HIGH**   | No peer-revocation flow. Once paired, a peer remains trusted until the user manually deletes the entry.     | Stable milestone (TODO before 1.0)|
| OI-3  | **MEDIUM** | mDNS advertisement spam susceptibility — no per-source rate-limit on incoming announcements; a LAN attacker can DoS discovery. | Required before stable             |
| OI-4  | **MEDIUM** | macOS Keychain ACL is **not enforced** for the backup-file recovery path (`~/Library/Keychains/login.keychain-db` copied off disk). User must enable FileVault. | Document in user-facing security guide |
| OI-5  | **LOW**    | Traffic-analysis on relay reveals push frequency, item-size buckets, and online/offline cadence per device. Acceptable for beta; full mitigation requires padding + cover traffic. | Post-stable feature                |
| OI-6  | **MEDIUM** | Sentry telemetry must be **opt-in only** and must scrub all clipboard content and identifying paths. Currently opt-in by default; we need a CI test that asserts no A1/A2/A3 paths appear in event payloads. | Beta-w4 testing pass               |
| OI-7  | **LOW**    | No protection against **clipboard history exfiltration via OS-level clipboard managers** (e.g. another installed app subscribing to pasteboard changes). Documented as user-OS responsibility. | User-docs only                     |
| OI-8  | **LOW**    | Daemon log files may include `device_id` (A7) and `item_id` — fine at A7's sensitivity, but logs should rotate + cap size to avoid disk-fill DoS. | Beta-w4                            |
| OI-9  | **MEDIUM** | No formal review of `unsafe` blocks in `copypaste-core` and `copypaste-daemon` for the beta cut.            | Required before stable             |
| OI-10 | **LOW**    | UniFFI bindings boundary for Android — verify no panics cross into JVM (would abort process and expose stack trace in `adb logcat`). | Android beta-w4 hardening         |

---

## 8. Disclosure Policy

We take security reports seriously and prefer **coordinated, private
disclosure**.

### 8.1 How to report

**Preferred:** open a security advisory on the GitHub repository
(maintainer-only visibility) using the security-issue template:

- Template path: `.github/ISSUE_TEMPLATE/security_report.md` (added in
  parallel beta-w3 work — see sibling task `beta-security-template`).

**Email fallback (private):**
`security@copypaste.invalid` (placeholder — to be configured before beta
release; the working address is `dmitriy.evseev.99@gmail.com` until a
dedicated alias exists).

PGP key: **TBD** — to be published in `docs/security/pgp-key.asc` before
v0.2.0-beta tag.

### 8.2 What to include

- Affected version (`copypaste --version` output).
- Affected component (daemon / UI / relay / Android).
- Reproduction steps.
- Suggested severity (CVSS optional).
- Whether you'd like credit in the changelog.

### 8.3 Our commitments

- Acknowledge within **72 hours**.
- Provide a remediation timeline within **7 days**.
- Coordinate public disclosure (default embargo: **90 days**, negotiable for
  high-severity issues affecting deployed users).
- Credit reporters in the changelog unless anonymity is requested.

### 8.4 Safe harbor

Good-faith security research that:

- Does **not** exfiltrate user data beyond what is necessary to demonstrate
  the issue,
- Does **not** degrade service for other users,
- Respects this disclosure policy,

is welcome. We will not pursue legal action against researchers acting in
good faith.

---

## 9. Change Log

| Date       | Version       | Change                                              |
| ---------- | ------------- | --------------------------------------------------- |
| 2026-05-23 | 0.2.0-beta-1  | Initial threat model. STRIDE per asset.             |

---

## 10. Review Triggers

This document **must** be reviewed when any of the following occur:

- New ADR is accepted that touches encryption, IPC, or sync.
- New trust boundary is introduced (e.g., browser extension, web UI).
- A new asset class is added (e.g., file attachments, OAuth tokens).
- Any open issue (§7) is closed.
- Before tagging a new minor release.

---

## 11. Adversary Profiles

We model three adversary tiers, in order of increasing capability. Mitigation
goals scale accordingly: every threat in §4 must be defeated against T1 and
T2; T3 is best-effort.

### 11.1 T1 — Passive network observer

- **Capabilities:** Read all traffic between daemon and relay, daemon and
  peer. Cannot modify, drop, or inject. No host access.
- **Goal:** Recover any A1 (plaintext clipboard item).
- **Defeat condition:** XChaCha20-Poly1305 over both LAN (TB5) and relay
  (TB6) paths; HTTPS provides additional layer at TB6.
- **Status:** Defeated.

### 11.2 T2 — Active network attacker (MITM)

- **Capabilities:** All of T1, plus modify, drop, replay, reorder traffic;
  operate a malicious relay; ARP/DNS-spoof on LAN; advertise rogue mDNS
  services.
- **Goal:** Recover A1, or trick a device into accepting attacker-supplied
  content as if from a trusted peer.
- **Defeat condition:** AAD-bound AEAD (per T1.T, T6.T), mTLS with pinned
  `device_id` SAN (T3.S, T3.T), OPAQUE-KE rejects MITM during pairing
  (T5.T). mDNS rogue services fail mTLS handshake (T9.S).
- **Status:** Defeated for established peers. Pairing window is the most
  sensitive moment — protected by OPAQUE-KE single-use code.

### 11.3 T3 — Malicious paired peer

- **Capabilities:** All of T2, plus full possession of one valid `device_id`
  + session-key pair (e.g., user's old laptop was stolen post-pairing).
- **Goal:** Continue receiving the victim's future clipboard items.
- **Defeat condition:** **Currently undefeated.** TOFU model with no
  revocation flow — see OI-2.
- **Workaround until OI-2 ships:** User must manually delete the peer entry
  from the trusted-peers list (`copypaste peer rm <device_id>`).

### 11.4 T4 — Local malware (out of scope)

- **Capabilities:** Code execution as the user.
- **Status:** Out of scope (§6).
- **Why:** Once malware runs as the user, it can read the OS clipboard
  directly, dump daemon memory, or coerce the Keychain prompt. No
  application-layer mitigation is meaningful.

---

## 12. Cryptographic Primitives — Quick Reference

| Use                              | Primitive                  | Library                   | ADR     |
| -------------------------------- | -------------------------- | ------------------------- | ------- |
| Clipboard envelope (at rest + in transit) | XChaCha20-Poly1305 AEAD    | `chacha20poly1305` crate  | ADR-001 |
| DB file encryption               | AES-256-CBC + HMAC-SHA256  | SQLCipher (`rusqlite`)    | ADR-003 |
| Device pairing                   | OPAQUE-KE (augmented PAKE) | `opaque-ke` crate         | ADR-008 |
| Peer mutual auth                 | mTLS 1.3, P-256 ECDSA      | `rustls`                  | (TB5)   |
| Key derivation                   | HKDF-SHA256                | `hkdf` crate              | —       |
| Random                           | OS CSPRNG (`getrandom`)    | `rand_core::OsRng`        | —       |

All AEAD nonces are random 192-bit (XChaCha20) — see ADR-001 rationale for
why birthday-bound risk is negligible at this nonce width.

End of document.
