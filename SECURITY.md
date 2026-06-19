# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| main branch | ✅ Active |
| Tagged releases | ✅ Latest only |

## Reporting a Vulnerability

**Do NOT open a public GitHub issue for security vulnerabilities.**

Report privately via email: security@copypaste.app

Include:
- Description of the vulnerability
- Steps to reproduce
- Affected component (core/daemon/relay/cli)
- Potential impact

Expected response: acknowledgement within 48 hours, fix timeline within 14 days for critical issues.

## Security Architecture

### Encryption
- Clipboard items encrypted with **XChaCha20-Poly1305** before storage
- Key derivation via **HKDF-SHA256** from X25519 ECDH shared secret
- Local-only key derived from device secret via HKDF (`copypaste-local-storage-v1`)
- Database at rest encrypted with **SQLCipher (AES-256-CBC)**

### Key Storage
- macOS: **Keychain Services** (service: `com.copypaste.daemon`, `ThisDeviceOnly` accessibility)
- Windows: **not implemented** — Windows support is frozen (ADR-012); the daemon uses an
  ephemeral in-memory key that is lost on restart.
- Linux: **not implemented** — the daemon uses an ephemeral in-memory key that is lost on
  restart. A Secret Service integration (GNOME Keyring / KWallet) is planned but not shipped.

### Sensitive Data Detection
- 20+ pattern types detected (AWS, GitHub, Stripe, OpenAI, JWT, SSH, etc.)
- Sensitive items get automatic TTL expiry
- Sensitive items **are synced encrypted** via relay/cloud/P2P; sensitivity is
  re-evaluated by the receiving daemon. Items never leave any device in plaintext.

### Relay Server
- Relay stores **only ciphertext** — never has decryption keys
- End-to-end encrypted: relay cannot read clipboard content
- Bearer token auth: **random 16-byte token** (`OsRng`) encoded as 32 hex characters,
  issued at registration — it is NOT derived from the public key or any other secret.
  Tokens are compared constant-time via `subtle::ct_eq` (no timing oracle).
- Per-device inbox with 500-item hard quota (oldest pruned on overflow)

### Known Limitations
- Clipboard monitoring requires accessibility permissions on macOS
- Android 10+ clipboard access restricted to foreground apps
- Windows key storage is ephemeral (process-restart loses the key); Windows is frozen (ADR-012)
- Linux key storage is ephemeral (process-restart loses the key); Secret Service integration planned

## Dependency Auditing

```bash
cargo deny check  # requires cargo-deny
cargo audit       # requires cargo-audit
```
