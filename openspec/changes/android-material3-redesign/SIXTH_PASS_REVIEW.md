# Sixth-pass critical review — post `FULL_DEEP_AUDIT`

Date: 2026-07-02  
Scope: `proposal.md`, `design.md`, `tasks.md`, all 14 capability specs, inventories, and the
correction requirements from `FULL_DEEP_AUDIT.md`.  
Validation: `openspec validate android-material3-redesign --strict` — **valid**.

## Verdict

The update closes most of the previous structural and technical gaps, including notification
channel inventory, `onError` contrast, first paint/system chrome, appearance-control wording,
`ContentVisualKind`, Gradle working directory, CI/test ownership, SAS semantics, Preview phases,
spacing/elevation/dimension token holders, ShareReceiver log redaction, and Paparazzi seams.

It is **not implementation-ready yet**. Three direct contradictions remain, followed by several
acceptance-contract gaps that can still produce materially different implementations while all
current checkboxes appear complete.

## P0 — direct contradictions (must fix before approval)

### 1. Scanner security has both the corrected and the rejected decision

Correct contract:

- `specs/android-pairing/spec.md:134–154` requires `FLAG_SECURE` before scanner preview.
- `design.md:R12` also requires it.

Stale contradictory contract:

- `design.md:175` says no `FLAG_SECURE` is accepted because the camera shows no secret.
- `tasks.md:159–160` directs implementation to preserve no `FLAG_SECURE`.

The scanned QR is a valid pairing credential (fingerprint + token); this is not editorial ambiguity.
Delete the stale resolved-decision row. Replace S8.3 with an explicit implementation and test task:
set `FLAG_SECURE` before `super.onCreate`/camera preview initialization and assert it for both pairing
activities, including recents/screenshot behavior.

### 2. Pairing task still replaces the six-digit SAS with a fingerprint

`specs/android-pairing/spec.md:46–70` correctly defines the six-digit SAS as primary and the
fingerprint as optional supplemental metadata. `tasks.md:155–156` still says `SAS confirm (full
fingerprint)`. This can lead the implementing agent to recreate the exact security-UX error the spec
was changed to prevent.

Rewrite S8.1 to say: six-digit SAS is primary; Match/Doesn't match actions; optional full fingerprint
is visibly supplemental; preserve polling/watchdog/waiting/terminal states; no SAS/token logging.

### 3. Tablet/foldable is still falsely attributed to explicit user approval

`design.md:168–173` and `tasks.md:51–52` state that phone + tablet/foldable was selected by the user.
The recorded answer was “Pixel”; it did not approve three form factors. This matters because the
choice multiplies layout, device, localization, font-scale, and golden obligations.

Either obtain explicit product approval, or make Pixel-class portrait phone the committed scope and
put tablet/fold responsive work behind an explicit S0 decision gate. Until then do not call it
approved, and do not make tablet/fold goldens unconditional SHALL requirements.

## P1 — executable-contract gaps

### 4. Async feedback requirement contradicts its own Save exclusion

`android-feedback-states` says Save is synchronous and SHALL NOT be modelled as async loading, but
its very next scenario says “Save (or another async action) while … in flight” and requires a progress
indicator. Remove Save from that scenario. Add a separate synchronous Save scenario: disable only
during the `commit()` call if needed to prevent re-entry; do not expose an async/loading state; on
`false`, retain dirty state and show retryable failure.

### 5. “Exact typography” is asserted but the values are not recorded

The design/spec say each role has exact size, weight, line height, and tracking, but neither records
the actual table or a precise STYLEGUIDE table-to-Kotlin mapping. “Use §4” is insufficient if §4 has
CSS roles that do not map one-to-one to M3 typography roles and bundled font files/weights.

Add a table for every `CpTypography` role: family, resource/weight, size sp, line height sp, tracking,
M3 consumer role, and mono/tabular feature. Add a unit-test/inspection acceptance row for every value.

### 6. `CpDimensions` still contains ranges and approximations

The contract calls dimensions fixed but lists content tile `32–36`, glyph `~16–20`, nav pill `≈50×38`,
and leaves QR/SAS sizes unnamed. An implementation cannot be pixel-checked against ranges.

