# Tenth-pass verification — cross-platform parity fixes

Date: 2026-07-02  
Validation: `openspec validate android-material3-redesign --strict` — **valid**.  
Pinned STYLEGUIDE hashes verified: sha256 `25b9bd05…84c9`; git blob `ad470fac…c67`.

## Verdict

The five headline divergences now have a direction, but NINTH_PASS is not fully implemented. The new
parity document is a useful summary, not yet the claimed normative/evidence matrix: several required
tables are referenced but absent, and some rows assert Exact before a desktop baseline or executable
desktop tokens exist.

## P0 — incomplete or contradictory contracts

### 1. §I still violates its own “every row” requirement

Only Sync has `consumer · activation`; General, Display, Storage, Notifications and non-tab rows still
have the old five columns. A header saying the missing data is “sourced from requirements” does not
identify a consumer or activation for each setting, and it cannot be mechanically audited. Exact
defaults are also still absent (`native default`, `ConfigKnobs list`, `keystore-wrapped`).

Expand every row with the promised columns. A worked example is not a complete inventory. Include
exact default values after native resolution, exact storage key/secret alias, runtime consumer,
activation, dependency, failure feedback and named test evidence.

### 2. The parity file references artifacts that do not exist

`cross-platform-parity.md` claims:

- canonical icon role→Lucide table “(#12)”;
- shared UX glossary/message-intent table “(#16)”;
- per-setting desktop comparison;
- event-level motion mapping.

None of those tables exists in the file or another named artifact. Add the actual rows now; do not
use review finding numbers as dangling references. Each must include platform owner and evidence.

### 3. Toast contract is internally contradictory

Android requires “single-slot”, “no stacked backlog”, and immediate replacement, but an actionable
toast is “re-queued or persisted.” Re-queueing is a backlog; persistence is a different UI mechanism.
The implementation and tests cannot infer which wins, ordering, capacity, expiry, or what happens when
two actionable toasts arrive.

Choose one exact mobile policy. A defensible option is: actionable toast is never replaced by a
non-actionable one; new non-actionable feedback is coalesced; a second actionable event becomes a
persistent banner/action center entry with defined capacity. Whatever is chosen, define priority,
ordering, capacity, expiry, accessibility announcements and process-death behavior. Remove
“re-queued or persisted.”

### 4. “Exact” parity is asserted before it is verifiable

Desktop `tokens.css` is pending and `index.css` is empty; desktop base/target commit remains an S0
placeholder. Nevertheless most rows are marked Exact. Split the matrix into:

- target classification: Exact/Native/Platform-only/Deferred;
- current state: Verified/Planned/Blocked/Drift;
- evidence artifact/command.

Until desktop commit and executable tokens are pinned, tokens/type/components cannot be “Verified
Exact.” This prevents a planning intent being mistaken for completed parity.

## P1 — parity precision gaps

### 5. Shared-settings parity is still one aggregate assertion

The single row lists theme through sound and points back to Android §I. It does not compare any desktop
property, default, range, persistence mode, dependency, activation or consumer. Add one row per shared
semantic setting and explicitly classify mismatches. Android-only notification/system controls and
desktop-only Shortcuts/Accessibility need individual rows, not one aggregate platform-only row.

### 6. Icon parity table must pin an upstream revision and fallback

“Lucide” plus owners is insufficient: `lucide-react` and the chosen Compose artifact may expose
different versions/names/path data. The missing table needs semantic role, canonical upstream icon
name, upstream revision, desktop binding, Android binding, size, stroke, content-description rule,
and approved fallback. Include all 12 content kinds, navigation, actions, states and empty screens.

### 7. Motion event parity is absent

Token duration equality does not prove behavioral parity. Add rows for theme transition, toggle,
modal/sheet enter-exit, list insertion, presence transition, toast, nav selection and masking/reveal.
Each row needs desktop primitive, Android primitive, duration/easing, interruption behavior and
reduced-motion result. Classify Android spring/gesture motion as Native adaptation where appropriate.

### 8. Paired fixtures have no executable desktop ownership

S2.11 names Android and desktop paired fixtures but does not identify Playwright command, fixture
files, shared fixture-data schema, desktop-epic owner/dependency, artifact paths, or what happens if
the desktop target commit is unavailable. The Android epic cannot claim the gate green by generating
only Paparazzi images.

Define both commands and outputs, or split the gate: Android structural conformance is blocking here;
desktop paired evidence is an explicit dependency supplied by the desktop epic. S0 must record that
dependency and its revision.

### 9. Some STYLEGUIDE citations are inaccurate

The Preview/Details row cites §9.13, but §9.13 specifies Desktop Popup / mobile Quick-paste, not the
History Details modal. Pairing cites “§pairing”, which is not a STYLEGUIDE section. Replace these with
actual source owners/spec references. If the shared guide lacks Preview/Pairing behavior, record that
as a guide gap and define a shared behavior addendum rather than implying existing authority.

### 10. Pre-31 masking fallback remains underspecified

“Geometry-preserving opaque/pixelated” still offers two implementations and does not say whether
plaintext is rendered underneath an overlay. Define one safe pipeline: sanitized placeholder or
offscreen-safe representation, exact geometry strategy, reveal transition, partial spans and display-
list/screenshot behavior. A layout-stability test alone does not prove secrecy.

### 11. Machine token parity against Markdown needs a stable extraction contract

The gate says compare against STYLEGUIDE §10/§11 “or” a generated canonical file. Parsing prose/tables
and using a generated file are non-equivalent. Select one canonical machine-readable representation,
define its schema/path/generator/check command and ensure Markdown, web and Android are checked or
generated from it. Otherwise the hash freezes prose but not a reliable executable mapping.

### 12. Typography parity still needs asset provenance and desktop-face evidence

Android now plans a real Inter 700 face, which fixes the local contradiction. Add exact upstream
version/checksum/license file and APK impact. The paired fixture must also prove desktop resolves
Inter 700 rather than falling back to system/SF font; otherwise both specs say 700 while rendered
faces differ.

## Confirmed improvements

- Root and docs STYLEGUIDE copies and recorded hashes match.
- Android Title is restored to shared 700 and S1 owns the missing face.
- System theme is correctly classified as a resolver/native adaptation.
- `preview_lines`, navigation/input, platform settings, Popup and Quick-paste are explicitly classified.
- API<31 bullets were removed from normative history behavior.
- S0.13, S2.11 and close-out governance exist.
- Quick-paste is clearly Deferred with no placeholder UI.

## Required correction order

1. Complete §I for every row.
2. Materialize icon, settings, motion and UX-glossary tables.
3. Resolve toast policy to one algorithm.
4. Split parity target from verification state/evidence.
5. Define paired desktop/Android commands, fixture schema and ownership.
6. Correct source citations and fully specify safe masking fallback.
7. Select one machine-readable token source and pin font provenance.

Approval criterion: no parity row may claim verified equality without a pinned owner on both
platforms and executable evidence; no referenced table may be implicit or deferred behind prose.
