# ADR-0008 — Propagate sensitive deletes without their payload

**Status:** accepted · 2026-08-01
**Scope:** peer and cloud behaviour when automatic sensitive-content expiry
removes an item.

## Decision

A live sensitive item is never advertised, fetched, or uploaded. After the
auto-wipe gate has decrypted it and confirmed high confidence, the store
replaces it with a tombstone: no ciphertext, nonce, or plaintext-derived hash.
Its deletion stamp is strictly newer than the live version, and its item id and
origin remain. That tombstone is eligible for peer and cloud sync.

## Consequences

The delete reaches a device that already has an older copy, including one made
before the sensitivity classification changed. It never permits offline
guessing of a short secret from a transmitted hash. A peer that never held the
item stores an empty tombstone, preventing delayed delivery from resurrecting
it.
