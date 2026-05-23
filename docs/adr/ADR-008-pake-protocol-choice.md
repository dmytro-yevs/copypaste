# ADR-008: PAKE Protocol Choice for Device Pairing

**Status:** Accepted
**Date:** 2026-05-23
**Deciders:** Project owner
**Related:** ADR-001 (XChaCha20-Poly1305 envelope), ADR-003 (SQLCipher at rest)

## Context

Beta milestone introduces device pairing via a short user-typed code (e.g.
`6-digit` or `4-word` passphrase). Naively comparing the code on both sides
leaks it to a network-active attacker. We need a **Password-Authenticated Key
Exchange (PAKE)** so two devices can derive a strong shared session key from a
weak shared password without ever transmitting the password (or anything
useful to a brute-forcer) over the wire.

Requirements:

1. Augmented PAKE preferred — server stores a password-file, not the plaintext
   password, so a one-shot DB exfiltration does not reveal the pairing code.
2. Pure-Rust impl that compiles on our workspace MSRV (1.75).
3. Active maintenance and ideally an external security audit.
4. Reasonable build-time / binary-size impact (we already ship rustls + ring +
   chacha20poly1305 + sqlcipher; we want to avoid pulling a second crypto
   stack).
5. Compatible with our existing `ring` / `curve25519-dalek` crypto stack.

## Decision

Adopt **`opaque-ke` 3.0.0** (Facebook, Apache-2.0 OR MIT, MSRV 1.74).

OPAQUE is an aPAKE standardised by the CFRG (RFC draft
`draft-irtf-cfrg-opaque`). The `opaque-ke` crate is the reference Rust
implementation, has had multiple external audits (NCC Group 2021, Trail of
Bits 2022 on related VOPRF), and is used in production by WhatsApp's E2E key
backup. Default ciphersuite is `Ristretto255 + SHA-512 + Argon2 + HKDF`, all
of which we already trust transitively.

Pinned version: `opaque-ke = "3.0.0"`. We deliberately avoid `4.1.0-pre.2`
until it goes stable. We use `default-features = false` and enable only
`ristretto255-voprf`, `argon2`, and `std` to keep the dep tree minimal.

## Alternatives Considered

| Crate | Verdict | Reason |
|-------|---------|--------|
| `opaque-ke` 3.0.0 | **Chosen** | Audited, RFC-aligned aPAKE, MSRV matches |
| `opaque-ke` 4.x-pre | Rejected | Pre-release; will revisit when stable |
| `srp` (RFC 5054 SRP6a) | Rejected | Symmetric PAKE — both sides hold password-equivalent material; pre-quantum design from 1998; weaker security proof |
| `password-auth` / `argon2` only | Rejected | Not a PAKE — just password hashing; offers no protocol-level protection against active MITM |
| `spake2` | Rejected | Symmetric (balanced) PAKE; suitable for short-lived codes but no augmented variant — server compromise reveals pairing code |
| Rolling our own (HKDF + ECDH + MAC) | Rejected | High risk of subtle attacks (offline dictionary, replay, key-compromise impersonation); no audit |

## Trade-offs

**Pros:**
- Strong cryptographic guarantees: zero password leakage on either passive
  observation or full server-DB exfil.
- Resistant to pre-computation / rainbow-table attacks (per-user OPRF salt).
- Forward-secret session key (used as input to our XChaCha20-Poly1305
  envelope from ADR-001).

**Cons:**
- Pulls `voprf`, `curve25519-dalek`, `argon2` — adds ~150 KB to the daemon
  binary and ~30s to a clean release build. Acceptable.
- Three-message handshake (vs. two for SRP). Negligible latency over LAN.
- `Argon2` costs CPU/memory at handshake time (~50 ms on M-series Mac).
  Pairing is rare, so this is a feature, not a bug.
- API has heavy use of generics (`CipherSuite` trait); learning curve.

## Migration Path

If we ever need to swap PAKE crates (e.g. switch to a post-quantum hybrid
when `opaque-ke-hybrid` matures, or to a lighter PAKE on constrained Android
builds), the public API of `crates/copypaste-p2p/src/pake.rs`
(`PakeInitiator` / `PakeResponder` / `SessionKey`) is intentionally generic
and crate-agnostic. Internally we hide the `opaque_ke::*` types behind our
own newtypes so a replacement is a single-module change.

`PasswordFile` is versioned (first byte = version tag = `0x01`) so we can
ship both formats during a transition.

## Wire Format (Three-Message Handshake)

Frames are length-delimited via `tokio_util::codec::LengthDelimitedCodec`
(same framing already used by `PeerTransport`).

```
Client                                              Server
  | --- 1. RegistrationRequest / CredentialRequest -> |
  | <-- 2. RegistrationResponse / CredentialResponse - |
  | --- 3. RegistrationUpload  / CredentialFinalize -> |
  |                                                    |
  | == both sides now hold the same 32-byte SessionKey |
```

Steps:

1. **Client → Server:** client blinds its password with a random scalar and
   sends the blinded element + a fresh ephemeral X25519 public key.
2. **Server → Client:** server runs OPRF evaluation on the blinded element
   using its per-user `PasswordFile`, returns the OPRF result + server's
   ephemeral X25519 public key + an envelope encrypted under the OPRF output.
3. **Client → Server:** client unblinds, derives the OPRF output locally,
   decrypts the envelope (recovers the long-term key material), completes the
   AKE, and sends its final authentication tag.

Output: 32-byte `SessionKey` on both sides, used as the seed for HKDF →
XChaCha20-Poly1305 key (per ADR-001).

## Storage

- **Server side** (`PasswordFile`): persisted in **SQLCipher** (ADR-003) in
  the `paired_peers` table, column `pake_password_file BLOB`. Per-peer row;
  rotated whenever the user re-pairs.
- **Pairing code** (the short password): **never** written to disk on either
  side. Held in `zeroize::Zeroizing<String>` for the duration of the
  handshake, then dropped.
- **Long-term identity key** (recovered from the envelope in step 3): stored
  in the macOS **Keychain** under `service = com.copypaste.pake`, never in
  the SQLCipher DB. Android equivalent uses the Android Keystore.

## Consequences

- New workspace dep: `opaque-ke = "3.0.0"` with default features off.
- `crates/copypaste-p2p/src/pake.rs` exposes the `PakeInitiator` /
  `PakeResponder` / `PasswordFile` / `SessionKey` API.
- Real handshake implementation lands in **Wave 2.4** (this ADR ships the
  skeleton + protocol decision only).
- `paired_peers` schema migration will add `pake_password_file BLOB NULL`
  in Wave 2.4 (separate ADR if migration is non-trivial).
- Integration tests in Wave 2.4 must include: (a) happy-path round-trip,
  (b) wrong-password rejection, (c) replay rejection, (d) MITM rejection.
