# Ninth-pass review — Android ↔ desktop design and behavior parity

Date: 2026-07-02  
Sources checked: root `STYLEGUIDE.md` (identical to `docs/design/STYLEGUIDE.md`), desktop React/Tauri
components, Android source, all Android redesign specs/tasks/inventories, and EIGHTH_PASS fixes.  
Validation: `openspec validate android-material3-redesign --strict` — **valid**.

## Verdict

The Android plan correctly uses the shared STYLEGUIDE as its source and covers most named components,
but it does not yet contain a testable cross-platform parity contract. “Use the same guide” is not
enough: several Android decisions already diverge from the guide/desktop, and no matrix distinguishes
required parity from intentional native adaptation.

The correct target is **semantic and visual-system parity, not identical platform chrome**:

- identical tokens, content-type mapping, typography intent, component anatomy, terminology,
  information hierarchy, states, defaults, and operation outcomes;
- native Android navigation, touch targets, system permissions, back behavior, notification surfaces,
  insets, and gestures;
- every divergence named, justified, owned, and tested.

## P0 — blocking parity contradictions

### 1. Android typography silently diverges from the shared desktop contract

The EIGHTH_PASS fix changes Android Title from Inter 700 to 600 because only 400/500/600 are bundled.
That makes the Android spec internally implementable, but root STYLEGUIDE §4 defines Title as 700,
and desktop uses the same shared design language. A platform-local spec may not downgrade a signature
role while claiming exact cross-platform parity.

Choose one cross-platform decision:

- preferred: bundle/license a real Inter 700 face on Android and retain Title 700; or
- change the root STYLEGUIDE and desktop implementation to 600 in the same approved design change.

Do not solve an asset gap by silently changing only Android. Add a paired typography fixture (same
title/section/body/meta/micro strings) and verify family, real face weight, relative hierarchy,
line-height, tracking, wrapping, and fallback on both platforms.

### 2. §I still does not implement the EIGHTH_PASS field-by-field behavior matrix

The updated §I still has only `Setting | key | default | mode | status`. It does not contain the
promised facade property, UI owner, runtime consumer, activation timing, dependencies, failure
feedback, or evidence. It also still groups Supabase URL/key and email/password and uses “native
default” rather than exact values.

This directly contradicts the new android-settings scenario: “every setting row names its activation
timing.” Expand §I now; do not defer it to S9 implementation. Add cross-platform columns for shared
settings: desktop property/consumer/default/range and whether parity is Exact, Adapted, Android-only,
or Desktop-only.

### 3. No cross-platform parity artifact or gate exists

Current traceability maps Android surfaces only. There is no normative mapping from STYLEGUIDE
component → desktop owner → Android owner → shared semantics → platform adaptation → paired evidence.
Consequently an Android implementation can pass every current test while differing from desktop in
labels, hierarchy, state handling, queue behavior, or action outcome.

Add `cross-platform-parity.md` as a source-of-truth matrix and an owning task/gate. At minimum cover:

- theme/accent/tokens/typography/shapes/elevation/motion/icons;
- History list, groups, search/filter, selection, pin/delete/copy/reveal, Preview/Details;
- Devices cards, presence, metadata, unpair/revoke, discovery;
- pairing QR/scan/SAS/success/failure;
- shared Settings concepts, defaults, ranges, validation and activation;
- toast/banner/status/error/loading/empty/degraded behavior;
- About/Logs, privacy, localization, accessibility.

Every row must say **Exact**, **Native adaptation**, **Platform-only**, or **Deferred**, with reason and
evidence. Add an S0 parity freeze before S1/S2 can close and a close-out parity audit.

### 4. Toast queue behavior differs from desktop

Android spec requires a single slot where a new toast replaces the current one. Desktop
`components/Toast.tsx` explicitly keeps an array and stacks newer toasts above older ones. This is an
observable behavioral mismatch in the same application, not native OS chrome.

Choose one product behavior for both platforms, or explicitly approve a platform adaptation with a
reason (e.g. mobile screen space), maximum loss policy, action-toast priority, duration reset, and
accessibility announcement behavior. A replacement policy must not silently discard an actionable
Undo/error toast. Add sequence tests for success→error, action toast→new toast, and burst events.

### 5. Android API<31 masking contradicts the shared masking visual contract

