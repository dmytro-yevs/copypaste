# Cross-Platform Parity Spec — Apple macOS Tahoe "Liquid Glass"

Canonical, single-source design contract for **web/desktop (copypaste-ui)** and
**Android (Compose)**. Target aesthetic: Apple macOS 26/27 (Tahoe) "Liquid Glass" —
**greyish, translucent, frosted, light-first**. Behaviour and appearance must be
**identical across platforms, adapted to platform conventions** (mouse/hover on
desktop, touch/press on phone; sidebar on desktop, bottom tabs on phone).

This doc resolves every "pick one" from the parity audit. When an agent must
choose a value, it uses the number HERE — not its platform's old value.

> **File references** (§A–§C added in the 2026-06-15 refresh):
> Web tokens live in `crates/copypaste-ui/src/index.css`; components in
> `crates/copypaste-ui/src/components/`; views in `crates/copypaste-ui/src/views/`.
> Android tokens live in
> `android/app/src/main/java/com/copypaste/android/ui/theme/Palette.kt`
> and `…/ui/theme/Theme.kt`; components in `…/ui/theme/Components.kt`;
> activities (screens) in `…/android/<Name>Activity.kt`.

---

## 0. Theme & default

- **Light-first.** Default theme is **light** on both platforms.
  - Web: `<html data-theme="light">` is the default (set in `index.html`); the
    saved pref re-syncs via `App.tsx`. Dark is an override block.
  - Android: default to **light**; a Settings control lets the user pick
    System / Light / Dark (parity with web's theme control). Follow-OS only when
    the user selects "System".
- Both platforms ship a user-facing **theme control** (System / Light / Dark).

---

## §A. Design Token Cross-Reference

### §A.1 Dark-theme neutral tokens (shared base ramp)

| Token | Role | Web (index.css) | Android (Palette.kt) | Expected value |
|---|---|---|---|---|
| `--ide-bg` / `bg` | Window canvas | index.css:93 `:root --ide-bg-rgb` | Palette.kt:173 `GmBg0` | dark `#13141A` → per-palette bg0 |
| `--ide-panel` / `panel` | Sidebar/list | index.css:94 | Palette.kt:174 `GmBg1` | per-palette bg1 |
| `--ide-elevated` / `elevated` | Cards, inputs | index.css:95 | Palette.kt:175 `GmBg2` | per-palette bg2 |
| `--ide-raised` / `raised` | Hover/pressed | index.css:96 | Palette.kt:178 `GmRaised` | per-palette surfaceStrong |
| `--ide-border` | Hairline separators | index.css:97 | Palette.kt:207 `border=darkLine(0.16f)` | `rgba(255,255,255,0.16)` dark |
| `--ide-divider` | Row separators | index.css:98 | Palette.kt:208 `divider=darkLine(0.14f)` | `rgba(255,255,255,0.14)` dark |
| `--ide-text` / `text` | Label color | index.css:107 | Palette.kt:194 `GmText` | `rgba(248,250,255,0.96)` dark |
| `--ide-dim` / `dim` | Secondary label | index.css:108 | Palette.kt:195 `GmDim` | `rgba(229,236,255,0.78)` dark |
| `--ide-faint` / `faint` | Tertiary label | index.css:110 | Palette.kt:196 `GmFaint` | `rgba(217,225,244,0.58)` dark |
| `--ide-ghost` / `ghost` | Metadata text | index.css:114 | Palette.kt:199 `GmGhost` | `rgba(255,255,255,0.55)` dark |
| `--ide-ghost-deco` / `ghostDeco` | Decorative icons | index.css:117 | Palette.kt:200 `GmGhostDeco` | `rgba(255,255,255,0.38)` dark |
| `--ide-hover` / `hover` | Hover fill | index.css:101 | Palette.kt:220 | `rgba(255,255,255,0.05)` dark |

### §A.2 Light-theme neutral tokens (`:root[data-theme="light"]` vs `LightIdeColors`)

| Token | Role | Web (index.css) | Android (Theme.kt) | Expected value |
|---|---|---|---|---|
| `--ide-bg` | Window canvas | index.css:234 | Theme.kt `LightIdeColors.bg` | `#E3E3E8` / `rgb(227,227,232)` |
| `--ide-panel` | Sidebar/list | index.css:235 | Theme.kt `LightIdeColors.panel` | `#F2F2F5` / `rgb(242,242,245)` |
| `--ide-elevated` | Cards | index.css:236 | Theme.kt `LightIdeColors.elevated` | `#FFFFFF` |
| `--ide-text` | Label color | index.css:249 | Theme.kt `LightIdeColors.text` | `rgba(29,29,31,0.92)` → `#1D1D1F` |
| `--ide-dim` | Secondary label | index.css:249 | Theme.kt `LightIdeColors.dim` | `rgba(91,91,96,0.76)` → `#5B5B60` |
| `--ide-faint` | Tertiary label (AA-darkened) | index.css:254 | Theme.kt `LightIdeColors.faint` | `rgba(108,108,114,0.58)` → `#6C6C72` |
| `--ide-accent` | System Blue | index.css:259 | Theme.kt `LightIdeColors.accent` | `#007AFF` / `rgb(0,122,255)` |
| `--ide-danger` | System Red (AA-darkened) | index.css:268 | Theme.kt `LightIdeColors.danger` | `#D7281E` / `rgb(215,40,30)` |
| `--ide-success` | System Green (AA-darkened) | index.css:270 | Theme.kt `LightIdeColors.success` | `#28884A` / `rgb(40,140,70)` |
| `--ide-warning` | System Orange (AA-darkened) | index.css:272 | Theme.kt `LightIdeColors.warning` | `#B06E14` / `rgb(176,110,20)` |

### §A.3 Per-palette accent tokens (dark mode) — the parity-check target

These are the tokens the automated check (`scripts/parity-check.mjs`) compares.
Tolerance: ±5 per channel (0–255). Background canvas (bg0/bg1/bg2) is also compared.

| Palette | Token | Web selector (index.css) | Android (Palette.kt) | Expected hex |
|---|---|---|---|---|
| graphite-mist | accent | index.css:387 `html[data-palette="graphite-mist"]` | Palette.kt:180 `GmAccent` | `#9DB7DF` |
| graphite-mist | bg0 | index.css:384 | Palette.kt:173 `GmBg0` | `#07090F` |
| graphite-mist | success | index.css:390 | Palette.kt:185 `GmSuccess` | `#7BE0B1` |
| graphite-mist | warning | index.css:391 | Palette.kt:186 `GmWarning` | `#FFCC6A` |
| graphite-mist | danger | index.css:392 | Palette.kt:187 `GmDanger` | `#FF7F8C` |
| liquid-blue | accent | index.css:418 `html[data-palette="liquid-blue"]` | Color.kt:53 `IdeAccent` (via `DarkIdeColors`) | `#4D8DFF` web / `#3D8BFF` Android — **known drift, delta=16** (see note below) |
| liquid-blue | bg0 | index.css:414 | Palette.kt:270 `LiquidBlueAurora.bg0` | `#061123` |
| deep-sky | accent | index.css:448 `html[data-palette="deep-sky"]` | Palette.kt:291 `DsAccent` | `#1F9CFF` |
| deep-sky | bg0 | index.css:444 | Palette.kt:288 `DsBg0` | `#021222` |
| nordic-cyan | accent | index.css:478 `html[data-palette="nordic-cyan"]` | Palette.kt:339 `NcAccent` | `#25D5B4` |
| nordic-cyan | bg0 | index.css:473 | Palette.kt:336 `NcBg0` | `#031216` |
| aurora-violet | accent | index.css:508 `html[data-palette="aurora-violet"]` | Palette.kt:387 `AvAccent` | `#9A7CFF` |
| aurora-violet | bg0 | index.css:503 | Palette.kt:384 `AvBg0` | `#11071F` |
| amber-night | accent | index.css:537 `html[data-palette="amber-night"]` | Palette.kt:435 `AnAccent` | `#FFAD33` |
| amber-night | bg0 | index.css:533 | Palette.kt:432 `AnBg0` | `#171008` |

> **Liquid Blue accent drift (known):** CSS `html[data-palette="liquid-blue"]` sets
> `--ide-accent-rgb: 77 141 255` (`#4D8DFF`) but Android's `DarkIdeColors.accent`
> in `Color.kt:53` is `IdeAccent = #3D8BFF` (`rgb 61,139,255`), a 16-unit delta on
> the red channel. This is a pre-existing divergence from before the palette system
> was introduced; the CSS palette block was tuned to `#4D8DFF` during the Liquid
> Glass campaign while Android's base `DarkIdeColors` was not updated. Fix: update
> `Color.kt:53 IdeAccent` to `Color(0xFF4D8DFF)` to match the CSS.
> Tracked in bd notes for CopyPaste-spj2.

### §A.4 Per-palette accent tokens (light mode, AA-tuned)

| Palette | Token | Web selector (index.css) | Android (Palette.kt) | Expected hex |
|---|---|---|---|---|
| graphite-mist light | accent | index.css:403 `html[data-theme="light"][data-palette="graphite-mist"]` | Palette.kt:705 `LightIdeColors.withAccent(0xFF3A6091)` | `#3A6091` |
| liquid-blue light | accent | index.css:432 | Palette.kt:706 `…withAccent(0xFF1A5FD4)` | `#1A5FD4` |
| deep-sky light | accent | index.css:462 | Palette.kt:707 `…withAccent(0xFF0070CC)` | `#0070CC` |
| nordic-cyan light | accent | index.css:491 | Palette.kt:708 `…withAccent(0xFF0A8F78)` | `#0A8F78` |
| aurora-violet light | accent | index.css:522 | Palette.kt:709 `…withAccent(0xFF5A35C8)` | `#5A35C8` |
| amber-night light | accent | index.css:552 | Palette.kt:710 `…withAccent(0xFFA05D00)` | `#A05D00` |

### §A.5 Light-palette accent tokens (their own native ramps)

| Palette | Token | Web selector (index.css) | Android (Palette.kt) | Expected hex |
|---|---|---|---|---|
| cloud-silver | accent | index.css:585 `html[data-theme="light"][data-palette="cloud-silver"]` | Palette.kt:480 `CsAccent` | `#5B8DEF` |
| cloud-silver | bg0 | index.css:583 | Palette.kt:483 `CloudSilverIdeColors.bg` | `#EDF2F8` |
| frost-blue | accent | index.css:619 | Palette.kt:526 `FbAccent` | `#2777FF` |
| frost-blue | bg0 | index.css:616 | Palette.kt:528 `FrostBlueIdeColors.bg` | `#EDF7FF` |
| porcelain | accent | index.css:655 | Palette.kt:572 `PorcAccent` | `#3C7DD9` |
| porcelain | bg0 | index.css:651 | Palette.kt:574 `PorcelainIdeColors.bg` | `#F3F6FA` |
| pearl-grey | accent | index.css:689 `html[data-theme="light"][data-palette="pearl-grey"]` | Palette.kt:618 `PearlAccent` | `#58677F` |
| pearl-grey | bg0 | index.css:687 | Palette.kt:620 `PearlGreyIdeColors.bg` | `#F1F1F2` |

### §A.6 Glass material tokens

| Token | Web (index.css) | Android (Palette.kt / Components.kt) | Expected value |
|---|---|---|---|
| `--glass-opacity` / `glassOpacity` (dark default) | index.css:179 `:root` | Palette.kt:233 `GraphiteMistLiquidTokens` | `0.40` web / `0.64` Android (different: web uses Tauri vibrancy) |
| `--glass-blur` / `glassBlurDp` | index.css:181 | Palette.kt:234 | `28px` / `28dp` |
| `--glass-saturation` / `saturation` | index.css:182 | Palette.kt:236 | `1.45` (Graphite Mist) |
| `--glass-opacity` (light mode) | index.css:303 `:root[data-theme="light"]` | Components.kt:573 `glassAlphaFor()` | `0.82` web / `0.62` Android |
| Glass fill (dark) | index.css:972 `.surface-glass` | Components.kt:460 `LiquidGlassSurface` | `rgba(surface-rgb, glass-opacity)` |
| Glass blur (strong tier) | index.css:953 `.surface-glass-strong` | Components.kt (GlassTier.STRONG) | `40px` / `40dp` |

> **Note on glass-opacity divergence:** Web glass-opacity is lower (0.40) because
> Tauri/WebKit's `NSVisualEffectView` already applies system-level vibrancy behind
> the window; Android uses direct RenderEffect blur, so it needs higher opacity (0.64)
> to achieve comparable visual weight. This intentional delta must NOT be treated as
> a parity failure by the automated check (it is excluded from token comparison).

### §A.7 Motion tokens

Both platforms use identical base durations and easing names.

| Token | Web (index.css) | Android (Components.kt / Motion.kt) | Value |
|---|---|---|---|
| `--motion-instant` | index.css:146 | `Motion.Instant` | `90ms` |
| `--motion-fast` | index.css:147 | `Motion.Fast` | `130ms` |
| `--motion-base` | index.css:148 | `Motion.Base` | `180ms` |
| `--motion-slow` | index.css:149 | `Motion.Slow` | `240ms` |
| `motionScale` cinematic | index.css:780 `--speed:.72` | Palette.kt:109 `MotionProfile.Cinematic=1.3f` | `0.72` speed scalar web / `1.3×` Android |
| Row stagger step | JS `STAGGER_STEP_MS` in views | Android row `animationDelay` | `~18–20ms`, cap 10 rows |

---

## §B. Component Cross-Reference

### §B.1 Shared components

| Component | Role | Web file:line | Android file:line | Contract |
|---|---|---|---|---|
| `Sidebar` / `CopyPasteBottomNav` | Primary nav | `components/Sidebar.tsx:43` | `MainActivity.kt` / nav host | Sidebar on desktop (≥640px), bottom tabs on phone |
| `ViewShell` / `CopyPasteTopBar` | View frame + header | `components/ViewShell.tsx:16` | `Components.kt:684` `fun CopyPasteTopBar` | Glass surface, 14px medium title, back button on Android |
| `CopyPasteCard` / `LiquidGlassSurface` | Glass card surface | `components/ViewShell.tsx` (surface-card) | `Components.kt:781` `fun CopyPasteCard` | `surface-card` / `GlassTier.CARD`, radius 12dp, 1px hairline border |
| `SectionHeader` / `SectionLabel` | Section label | `components/SectionHeader.tsx:20` | `Components.kt:1056` `fun SectionLabel` | Uppercase, 11px/11sp semibold, `--ide-dim` grey (NOT accent), tracking-wide |
| `ActionButton` / `CopyPasteButton` | Primary action button | `components/ActionButton.tsx:57` | `Components.kt:1503` `fun CopyPasteButton` | Accent fill, 12–13px/12sp label, radius 6dp, no glow shadow |
| `GlassToastItem` / `GlassToast` (TBD) | Toast notification | `components/Toast.tsx:41` | `Components.kt` (build task) | Glass card, semantic dot, slide-up 180ms |
| `IdeSwitch` | Toggle switch | `SettingsView.tsx` | `Components.kt:968` `fun IdeSwitch` | 34×18 track, white thumb ~12px, accent fill checked, 120ms, no glow |
| Segmented control | Row density / theme picker | `SettingsView.tsx:372` `TabBar` | `Components.kt` `SingleChoiceSegmentedButtonRow` | iOS-style: container `--ide-bg`, selected `--ide-elevated` + shadow |
| `SliderRow` / `SteppedSliderRow` | Stepped slider | `SettingsView.tsx:202` | `Components.kt:1143` `fun SteppedSliderRow` | 4px track, accent fill, 14px thumb, no Material state halo |
| `ContinuousSliderRow` | Continuous slider | (same file) | `Components.kt:1263` `fun ContinuousSliderRow` | Same geometry as SteppedSliderRow |
| `GlassAlertDialog` | Confirmation dialog | (web: custom modal) | `Components.kt:870` `fun GlassAlertDialog` | Glass card, blurred scrim, 16dp radius |
| Content-type chip | Kind badge | `components/FileChip.tsx` | `ContentType.kt` + chip composable | 9px/9sp semibold uppercase, radius 4dp, tinted fill + 1px tinted border |
| `DeviceCard` / `CopyPasteCard` (device) | Device identity block | `components/DeviceCard.tsx` | `DevicesActivity.kt:277` `fun DevicesScreen` | Inset divided list; glass card; own-device + peer rows |

### §B.2 Nav model

| Platform | Primary nav | Items | Inactive style | Active style |
|---|---|---|---|---|
| Web (desktop) | `Sidebar` — left panel | History, Devices, Settings, About, Log | dim icon (`--ide-dim`), no tint | accent icon + accent fill pill |
| Android | Bottom tab bar / `NavigationBar` | History, Devices, Settings (+ About/Log via Settings) | dim icon | accent icon + accent indicator |

**Contract:** Inactive nav icons MUST be `--ide-dim` / `IdeColors.dim` (uniform grey).
No rainbow tints on inactive items.

---

## §C. Screen Cross-Reference

| Screen | Web file:line | Android file:line | Notes |
|---|---|---|---|
| History | `views/HistoryView.tsx:44` (Toast at :44, view root) | `HistoryActivity.kt:213` `class HistoryActivity` | Row stagger, copy-flash 90ms, pin badge, kind chip |
| Devices | `views/DevicesView.tsx:627` `export function DevicesView` | `DevicesActivity.kt:277` `fun DevicesScreen` | Inset divided list; glass TopBar; QR blur+reveal |
| Settings | `views/SettingsView.tsx:160` `SettingsActivity` | `SettingsActivity.kt:160` `fun SettingsScreen` | Apple grouped-inset style; segmented density/theme pickers |
| About | `views/AboutView.tsx:32` `export function AboutView` | `AboutActivity.kt:93` `class AboutActivity` | App icon, version, links |
| Log | `views/LogView.tsx:36` `export function LogView` | `LogViewerActivity.kt:73` `class LogViewerActivity` | Level-filtered logcat/IPC log |
| Pair / QR | `views/DevicesView.tsx:72` `SasPairingModal` | `PairActivity.kt:162` `class PairActivity` | Privacy blur+reveal; countdown accent→warning at ≤20s; SAS modal glass |
| Onboarding | (web: initial wizard in App.tsx) | `OnboardingActivity.kt:107` `class OnboardingActivity` | Permissions flow; glass cards |

---

## 1. Color tokens (Apple system palette)

LIGHT (default):

| Token | Value | Role |
|---|---|---|
| `--ide-bg` | `#E3E3E8` | window canvas — greyish (systemGray5) |
| `--ide-panel` | `#F2F2F5` | sidebar / list — frosted near-white |
| `--ide-elevated` | `#FFFFFF` | cards, inputs |
| `--ide-raised` | `#ECECF0` | hover/pressed on elevated |
| `--ide-border` | `#D3D3D8` | hairline separators |
| `--ide-divider` | `#E2E2E6` | row separators |
| `--ide-text` | `#1D1D1F` | labelColor |
| `--ide-dim` | `#5B5B60` | secondaryLabel |
| `--ide-faint` | `#8A8A8E` | tertiaryLabel |
| `--ide-ghost` | `rgba(60,60,67,0.55)` | secondary metadata text |
| `--ide-ghost-deco` | `rgba(60,60,67,0.32)` | 24px+ decorative icons |
| `--ide-accent` | `#007AFF` | systemBlue |
| `--ide-accent-hover` | `#0063D1` | |
| `--ide-danger` | `#FF3B30` | systemRed |
| `--ide-success` | `#34C759` | systemGreen |
| `--ide-warning` | `#FF9500` | systemOrange |
| `--ide-info` | `#32ADE6` | systemTeal/cyan |
| `--ide-violet` | `#AF52DE` | systemPurple |

DARK (override) — keep the existing dark ramp (`#13141A` … `#E8EAED`), accent
`#3D8BFF`, semantic = the slightly-muted dark variants.

- **`IdeFaint` drift fix (P0):** Android currently `#6B6F78` (fails WCAG AA). Set
  Android faint to **`#82868F`** (dark) / **`#8A8A8E`** (light) and add
  `IdeGhost` / `IdeGhostDeco` tokens mirroring web.

## 2. Glass material

- Frosted translucent surface, blur(32px) saturate(180%), top highlight.
- LIGHT glass fill: `rgba(250,250,252,0.62)`, top highlight inset
  `rgba(255,255,255,0.70)`.
- DARK glass fill: `rgba(30,32,42,0.55)`, top highlight inset
  `rgba(255,255,255,0.08)`.
- Canvas behind glass must be **opaque** (gradient), so blur has something to
  sample. Light canvas: `linear-gradient(160deg,#ECECF1,#E3E3E9,#DADAE1)` + faint
  blue/violet radial glows. Dark canvas: the deep aurora gradient.
- Android: glass alpha **0.62 light / 0.55 dark** (was a flat 0.72); tune the
  light variant warm-near-white. Route ALL cards (incl. PeerCard / OwnDeviceCard /
  DiscoveredCard) through the one glass `CopyPasteCard` — no card bypasses it.

## 3. Typography

- Family: **Inter** (UI) + **JetBrains Mono** (code/mono), bundled on both.
- View title: **14px medium** (both). (web was 13/semibold.)
- Section label: **uppercase, 11px semibold, `--ide-dim` (grey, NOT accent)**,
  tracking-wide. (Apple section headers are grey, not blue.)
- Body / preview: 13px. Metadata / timestamp: 11px tabular-nums.
- Button label: 12–13px. Chip label: 9px semibold uppercase.

## 4. Shape / spacing / elevation

- Radii: chip/tag **4**, control **6**, **card 12** (both — web bumps 10→12),
  hero/modal 16.
- Spacing grid: 4 / 8 / 12 / 16 / 24.
- Border: **single 1px/1dp hairline** everywhere (Android: kill the 0.5dp mix).
- Elevation: flat hairline + subtle shadow (web `--ide-e2`). Android: **drop
  Material tonal elevation drift**; use the e2-equivalent shadow only.
- Disabled: **opacity 0.40** both (Android was 0.38).

## 5. Icons

- **Thin outline** family both. Web keeps **Lucide** (stroke 1.5). Android
  switches `Icons.Filled.*` → **`Icons.Outlined.*`** (closer to SF Symbols).
- Sizes: nav **18–20**, row/action **16**, header **18**.
- Pick one glyph per concept across platforms: pin = bookmark, "too large" =
  **one** glyph (choose the warning triangle), delete = trash, search = search.

## 6. Content-type chip — CANONICAL kind→color table (a spec, both classifiers)

Filled tint + **1px tinted border**, 9px semibold uppercase, radius 4.

| Kind | Color token |
|---|---|
| TEXT | accent (blue) |
| URL | info (teal) |
| EMAIL | success (green) |
| PHONE | success (green) |
| COLOR | warning (amber) |
| NUMBER | warning (amber) |
| PATH | warning (amber) |
| JSON | danger (red) |
| CODE | violet |
| IMAGE | violet |
| FILE | dim (grey) |
| PRIVATE / sensitive | danger (red) |

Both platforms render the chip **with** the border (Android adds it).

## 7. Controls

- **Switch:** one geometry — 34×18 track, white thumb ~12px, accent fill checked,
  **no glow shadow**, 120ms. Remove web's accent glow; align Android thumb size;
  unchecked thumb = white (not a dim dot).
- **Segmented control:** iOS-style (container `--ide-bg`, selected pill
  `--ide-elevated` + subtle shadow). Used for **Row density** (Comfortable/Compact)
  AND **theme** (System/Light/Dark). **Add to Android** (`SingleChoiceSegmentedButtonRow`);
  replace Android's density Switch.
- **Slider:** thin 4px track, accent fill, small 14px round thumb, **no Material
  state-layer halo**. Value label min 80px.
- **Tabs:** sliding accent underline, 180ms standard (already parity). Reconcile
  tab taxonomy: General / Display / Sync / Shortcuts / Storage / Advanced — phone
  may fold Shortcuts (no hardware kbd) but keep the same names for shared tabs.
- **Checkbox:** unify glyph (rounded box + check). Touch = always visible; desktop
  row checkbox may hover-reveal.

## 8. Surfaces & rows

- **Settings rows:** Apple grouped-inset style — rows inside a **card with
  dividers**, label left, control right. **Android wraps bare rows in cards +
  dividers.** Reconcile subtitle vs InfoPopover: keep web's `ⓘ` InfoPopover model;
  Android drops always-on subtitles in favour of the same affordance (or keeps
  subtitle but both platforms match).
- **Device cards:** Apple grouped **inset divided list** (web's model). **Convert
  Android's stacked raised Cards into a grouped inset list.**
- **Header:** **glass** on both (web `ViewShell` becomes glass; Android History
  header stops being solid). 14px medium title.
- **Dialogs/modals:** **glass cards** both, blurred scrim. **Restyle Android
  Material `AlertDialog`s to glass** (or custom).
- **Toast:** bespoke **glass toast** both (semantic dot + slide-up 180ms). **Build
  an Android `GlassToast` composable; replace Material Snackbar.**

## 9. Nav & badges

- Nav: **uniform dim inactive icon, accent active** (drop web's rainbow inactive
  tints). Bottom tabs on phone, sidebar on desktop. Add About + Logs reachability
  parity (phone reaches them via Settings — acceptable).
- Footer label: "CopyPaste" (match case), one size.
- **Sync-status badge:** 3 states — success / faint-idle / **danger-offline** —
  plus the 2s pulse when connected, plus tooltip. **Add offline(red)+pulse+tooltip
  to Android.**
- Origin/device/total badges: unify size (10px) + bordered, both.

## 10. QR / pairing

- **Privacy-first blur+reveal on BOTH.** Android already blurs+reveals; **add
  blur + tap/click-to-reveal to web QR.** Regenerating must NOT drop the blur
  state inadvertently (Android v5a bug).
- Countdown: **accent → warning at ≤20s**, "Expires in Ns". Unify color +
  threshold (Android moves 15→20s, faint→accent fill).
- SAS modal: glass card both; 28px mono SAS code.

## 11. Motion

- Tokens already parity (instant 90 / fast 130 / base 180 / slow 240; eases match).
- Row mount stagger: **~18–20ms step, cap 10 rows** (Android slows from 130ms).
- Copy-flash 90ms success (parity). Online pulse 2s (parity).
- Press-scale 0.98 on touch (Android) is fine; desktop uses hover instead.
- Selection: prefer the animated **glide layer** model (add to Android, or accept
  per-row bg if cost-prohibitive — glide is the Apple-canonical target).

---

### Cross-platform contracts (MUST match exactly, do not re-diverge)
§1 tokens · §3 type roles · §4 radii · §6 chip color table · §9 nav model ·
§A.3 palette accent table · §A.4 light-mode AA-tuned accents.

Any agent touching these uses the values above verbatim.
Run `node scripts/parity-check.mjs` to verify token alignment before merging.