Record exact role-based constants (a 32 tile and a 36 tile may be two named roles), exact glyph boxes,
nav pill geometry, QR bounds/quiet zone, SAS cell, touch expansion, and screen max widths. Link each
component to one named dimension token.

### 7. D2 still says ColorScheme is mapped “minimally” while the task/spec require full coverage

`design.md:D2` uses “mapped minimally”; `tasks.md:1.7` asks for a full map *or* per-component
overrides. That leaves two non-equivalent acceptance paths and does not prove no default purple/tonal
role leaks from nested M3 components.

Choose one strategy. Prefer a complete explicit light/dark `ColorScheme` table for every M3 role,
plus a leakage test/golden containing every used M3 component. If overrides remain allowed, enumerate
every component and every overridden colors parameter; “or” is not testable acceptance.

### 8. Resource/system surfaces are inventoried in prose but not fully assigned to implementation

D16 names launcher/adaptive icon, splash, recents thumbnail, and sharesheet entry. Tasks only clearly
assign system-bar and first-paint tokens, while S12.3 says “correct labels/icons/intents” without a
per-resource decision or evidence.

Add explicit Preserve/Restyle/N-A rows and file owners for:

- legacy + adaptive launcher/monochrome icon;
- pre-Android-12 window background and Android-12 splash icon/background;
- task/recents color, label, thumbnail privacy behavior;
- sharesheet app label/icon and direct-share surface if any;
- notification small icons (including monochrome constraints);
- XML DayNight themes and status/navigation bar defaults before Compose.

Each row needs a task, a verification method, and expected EN/UK behavior where text exists.

### 9. Scanner implementation ownership conflicts with “OS-owned, do not style” wording

S12.3 groups ZXing with OS-owned surfaces and asks only labels/icons/intents, while the pairing spec
requires a security change inside `PortraitCaptureActivity`. ZXing is external-library UI, not an
OS-owned surface. Classify it separately: preserve library visuals, but S8 owns window security,
orientation, decoder, lifecycle, and tests. S12 should not imply that the scanner is untouched.

### 10. Notification task remains too coarse for the now-correct four-channel spec

The spec is substantially better, but S12.1 does not name all four IDs, their creating owners,
importance, localized metadata, channel migration decision, action targets, notification IDs, or
small icons. Convert it into separate implementation rows for service/copy/pair/sync plus restart
worker reuse. Explicitly test Open→`MainActivity`, Pause/Resume→`CaptureControlReceiver`, and
pairing→SAS modal. State whether existing channel metadata is preserved or migrated for each ID.

### 11. Partial-span masking needs a named implementation path and negative evidence

The global requirement now correctly says plaintext must be absent in merged and unmerged trees, but
S5.4 still says only `span-mask`. Define whether partial-sensitive text is replaced before building
`AnnotatedString`, represented as separate safe spans, or cleared at the parent. Add tests that search
the complete semantics dump for every plaintext fragment, not only the merged content description.

### 12. Connected tests are listed as a universal gate without CI scheduling semantics

The gate names a command and emulator, but R18 allows heavy jobs to be path-gated and the task does
not define which slices mark connected tests Required. “Each slice decides” can silently make the
security/a11y gate optional everywhere.

Define the minimum: Required for S3 (window/security semantics), S4 (focus/insets/nav), S5/S6
(masked secrecy), S8 (`FLAG_SECURE`), S9/S10 (focus/input/permission flows), and S15; define PR vs
nightly execution and required-check names. Other slices may be N-A only with recorded rationale.

### 13. Golden threshold remains intentionally undecided after claiming pixel-level review

The spec permits threshold 0 or nonzero, while the user requested pixel-level validation. The S0
decision is acceptable only if it has a bound and approval criterion. Record: metric, per-pixel
tolerance, allowed differing-pixel percentage, alpha handling, image dimensions, and who approves a
nonzero threshold. “Within configured threshold” alone can hide regressions.

### 14. S9 still applies a synthetic state list to “all tabs” rather than mapping state to owner

The settings spec correctly says only applicable states, but `tasks.md:164–166` remains broad and
does not map loading/validation/destructive states to concrete controls. Add a tab/control matrix:
field, persistence mode (draft/immediate/ephemeral), validation, disabled precondition, async owner,
error surface, Save/Discard participation, process-death behavior. This is the implementation-facing
artifact that prevents immediate controls from accidentally entering the Save batch.

