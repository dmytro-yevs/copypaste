# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| main branch | ✅ Active |
| Tagged releases | ✅ Latest only |

## Reporting a Vulnerability

**Do NOT open a public GitHub issue for security vulnerabilities.**

Report privately via email: security@copypaste.app (placeholder — replace before publishing)

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
- macOS: **Keychain Services** (service: `com.copypaste.daemon`)
- Windows: **DPAPI** (`CryptProtectData`)
- Linux: **Secret Service API** (GNOME Keyring / KWallet)

### Sensitive Data Detection
- 20+ pattern types detected (AWS, GitHub, Stripe, OpenAI, JWT, SSH, etc.)
- Sensitive items get automatic TTL expiry
- Sensitive items **never synced** to relay server

### Relay Server
- Relay stores **only ciphertext** — never has decryption keys
- End-to-end encrypted: relay cannot read clipboard content
- Bearer token auth (Phase 2b: SHA-256 of public key — TODO: replace with signed challenges in Phase 3)
- Per-device inbox with 500-item quota

### Known Limitations
- Relay bearer tokens use string equality comparison (not constant-time) — tracked for fix in Relay v2
- Clipboard monitoring requires accessibility permissions on macOS
- Android 10+ clipboard access restricted to foreground apps

## Dependency Auditing

```bash
cargo deny check  # requires cargo-deny
cargo audit       # requires cargo-audit
```
