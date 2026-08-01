# ADR-0009 — macOS Keychain attributes use low-level bindings

**Status:** accepted · 2026-08-01
**Scope:** writing the macOS device secret as a generic-password item.

## Context

Port manifest 02 requires `kSecAttrAccessControl` with
`kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, explicit
`kSecAttrSynchronizable=false`, and the same attributes on `SecItemUpdate`.

The current maintained `security-framework` 3.7 API does not provide that
operation. `PasswordOptions` can add both attributes, but its duplicate path
puts every option in the update search and only the password data in the
attributes-to-update dictionary. `ItemUpdateOptions` exposes neither access
control nor synchronizability. Apple requires changed attributes in
`SecItemUpdate`'s second dictionary.

Sources evaluated:

- <https://docs.rs/security-framework/3.7.0/security_framework/passwords/struct.PasswordOptions.html>
- <https://docs.rs/security-framework/3.7.0/security_framework/item/struct.ItemUpdateOptions.html>
- <https://docs.rs/security-framework/3.7.0/src/security_framework/passwords.rs.html>
- <https://developer.apple.com/documentation/security/secitemupdate(_:_:)>

## Decision

Use CLAUDE.md dependency exemption 1: no maintained high-level package
provides the required update. Keep `security-framework` for access-control
construction and reads, and use its maintained `security-framework-sys`
bindings plus `core-foundation` in one macOS-only support crate.

The core keystore owns the frozen device-secret service/account pair and its
32-byte type boundary. The support crate builds one add dictionary and, on
`errSecDuplicateItem`, one identity query plus one update dictionary carrying
the data and both security attributes. It exists separately because
`copypaste-core` forbids unsafe code. Tests use the high-level attribute search;
one test-only declaration supplies `kSecAttrAccessible`, which
`security-framework-sys` 2.17 omits.

## Consequences

No second crypto or TLS stack enters the tree. The support crate compiles empty
off macOS, and its unsafe surface is limited to `SecItemAdd`, `SecItemUpdate`,
constant wrapping, and the test-only missing constant. Real-Keychain tests read
the accessibility and synchronizability values after both creation and a
legacy-item update.