### 15. S11 is still too broad to guarantee every feedback producer is migrated

Listing shared toast/banner/badge/dialog components does not enumerate their call sites. Add a
producer inventory: current Toast/Snackbar/custom toast/dialog/banner/notification outcomes, owning
screen/service, semantic kind, message resource, action/retry, and migration slice. Include About and
Logs separately because the traceability table assigns both to S11 but the task text does not mention
them.

## P2 — precision and maintenance issues

### 16. Decision numbering is out of order

D16 appears before D15. Renumber or reorder to keep references reliable. If IDs are intentionally
stable, order D15 then D16 and preserve IDs.

### 17. Audit provenance text is stale and misleading

`proposal.md`/`design.md` still describe a “three-agent audit”. The user explicitly required this
work to be done without agents. Replace this with “read-only code audit” unless there is retained,
reviewable evidence for those agent results. This does not change implementation but affects trust
and handoff accuracy.

### 18. Hardcoded-text allowlist criteria are too permissive

The localization spec exempts “version string” generically. A version value may be machine data, but
surrounding labels/sentences must remain localized. Restrict the allowlist to protocol literals,
format patterns, URLs, IDs, and raw values passed into localized formatted resources. Require each
allowlist entry to include file, literal/pattern, reason, and owner; reject broad directory/sink
exemptions.

### 19. Blur acceptance lacks a measurable fallback/performance budget

The spike mentions perf/clipping/edge but has no pass/fail values. Add target devices/API levels,
frame-time/jank budget, memory/allocation ceiling, clipping cases, scrolling behavior, and exact
fallback trigger. Otherwise the spike can “pass” while producing unusable chrome.

### 20. Branch/workflow mutations need an explicit execution boundary

Tasks now include branch creation and bd issue creation. Preserve the current wording that bd is
untouched until approval, and apply the same explicit approval boundary to branch creation/commit if
the implementing agent is only asked to modify specs. Document the local-main base SHA as evidence
when execution is authorized.

## Recheck of previous P0 findings

| Previous item | Status | Evidence / residual |
|---|---|---|
| P0-1 scanner privacy | **REOPENED** | Correct spec exists; stale design/task directly contradict it |
| P0-2 four notifications | Mostly closed | Spec corrected; implementation task needs per-channel detail |
| P0-3 `onError` AA | Closed | Near-black/AA test explicitly required |
| P0-4 system chrome/first paint | Mostly closed | Compose/XML contract added; resource execution matrix missing |
| P0-5 Display controls | Closed | Appearance subsection exactly four; existing controls retained |
| P0-6 Android content kind | Closed | `ContentVisualKind` resolver and precedence assigned |
| P0-7 partial-span semantics | Mostly closed | Security outcome present; concrete span strategy/test needs detail |
| P0-8 Gradle paths | Closed | All shown gates execute under `android/` |
| P0-9 deps/CI ownership | Closed | Robolectric/Paparazzi/warnings/CI assigned to S2 |
| P0-10 form factor approval | **REOPENED** | Unsupported approval claim remains |
| P0-11 SAS vs fingerprint | **REOPENED** | Spec fixed; S8.1 remains wrong |
| P0-12 Preview behavior | Closed | Peeking/Pinned and visible underlying list preserved |

## Required correction order

1. Remove the three P0 contradictions (scanner, SAS, form-factor approval).
2. Fix the self-contradictory Save scenario.
3. Freeze exact typography/dimension/M3-role tables.
4. Add executable system-resource and notification matrices.
5. Bind security/a11y connected gates to concrete slices and CI checks.
6. Add settings-state and feedback-producer matrices.
7. Resolve golden threshold and blur pass/fail metrics.
8. Run strict validation, then run stale-string checks for the rejected scanner decision,
   “SAS confirm (full fingerprint)”, and unsupported “user-approved” wording.

Approval criterion: no contradictory SHALL/task instruction, no product-approval claim without an
actual decision, and every visible/resource surface has one owner, one concrete action, and one
verification artifact.
