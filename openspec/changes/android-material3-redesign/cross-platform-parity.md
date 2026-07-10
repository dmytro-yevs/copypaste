# Cross-platform parity (Android ↔ desktop)

Normative parity contract. Target = **semantic + visual-system parity, not identical platform chrome**:
same tokens, content-type mapping, typography intent, component anatomy, terminology, hierarchy, states,
defaults, and operation outcomes; native Android navigation/input/permissions/notifications/insets/
gestures. Every divergence is named, justified, owned, tested.

## Pinned baseline (frozen at S0)

- **Source of truth (human-readable mirror):** root `STYLEGUIDE.md` — sha256
  `993d93a7b447c36ee89d32db9ae78ce73106dd6f6decef1ed155b537839bcce4`
  (git blob `319befc0dcd22492de1b83d7b8414308cfa500c5`)
  (re-pinned by S0.14 after §10/§11 refresh from tokens.css@6960539d). HTML reference is illustrative
  only. Per the Drift rule below, S0.14 (`tasks.md`) re-pins STYLEGUIDE's §10/§11 fenced token blocks
  against the current `tokens.css` — STYLEGUIDE is a mirror kept in sync, not itself the machine source.
- **Source of truth (machine-readable):** `crates/copypaste-ui/src/styles/tokens.css` — populated
  (232 lines: theme × accent × translucency tokens, additive scale tokens) and executable today.
