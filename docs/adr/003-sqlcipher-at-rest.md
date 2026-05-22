# ADR-003: SQLCipher for database at-rest encryption

**Date:** 2026-05-22  
**Status:** In progress (Phase 2c)

## Context

Clipboard history contains potentially sensitive data. The SQLite database should be encrypted on disk.

## Decision

Use SQLCipher via `rusqlite`'s `bundled-sqlcipher` feature.

## Rationale

1. **Transparent encryption** — existing SQL queries work unchanged
2. **AES-256-CBC** — well-established, audited encryption
3. **Key source** — 32-byte key from OS keychain (macOS Keychain / Windows DPAPI / Linux Secret Service), never stored in plaintext
4. **bundled feature** — no system libsqlcipher dependency, compiles on all target platforms
5. **Migration path** — `sqlcipher_export()` SQL function for encrypting existing databases atomically

## Consequences

- Compile time increases (bundled OpenSSL for SQLCipher)
- `Database::open()` signature changes to accept key parameter
- Tests use fixed test key (`[0u8; 32]`)
