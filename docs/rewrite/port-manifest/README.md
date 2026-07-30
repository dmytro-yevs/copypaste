# Port manifests — what is binding and what is reference

~9,100 lines harvested from the v0.4.1 implementation and its tests: roughly 500
acceptance tests and 200+ recovered bug IDs.

v2 **drops backward compatibility** with v0.4.x (CLAUDE.md rule 3). That changes
what these documents are for, but far less than it might appear. The formats
stop being contracts. The behaviour does not — a bug that someone found in
production is still a bug in a clean-slate implementation, and rediscovering it
costs exactly as much the second time.

Read this file before treating any manifest section as a requirement.

## Per manifest

| Manifest | Binding | Now reference only |
|---|---|---|
| **01 clipboard-capture** | Nearly all of it. NSPasteboard quirks, `changeCount` and self-write suppression, `org.nspasteboard.*` privacy markers, the `NSFilenamesPboardType` binary plist, burst handling, frontmost-app cache TTL, the 84 acceptance tests | Nothing meaningful — this manifest is about macOS, not about our formats |
| **02 crypto** | Security properties: fail-closed on wrong key/AAD/version, AAD must bind item identity, zeroization, constant-time comparison, keychain service naming | Verbatim HKDF info strings, AAD byte layouts, `CHUNK_FORMAT_V1` framing, the QR envelope, `key_version` dispatch, the v1-key-is-the-seed quirk, the repair sweep |
| **03 storage** | Sensitive-never-in-FTS (all three enforcement layers), tombstone and pinning semantics, TTL/cap eviction, the keyset-pagination ordering contract, SQLCipher raw-key usage | The whole v1→v15 migration ladder, per-version idempotency notes, the `(wall_time / 60)` dedup bucket, `migration_state` |
| **04 ipc-protocol** | The method catalogue as a *feature* inventory, error-code taxonomy, readiness/degraded-mode semantics, the rule that errors never leak paths | Exact JSON envelope, protocol-version negotiation, legacy verb behaviour, wire-compatibility requirements |
| **05 sync-and-backend** | Merge ordering and its tie-breaks, idempotency and replay safety, delete-wins, clock-skew handling, the relay→Supabase parity checklist, adaptive idle backoff | On-wire framing, relay-specific endpoint shapes |
| **06 ui-behaviour** | Nearly all of it. Scroll anchoring, row-height over-reservation, the dedup/signature contract, 15 accessibility requirements, error copy, SAS flow, design tokens, 73 acceptance tests | Nothing meaningful — this manifest is about behaviour, not storage |
| **07 secret-detection** | Nearly all of it. The full ruleset, confidence model and auto-wipe floor, false-positive defences, NFKC normalisation, Luhn validation | Nothing meaningful |

Roughly: **manifests 02 and 03 shrink** from byte-exact contracts to design
references. **01, 06 and 07 are untouched** by the compatibility decision — they
were always about behaviour. **04 and 05 lose their wire constraints** but keep
their semantics.

## What dropping compatibility does not license

The manifests' "looks gratuitous but is load-bearing" sections still apply to
everything in the Binding column. A clean slate removes the obligation to
reproduce old *bytes*; it does not remove the reasons behind the rules. Three
worth restating, because each was a real defect and each is easy to reintroduce:

- Burst handling must not advance the change-count cursor without capturing the
  content. v1 did, and copying several things quickly permanently lost the most
  recent item.
- Frontmost-app lookup must not be skipped when the exclusion list is empty.
  Empty is the default, and skipping it meant password-manager content was never
  flagged sensitive.
- Row heights must over-reserve. The "smarter" character-count estimate was
  itself the bug.

## If a manifest and this file disagree

This file wins on *scope*; the manifest wins on *fact*. If you think a manifest
is factually wrong, change it in the same commit as the code, and say so — do
not silently build something that contradicts it.