- **Desktop base/target commit:** pinned at `6960539d` ("fix(app): redesign a11y/layout/contrast
  polish"), the last commit to touch `tokens.css`, landed as part of the desktop redesign epic merged
  to `main` via `5ab3a993` ("Merge design-system-redesign epic (desktop UI redesign) into main") and
  released as v0.4.0 (`62a9568f`). Every token/component comparison in this document targets
  `tokens.css` as of this commit; a row may be marked *Verified Exact* once its evidence passes
  against it.
- **Drift:** any post-freeze shared-token/component change updates BOTH platforms/specs in the same
  change OR records an exception here.

## Status model (two axes + evidence)

- **Target:** `Exact` · `Native` (approved platform difference) · `Platform-only` · `Deferred`.
- **State:** `Verified` (evidence passes on both) · `Planned` · `Blocked` (needs desktop commit/tokens) · `Drift`.
- **Evidence:** the artifact/command that proves it.

## Canonical machine-readable token source (single choice)

The canonical token representation is a generated **`parity/tokens.json`** (schema: `{theme:{dark,light}:
{surfaces,lines,text,overlays,status,statusStrong,content},accents[6]{dark,light,onAccent,variant},radii,spacing,
motion{durations,easing},alpha,typography[role]{family,weight,sp,lh,tracking},iconSizes}`). It is
generated from `crates/copypaste-ui/src/styles/tokens.css` at the pinned desktop commit `6960539d`
(the machine source). STYLEGUIDE §10/§11 is re-pinned from that same commit as the human-readable
mirror (tasks.md S0.14), but the generator reads `tokens.css` directly — Markdown is NOT parsed ad hoc
at test time. Android `CpColors`/`CpShapes`/`CpTypography`/`CpDimensions` and the web tokens are
BOTH checked against `parity/tokens.json` (or generated from it) by `verifyTokenParity`.
Command: a JVM `TokenParityTest` loads `parity/tokens.json` and asserts
each Android token equals it; a web check does the same on that side (desktop-epic-owned).

**Legacy parity gate retired.** The pre-existing `.github/workflows/parity.yml` +
`scripts/parity-check.mjs` CI gate is retired/deleted by S2.11a (`tasks.md`): the old script hardcodes
the deleted `android/app/src/main/java/com/copypaste/android/ui/theme/Color.kt` path and cannot map
the ~35 additive non-color tokens introduced by the desktop redesign (`--fs-*`, `--fw-*`, `--lh-*`,
`--ls-*`, `--focus-ring-width`/`--focus-ring-offset`, `--hairline`, `--icon-sm`/`-md`/`-lg`, `--ctl-h-*`,
`--main-min-*`, `--popup-w`, `--r-xs`/`-sm`, `--stroke-*`, `--pad-*`, `--gap-*`, `--sz-*`,
`--modal-w*`, `--sel-bar-*`, `--r-chk`, `--chk-bw`, `--r-empty-ic`, `--pad-empty`/`-grouphead`/
`-bulkbar`, `--mask-blur`, `--frost-filter`, `--chrome-bg`, `--scrim-blur`); `TokenParityTest` +
`parity/tokens.json` fully replace it.

## Parity matrix

| Area | Owner (desktop → Android) | Target | State | Evidence |
|---|---|---|---|---|
| Tokens (surfaces/lines/text/overlays/status+3 strong variants/6 accents/on-accent/10 content+aliases) | web tokens → `CpColors`/`AccentColor` | Exact | Planned | `parity/tokens.json` + `TokenParityTest` |
| Radii/spacing/elevation/motion/easing | CSS vars → `CpShapes`/`CpSpacing`/`CpElevation`/`CpMotion` | Exact | Planned | `TokenParityTest` |
| Typography (Title 700 both) | CSS roles → `CpTypography` | Exact | Planned | paired type fixture + font-provenance (below) |
| Icons | `lucide-react` → Lucide-Compose | Exact (glyph mapping) | Planned | icon table below + paired review |
| Theme axis (System = resolver) | data-theme/accent → `ThemeMode` | Native | Planned | forced-vs-resolved snapshot equality test |
| History list/rows/tiles/kinds | HistoryView/Row → History* | Exact semantics | Planned | resolver/order/grouping; paired row + masked-row fixtures |
| History input (hover/kbd vs swipe/long-press) | `HistoryRow.tsx` → | Native | Planned | outcomes-identical tests |
| Preview vs Details modal | HistoryView Details modal → Peeking/Pinned overlay | Native | Planned | same content/actions/masking; input differs |
| Devices cards/presence/metadata | DeviceCard → PeerRow/OwnDeviceRow | Exact data/action | Planned | device-card table below + paired fixture |
| Pairing QR/scan/6-digit SAS/success/error | Pair views → Pair* | Exact | Planned | SAS-primary both; paired SAS fixture; scanner FLAG_SECURE (Android security) |
| Shared settings | SettingsView → Settings tabs (§I) | Exact semantics | Blocked (desktop defaults pending) | per-setting table below |
| Platform-only settings | macOS Shortcuts/Accessibility ‖ Android Notifications/Permissions/Background/OEM | Platform-only | Planned | individual rows below |
| Toast | Toast.tsx stacked array → single-slot policy | Native | Planned | sequence tests (feedback spec) |
| Banner/status/error/loading/empty/degraded | shared → feedback-states | Exact severity+intent | Planned | banner + empty/degraded fixtures |
| Masking privacy | CSS blur → blur(31+)/opaque-overlay(<31) | Native (<31) | Planned | two-layer contract; layout-stability + no-plaintext test |
| About/Logs | About/Logs → AboutActivity/LogViewer | Exact intent | Planned | version/build/links/licenses; log levels |
| Localization/terminology | inline EN → EN+UK | Exact intent | Planned | UX glossary below |
| Accessibility | web a11y → Compose semantics | Exact intent | Planned | AA, focus, 48dp, non-color signal |
| Motion events | shared → per-event map | Exact/Native | Planned | motion table below |
| Quick-paste sheet / QS tile (§9.13) | Popup = Desktop-only ‖ mobile sheet | Deferred | Deferred | separate epic; NO placeholder UI |

## Icon role → canonical glyph map

Upstream = Lucide at the revision pinned in S0.3 (`icons.lucide.rev`). Desktop binding = `lucide-react`;
Android binding = the pinned Lucide-Compose artifact. Stroke 1.6, 24×24 viewBox, `currentColor`.

| Semantic role | Lucide name | desktop | Android | size | cd / fallback |
|---|---|---|---|---|---|
| nav History | `history` | ✓ | ✓ | 24 | informative · fallback `clock` |
| nav Devices | `monitor-smartphone` | ✓ | ✓ | 24 | informative |
| nav Settings | `settings-2` | ✓ | ✓ | 24 | informative |
| kind TEXT | `align-left` | ✓ | ✓ | 18 | decorative(has type word) |
| kind URL | `link` | ✓ | ✓ | 18 | decorative |
| kind EMAIL | `mail` | ✓ | ✓ | 18 | decorative |
| kind PHONE | `phone` | ✓ | ✓ | 18 | decorative |
| kind CODE | `code` | ✓ | ✓ | 18 | decorative |
| kind JSON | `braces` | ✓ | ✓ | 18 | decorative |
| kind NUMBER | `hash` | ✓ | ✓ | 18 | decorative |
| kind COLOR | (swatch, no glyph) | ✓ | ✓ | tile | renders color |
| kind PATH/FILE | `file` / `folder` | ✓ | ✓ | 18 | decorative |
| kind IMAGE | (thumbnail, no glyph) | ✓ | ✓ | tile | renders image |
| kind SECRET | `lock` | ✓ | ✓ | 18 | informative |
| status ok/warn/err/info | `check-circle`/`alert-triangle`/`alert-circle`/`info` | ✓ | ✓ | 16–20 | informative |
| action pin/delete/copy/reveal | `pin`/`trash-2`/`copy`/`eye` | ✓ | ✓ | 20 | informative |
| action unpair/revoke | `unlink`/`shield-x` | ✓ | ✓ | 20 | informative |
| empty state | `inbox` | ✓ | ✓ | 24+ | decorative |

A missing Android glyph maps to a defined fallback (never a silent generic icon); a font/icon test
asserts every role resolves on both platforms.

## Shared-settings desktop ↔ Android comparison (State=Blocked until desktop defaults pinned in S0)

| Setting | desktop prop · default · mode | Android key · default · mode | Target |
|---|---|---|---|
| Theme | `theme` · dark · immediate | `theme_mode` · dark · draft(→publish) | Native (Save model differs) |
| Accent | `accent` · indigo · immediate | `accent` · indigo · draft | Native |
| Translucency | `translucency` · on · immediate | `translucency` · on · draft | Native |
| Mask sensitive | `maskSensitive` · on | `mask_sensitive_content` · true · draft | Exact |
| Private mode | `private_mode` · off | `private_mode` · false · draft | Exact |
| Sync enable | `sync_enabled` · on | `sync_enabled` · true · draft | Exact |
| Relay / Supabase enable | transport flags | `relay_enabled`/`supabase_enabled` · true · immediate | Exact |
| Wi-Fi-only / P2P / LAN / auto-apply | shared | `sync_on_wifi_only`/`p2p_sync_enabled`/`lan_visibility`/`auto_apply_synced_clip` | Exact (auto-apply=Repair on Android) |
| Text/image/file limits · quota · TTL · max items | shared | `max_*`/`storage_quota_bytes`/`sensitive_ttl_secs`/`max_history_items` | Exact (max_file=Repair) |
| Excluded apps | shared | ConfigKnobs list | Exact |
| Notify/sound on copy | shared where supported | `notify_on_copy`/`sound_on_copy` | Exact (independent) |
| macOS Shortcuts / Accessibility | desktop | — | Platform-only |
| Notifications / Permissions / Background / OEM | — | Android | Platform-only |

Exact defaults/ranges/labels are filled once the desktop base commit is pinned (S0); rows stay
State=Blocked until then.

## Motion event map

| Event | desktop primitive | Android primitive | duration · easing | interruption | reduced-motion | target |
|---|---|---|---|---|---|---|
| Theme/accent change | CSS crossfade | Crossfade | 300 · std | cancel→settle | instant | Exact |
| Toggle | CSS transition | animate*AsState | 120 · std | reverse | instant | Exact |
| Modal/sheet enter-exit | CSS | Compose transition | 200 · std | reverse | instant | Exact |
| List insert/remove | CSS | animateItem | 200 · std | additive | instant | Exact |
| Presence dot | soft glow | soft glow | one-shot | n/a | no glow | Exact |
| Toast in/out | slide | slide | 200 · std | replace | instant | Exact |
| Nav selection | (none/instant) | spring pop | spring | reverse | instant | **Native** |
| Masking reveal | fade | fade | 120 · std | reverse | instant | Exact |

## UX glossary / message-intent (same intent, localized per platform)

| Term/intent | desktop label | Android EN | Android UK | consequence/target |
|---|---|---|---|---|
| Clip | Clip | Clip | Кліп | clipboard item |
| Device / peer | Device | Device | Пристрій | paired peer |
| Pair | Pair | Pair | Парувати | establish trust |
| Unpair | Unpair | Unpair | Роз'єднати | local-only remove (no peer signal) |
| Revoke | Revoke | Revoke | Відкликати | audit-first, optional key rotation |
| Private mode | Private | Private | Приватний | no persist/sync |
| Auth failure | (msg) | "Check credentials" | "Перевірте облікові дані" | re-enter creds |
| Degraded DB | (msg) | degraded/reset | деградовано/скинути | reset offer |
| Destructive confirm | names target | names target | називає ціль | irreversible |

## Paired fixtures (structural parity; not pixel diff)

Shared fixture-data schema `parity/fixtures/*.json` (same synthetic records both sides). Android:
Paparazzi (`:app:verifyPaparazziDebug`) + structural assertions → `app/build/reports/paparazzi/…`.
Desktop: Playwright (`pnpm test:visual` in the desktop epic) → desktop CI artifacts. **Gate split:**
Android structural conformance is blocking HERE; the desktop paired evidence is an explicit
**dependency owned by the desktop epic** and its target commit is recorded in S0 — if unavailable, the
Android side still runs and the desktop comparison is marked Blocked, never silently green. Required
pairs: history row, masked row, device card, SAS dialog, a Settings group, banner, destructive modal,
empty state, sync status.

## Masking, toast, typography (single definitions)

- **Masking:** single pipeline per `android-history` — blur (API 31+) / geometry-preserving opaque
  overlay over a sanitized representation (<31); never plaintext in display list/semantics/logs/recents.
- **Toast:** single-slot policy per `android-feedback-states` (non-actionable coalesces; actionable not
  replaced by non-actionable; second actionable → banner). No backlog.
- **Typography:** Android bundles a real Inter 700 face (upstream version/checksum/license recorded in
  S1 with APK-size impact); the paired type fixture asserts BOTH platforms render Inter 700 (no system/
  SF fallback).

## Guide gaps (recorded)

STYLEGUIDE has no dedicated section for the History **Preview/Details** interaction or the full
**Pairing** flow (it covers Popup/Quick-paste §9.13 and component anatomy, not these flows). Those
matrix rows therefore follow their component owners (desktop Details modal / Pair views → Android
Peeking-Pinned overlay / Pair*) with THIS document as the shared behaviour addendum — not an implied
STYLEGUIDE section. Any future guide update should absorb these addenda.

## Governance

- **S0 parity freeze** (pin desktop commit; fill Blocked rows) precedes S1/S2 close.
- **S15 close-out parity audit** re-checks every row; no row is *Verified Exact* without a pinned owner
  on both platforms and passing executable evidence.