STYLEGUIDE §7 says sensitive data is masked by blur, never deletion, and retains its real width.
Android history spec substitutes bullet characters below API 31. Bullets change geometry and are not
the same visual behavior; partial-span placeholders can also reveal length/structure differently.

Security may require plaintext not to enter semantics/rendering, so copying desktop CSS blur blindly
is not acceptable either. Define one cross-platform privacy contract with two layers:

- visual: stable geometry and the same masked affordance/reveal icon/state;
- accessibility/security: no plaintext in semantics, logs, fixtures, recents, or notifications.

If Android cannot safely blur pre-31, specify an approved opaque/pixelated geometry-preserving native
fallback and classify it as Native adaptation. Test layout stability before/after reveal and ensure
the fallback does not expose plaintext in the display list/accessibility tree.

## P1 — missing parity contracts

### 6. The reference baseline is not pinned

The desktop redesign is actively changing, while `STYLEGUIDE.md` is the declared authority and the
HTML reference is illustrative. Pin the STYLEGUIDE hash/commit used by Android S0 and record the
desktop redesign base/target commit or issue. Define drift handling: any shared-token/component
change after the freeze updates both implementations/specs or records a platform exception.

Do not compare Android against whichever desktop code happens to be in the working tree. The current
desktop `index.css` is empty, so it cannot serve as an executable token baseline by itself.

### 7. Shared token parity needs machine-readable verification

Android tests validate its own values, but not equality with web tokens. Create one canonical
machine-readable token source or a parity test/parser that compares:

- dark/light surfaces, lines, text, overlays, six accents and on-accent values;
- status and ten content colors plus PHONE→NUMBER/PATH→FILE aliases;
- radii, spacing, durations/easing and semantic alpha formulas;
- typography roles and icon role sizes.

Generated platform files are acceptable only if generated diffs are reviewed and both builds test
the same source. Otherwise add explicit duplicated-value drift tests.

### 8. Component parity must compare anatomy and states, not platform pixels

Paparazzi Android screenshots and Playwright desktop screenshots cannot be byte-compared because of
font rasterization, density, window geometry, and native controls. Define paired reference fixtures
with the same synthetic data and assert:

- same information and ordering;
- same semantic token/variant;
- same state and available actions;
- same content kind/icon/color;
- same success/error outcome.

Use screenshots for human side-by-side review, plus structural assertions for enforceable parity.
Create paired fixtures for a history row, masked row, device card, SAS dialog, Settings group,
banner, destructive modal, empty state, and sync status.

### 9. History/Preview behavior parity is not mapped

Desktop uses hover/keyboard actions and a Details modal; Android uses swipe/long-press and
Peeking/Pinned Preview. Those are valid native adaptations, but shared behavior must be explicit:
same type resolver, metadata order, pinned grouping, copy/delete/pin/reveal outcomes, file/image error
states, scroll restoration, and sensitive guard scope. Document gesture/input differences separately.

Also resolve style-guide ambiguity: §9.5 says single-line preview while Android exposes
`preview_lines` 1–6. If multi-line is a retained Android setting, mark it Native adaptation or update
the global guide/desktop capability.

### 10. Settings parity is not compared with desktop

Android §I verifies Android keys only. For settings present on both platforms, compare semantic
defaults, ranges, labels, dependencies, validation, side effects, and save/immediate behavior:

- private mode, sync enable/transports/Wi-Fi/P2P/LAN/auto-apply;
- masking/reveal warning/translucency/theme/accent;
- text/image/file limits, quota, TTL, max items, excluded apps;
- notification/copy sound behavior where supported.

Platform-only sections must be explicit: macOS Shortcuts/Accessibility versus Android
Notifications/Permissions/Background/OEM. The user should recognize the same concepts even when the
navigation container differs.

### 11. Theme `System` must be a resolver, not a third visual axis

The shared guide defines only dark/light × six accents. Android adds System. That is acceptable only
if System resolves strictly to the same dark or light token set and never introduces a third palette,
dynamic color, or different component values. Add a parity test: forced Dark, forced Light, and
System-resolved Dark/Light yield identical token/component snapshots for the same resolved theme.

### 12. Icon parity needs a canonical glyph-role mapping

