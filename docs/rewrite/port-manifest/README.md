# Port manifests — what is binding and what is reference

~9,100 lines harvested from the v0.4.1 implementation and its tests: roughly 500
acceptance tests and 200+ recovered bug IDs.

Two decisions have narrowed what these documents oblige. Both narrow *formats*
and *appearance*; neither narrows behaviour.

1. **v2 drops backward compatibility** with v0.4.x (CLAUDE.md rule 3). The
   on-disk and on-wire formats stop being contracts.
2. **v1's visual design is explicitly rejected.** The look is being redesigned,
   so manifest 06's *visual* sections stop being requirements. What the UI must
   *do* is unaffected. (The surface is one Tauri v2 + React app for both
   platforms — see [ADR-0002](../../adr/0002-one-cross-platform-app.md). An
   earlier revision of this file said the apps go native; that decision was
   reversed and the toolkit never affected which sections bind.)

The behaviour survives both, and that is most of the value here: a bug that
someone found in production is still a bug in a clean-slate implementation, and
rediscovering it costs exactly as much the second time.

Read this file before treating any manifest section as a requirement.

## Per manifest

| Manifest | Binding | Now reference only |
|---|---|---|
| **01 clipboard-capture** | Text-capture behaviour: pasteboard change detection and self-write suppression, privacy markers, burst handling, text size gates, frontmost-app sensitivity/exclusion, ingest, retention and failure posture | Image, file and rich-text capture. v2 is text-only; their pasteboard formats, extraction, metadata and binary ingest tests remain design reference for a future product decision |
| **02 crypto** | Security properties: fail-closed on wrong key/AAD/version, AAD must bind item identity, zeroization, constant-time comparison, keychain service naming | Verbatim HKDF info strings, AAD byte layouts, `CHUNK_FORMAT_V1` framing, the QR envelope, `key_version` dispatch, the v1-key-is-the-seed quirk, the repair sweep |
| **03 storage** | Sensitive-never-in-FTS (all three enforcement layers), tombstone and pinning semantics, TTL/cap eviction, the keyset-pagination ordering contract, SQLCipher raw-key usage | The whole v1→v15 migration ladder, per-version idempotency notes, the `(wall_time / 60)` dedup bucket, `migration_state` |
| **04 ipc-protocol** | The method catalogue as a *feature* inventory, error-code taxonomy, readiness/degraded-mode semantics, the rule that errors never leak paths | Exact JSON envelope, protocol-version negotiation, legacy verb behaviour, wire-compatibility requirements |
| **05 sync-and-backend** | That the merge is deterministic and symmetric with total tie-breaks, idempotency and replay safety, delete-wins, clock-skew handling, the relay→Supabase parity checklist, adaptive idle backoff | On-wire framing, relay-specific endpoint shapes, and the `lamport_ts` key itself: v2 has no Lamport clock and orders on `created_at → content_hash → deleted → origin_device_id`, one comparator shared by both transports. The *properties* the Lamport stamp bought are still binding — monotonicity is replaced by refusing implausibly-future stamps, not by dropping the concern |
| **06 ui-behaviour** | Everything about what the UI *does*: scroll anchoring and the shrink clamp, row heights reserving the full cap, the dedup/signature contract, the 15 accessibility requirements, sensitive content absent from the view rather than obscured over the top of it, no filesystem path in any user-facing error, error and empty-state semantics, the SAS pairing flow, the 73 acceptance tests | Everything about how it *looks*: §8's palette and token values, the spacing, type, radius, elevation and translucency scales, the accent set, and `design-reference.html`. v1's design is not being carried over |
| **07 secret-detection** | Nearly all of it. The full ruleset, confidence model and auto-wipe floor, false-positive defences, NFKC normalisation, Luhn validation | Nothing meaningful |

Roughly: **manifests 02 and 03 shrink** from byte-exact contracts to design
references. **04 and 05 lose their wire constraints** but keep their semantics.
**01 narrows to the text product and 07 is untouched.** **06 splits**:
its behaviour and accessibility half is binding in full, its visual half is
reference only.

Two cautions about that split, because it is the one most likely to be read too
broadly:

- **The toolkit does not get to drop the behaviour.** Scroll anchoring, the
  row-height rule, the accessibility contract and the sensitive-content rule are
  requirements of the *product*. Nothing satisfies them for free — a webview
  reaches them through DOM and ARIA where a native app would have used SwiftUI
  and TalkBack — and the manifest's acceptance tests still say what "satisfied"
  means either way.
- **"Visual is reference" is not "visual is undecided by default".** The new
  look is a decision that has not been taken yet. Until it is, do not treat
  §8's values as a fallback and do not invent replacements in passing — an
  implementation that quietly re-derives v1's palette is the outcome this
  decision exists to prevent.

## What these decisions do not license

The manifests' "looks gratuitous but is load-bearing" sections still apply to
everything in the Binding column. A clean slate removes the obligation to
reproduce old *bytes*, and a new design removes the obligation to reproduce old
*pixels*; neither removes the reasons behind the rules. Three worth restating,
because each was a real defect and each is easy to reintroduce:

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
