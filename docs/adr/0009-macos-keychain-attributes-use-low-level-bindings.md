# ADR-0009 — Keep macOS Keychain compatible with Homebrew Cask

**Status:** accepted; supersedes the 2026-08-01 low-level-binding decision
**Scope:** writing the macOS device secret as a generic-password item.

## Context

The device secret must live in the login Keychain, but ADR-0001 makes our own
Homebrew Cask with a local self-signed identity the only macOS distribution
model. A Data Protection Keychain item created with `SecAccessControl` failed
with `errSecMissingEntitlement` on the release-equivalent macOS runner. Those
entitlements depend on Apple-managed signing and provisioning, which this
project explicitly does not use.

## Decision

Use `security-framework`'s generic-password API in the traditional login
Keychain. Preserve the frozen service/account pair and fail closed on every
read error except `errSecItemNotFound`. Do not add Keychain attributes or
entitlements that require an Apple team or provisioning profile.

## Consequences

The secret remains outside the application data directory and is protected by
the user's login Keychain. It does not claim device-only backup or iCloud-sync
semantics. Release compatibility with the only supported macOS distribution
path takes precedence over attributes that require Apple-managed signing.