Both platforms use Lucide-style icons but different libraries/artifacts can contain renamed or
redrawn glyphs. Add a table of semantic role → canonical Lucide icon name/source revision → platform
binding → size/stroke/fallback. Paired review must cover navigation, all content kinds, status,
actions, empty states, and destructive operations. Fallback must preserve meaning, not silently use a
generic icon on one platform.

### 13. Device-card and pairing terminology needs shared-data parity

“This Mac” versus “This phone” is an intentional localized label adaptation; metadata order and
meaning must remain identical. Add paired contract tests for own-device six rows, peer eight rows,
missing-value hiding, fingerprint truncation/copy, presence, transport, Verified, dates, RTT,
Unpair/Revoke, QR lifetime, six-digit SAS and terminal states. Platform labels may differ; underlying
fact and action must not.

### 14. Motion parity lacks event-level mapping

Shared duration/easing tokens exist, but named transitions do not. Map each shared event (theme
change, modal enter/exit, toggle, list insert, presence transition, toast) to duration/easing and
reduced-motion result on both platforms. Android spring navigation must have a documented desktop
counterpart or be classified Native adaptation. Both must suppress non-essential transforms/glows
under the platform reduced-motion signal.

### 15. Quick-paste is a declared guide component but excluded from Android

STYLEGUIDE §9.13 defines mobile quick-paste, while proposal/design explicitly defer the sheet/QS
tile. Deferral is valid, but “every surface redesigned to the guide” must not imply that §9.13 is
delivered. Add it to the parity matrix as **Deferred / separate epic**, with no placeholder UI and a
tracked dependency. Desktop Popup remains Desktop-only for this epic.

### 16. Cross-platform copy and error language is not governed

Android requires EN/UK localization; desktop currently contains many inline English messages.
Literal equality is neither possible nor always desirable, but terminology and severity must match.
Create a shared UX glossary/message-intent table for Clip/Device/Pair/Unpair/Revoke/Private/Sync,
auth failure, degraded DB, destructive confirmations, and recovery actions. Each platform localizes
from the same intent and names the same target/consequence.

## EIGHTH_PASS recheck

| Item | Status | Residual |
|---|---|---|
| Bundled typography weights | **Partially closed / parity reopened** | Android table is feasible but Title 600 conflicts with shared Title 700 |
| auto-apply no-op | Assigned | Repair + S9.5 exists; consumer/evidence still implementation work |
| sensitive-skip no-op | Assigned | Repair + behavior scenario exists |
| max-file-size no-op | Assigned | Repair + acquisition-path scenario exists |
| legacy sync backend | Closed in contract | Retain key, hide effective selector |
| per-key §I matrix | **Not closed** | Limits split, but required consumer/activation/evidence columns are absent |
| three-layer tests | Assigned | S9.4/requirements cover migration, consumer and UI |
| activation timing | Normative but unreconciled | Requirement says every row; §I does not contain it |
| screenshot propagation | Closed in contract | Ordinary versus unconditional-secure windows defined |
| action kinds | Closed | Action separated from ImmediatePreference |
| private mode | Closed in contract | Observable effects defined |
| reveal guard | Closed in contract | Entry points and pre-confirm semantics defined |
| limit semantics | Partially closed | Requirement exists; §I still lacks exact defaults/consumers/evidence |
| dependencies/disabled rules | Closed in contract | Requires visual + callback tests |
| no-consumer source gate | Assigned | S2.10 exists; it must understand adapters/DI to avoid false positives |

## Required correction order

1. Resolve Title weight at the shared STYLEGUIDE level or bundle Inter 700 on Android.
2. Expand §I into the promised behavior/evidence matrix.
3. Add and freeze `cross-platform-parity.md` with Exact/Native/Platform-only/Deferred status.
4. Resolve toast queueing and masked fallback divergences.
5. Pin the STYLEGUIDE/desktop baseline and add drift handling.
6. Add shared-token machine checks and paired component fixtures.
7. Map shared settings, history/preview, devices/pairing, motion, terminology, and error outcomes.
8. Mark quick-paste explicitly Deferred in the parity artifact.

Approval criterion: an Android screen may differ in native layout/input mechanics, but the same user
fact, state, setting, action, privacy rule, and outcome must have the same meaning and recognizable
visual language on desktop and Android; every exception must be explicit and test-backed.
