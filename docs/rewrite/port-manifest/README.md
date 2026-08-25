# Port manifests — v2 requirements

These manifests preserve product behaviour and security properties that were
earned through production defects. They are requirements for the current
product, not a request to carry another product's files, protocols, commands or
visual design.

CopyPaste v2 has one database filename, one schema, one key derivation path and
one local IPC contract. It does not probe, open, decode, repair, migrate or
advertise support for any other format. A historical format is not retained here
as a decode-only path or a compatibility test. If a future migration or second
format is desired, it is a product feature requiring its own decision and tests.

## Per-manifest scope

| Manifest | Binding v2 contract | Excluded from v2 |
|---|---|---|
| **01 clipboard-capture** | Text capture, change detection, self-write suppression, privacy markers, burst handling, size gates, source-app sensitivity/exclusion, ingest, retention and failure posture | Image, file and rich-text capture until those become product features |
| **02 crypto** | One v2 key path; authenticated encryption; item-identity AAD; fresh nonces; zeroization; constant-time comparison; fail-closed keystore and decrypt behaviour | Alternate key generations, trial decrypt, format repair and decode-only crypto paths |
| **03 storage** | One v2 SQLCipher schema; sensitive-index exclusion; tombstones; pinning; TTL/quota eviction; total keyset pagination; atomic row/index updates | Schema ladders, encounter probes, plaintext conversion and data-format repair |
| **04 ipc-protocol** | One typed method catalogue; one response/error model; readiness gates; bounded local transport; path-redacted errors | Alias commands, retired envelopes, dual protocol serving and compatibility dispatch |
| **05 sync-and-backend** | Deterministic symmetric merge, total tie-breaks, idempotency, replay safety, delete-wins, clock-skew refusal, transport parity and adaptive idle backoff | Retired transport framing and endpoint shapes |
| **06 ui-behaviour** | Product behaviour, accessibility, scroll and row-height contracts, sensitive-content absence, error/empty states and pairing flow | Retired palette, spacing, typography, radius, elevation and translucency values |
| **07 secret-detection** | Ruleset, confidence model, auto-wipe floor, false-positive defences, normalization and validators | Nothing; credential format names describe real inputs, not application compatibility |

## Rules for applying a manifest

- Binding behaviour is independent of toolkit and platform implementation.
- Sensitive content must never reach the search index. This is enforced when
  writing, when reading and by a bounded purge that accounts for rules added
  after a v2 item was captured.
- Crypto failures never fall back to another key, AAD or plaintext path.
- Errors shown to a user never contain a filesystem path.
- Data loss is the worst outcome. A detector may remove an index entry at a
  lower confidence than it may delete the underlying item.
- Acceptance tests describe observable properties. Rename them to fit the v2
  API, but do not weaken the assertion.

The sections titled “looks gratuitous but is load-bearing” remain binding where
they explain current behaviour. A defect reference is evidence for a rule, not
permission to retain the implementation in which the defect occurred.

## When a manifest and implementation disagree

The manifest is the product requirement. Change it explicitly in the same
reviewed change when the requirement is wrong; do not silently implement a
different contract. Generated files and wire bindings are changed at their
authoring source, never by editing generated output.
