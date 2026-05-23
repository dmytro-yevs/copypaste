# ADR-001: XChaCha20-Poly1305 over AES-GCM for clipboard encryption

## Status

Accepted

Date: 2026-05-22

## Context

Clipboard items must be encrypted at rest (SQLite) and in transit (relay). We chose between AES-GCM and XChaCha20-Poly1305.

## Decision

Use XChaCha20-Poly1305 (specifically the XChaCha20 variant via `chacha20poly1305` crate).

## Rationale

1. **192-bit nonce** eliminates birthday bound risk when generating random nonces. AES-GCM uses 96-bit nonces — with random generation, collision probability becomes significant after ~2^32 encryptions.
2. **No AES-NI requirement** — portable across ARM (mobile) and x86 without hardware acceleration dependency.
3. **Nonce misuse resistance** — XChaCha20 derives a subkey from the nonce, providing better security if nonces are ever reused accidentally.
4. **Native Rust support** — `chacha20poly1305` crate is well-audited, no C bindings needed.

## Consequences

- Cannot use standard AES-GCM test vectors for compatibility testing
- 24-byte nonce stored per item vs 12-byte for AES-GCM (negligible overhead)
