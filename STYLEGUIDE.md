# CopyPaste — Design Style Guide

**Status:** source of truth for the design migration.
**Scope:** macOS (Tauri 2 / React) **and** Android (Jetpack Compose).
**Supersedes:** the Liquid-Glass system (10 palettes × 3 skins × density/contrast/motion). That system is being removed in full — see [§12 Migration](#12-migration-old--new).

---

## 1. Philosophy

One design language. Calm, quiet, graphite. The interface gets out of the way so the **content** is the loudest thing on screen.

Five rules everything else follows:

1. **Color is information, not decoration.** Chroma is reserved for exactly two jobs: the **accent** (one user-chosen hue, for interactive/selected/brand) and the **content-type** code (teal = link, violet = code, …). Nothing else is colored. No colored rows, no rainbow chrome.
2. **One encoding per fact.** A clip's type is shown by its tile color *once*. We don't also tint the row, the border, and the text. Redundant encoding is the "social-feed" look we're leaving behind.
3. **Whitespace over borders.** Group by spacing and a single hairline, not boxes inside boxes.
4. **Quiet by default, loud on demand.** Sensitive data is masked, destructive actions are tinted red, banners appear *only* when something needs the user. Everything else is neutral.
5. **Two axes, nothing more.** The only appearance choices are **theme** (`dark` / `light`) and **accent** (6 hues). There are no skins, palettes, density modes, contrast modes, or motion modes.

---

## 2. The two axes

| Axis | Values | Web mechanism | Android mechanism |
|---|---|---|---|
| **Theme** | `dark` (default) · `light` | `<html data-theme="…">` | `isDarkTheme: Boolean` → `CpColors` |
| **Accent** | `indigo` (default) · `blue` · `teal` · `green` · `amber` · `rose` | `<html data-accent="…">` | `AccentColor` enum |

`prefers-reduced-motion` / system reduce-motion is honored automatically and is **not** a user setting.
Optional booleans that may remain in Settings: **Translucency** (frosted surfaces on/off) and **Mask sensitive data**. Everything else from the old appearance panel is deleted.

---

## 3. Color system

All values are tokens. Components **never** hardcode hex — they reference a token. Tints/borders are produced with `color-mix(in srgb, var(--token) N%, transparent)` on web and `token.copy(alpha = …)` on Android.

### 3.1 Surfaces

| Token | Role | Dark | Light |
|---|---|---|---|
| `--bg` | root / window | `#0E0F14` | `#F5F6F8` |
| `--panel` | primary surface (list, sidebar, nav, sheets) | `#16181F` | `#FFFFFF` |
| `--elevated` / `--card` | cards, inputs, menus | `#1E2027` | `#FFFFFF` |
| `--raised` | hover / pressed on elevated | `#282B33` | `#EFF1F4` |
| `--raised-2` | control track, disabled fill | `#33373F` | `#E2E5EA` |

### 3.2 Lines

| Token | Role | Dark | Light |
|---|---|---|---|
| `--border` | hairline outline (1px) | `#33363F` | `#E1E4E9` |
| `--divider` | row separators (subtler) | `#24262D` | `#ECEEF1` |

### 3.3 Text (WCAG AA verified on `--panel`)

| Token | Role | Dark | Light |
|---|---|---|---|
| `--text` | primary | `#E7E9EE` | `#1A1C22` |
| `--dim` | secondary / labels | `#9CA1AC` | `#565B66` |
| `--faint` | tertiary / meta | `#7E838E` | `#767B86` |
| `--mute` | disabled / placeholder | `#5C616B` | `#A2A7B1` |

### 3.4 Overlays

| Token | Dark | Light |
|---|---|---|
| `--hover` | `rgba(255,255,255,.045)` | `rgba(15,18,26,.045)` |
| `--pressed` | `rgba(255,255,255,.075)` | `rgba(15,18,26,.075)` |
| `--selected` | `accent @ 16%` | `accent @ 12%` |
| `--scrim` (modal backdrop) | `rgba(0,0,0,.55)` | `rgba(20,22,30,.28)` |

### 3.5 Accents

Each accent ships a base color, a lighter `--accent-2` (for text/icons on dark tinted surfaces), and `--on-accent` (text/icon laid on a filled accent button). Light theme deepens some hues to keep AA against white.

| Accent | Dark base | Light base | `--accent-2` | `--on-accent` |
|---|---|---|---|---|
| **indigo** (default) | `#6E5BFF` | `#5B49E0` | `#9C8FFF` | `#FFFFFF` |
| blue | `#3B82F6` | `#2563EB` | `#7CB0FF` | `#FFFFFF` |
| teal | `#13B8A6` | `#0E9E8C` | `#5FE0D2` | dark `#06302C` / light `#FFFFFF` |
| green | `#46C56A` | `#1FA85B` | `#84E29A` | dark `#062A12` / light `#FFFFFF` |
| amber | `#F5A524` | `#C77F1A` | `#FFC56B` | dark `#2A1B05` / light `#FFFFFF` |
| rose | `#F43F7E` | `#E11D6B` | `#FF85AC` | `#FFFFFF` |

### 3.6 Status

| Token | Role | Dark | Light |
|---|---|---|---|
| `--ok` | success, online | `#4FB866` | `#1FA85B` |
| `--warn` | warning, degraded | `#E0A33F` | `#C77F1A` |
| `--err` | error, destructive, offline | `#E5645F` | `#D64545` |
| `--info` | informational | `#5B9DFF` | `#2563EB` |

### 3.7 Content-type colors — the signature

Maps `HistoryEntry.kind` → color. This is the only place outside the accent where chroma lives. `PHONE` shares `NUMBER`'s cyan; `PATH` shares `FILE`'s blue.

| kind(s) | Token | Dark | Light | Tile icon |
|---|---|---|---|---|
| `TEXT` | `--c-text` | `#8B93A5` | `#6A7282` | lines |
| `URL` | `--c-url` | `#34D1BF` | `#0E9E8C` | link |
| `EMAIL` | `--c-mail` | `#4ED98A` | `#1FA85B` | envelope |
| `PHONE` | `--c-num` | `#5CC1CE` | `#1C8B9B` | phone |
| `CODE` | `--c-code` | `#A78BFA` | `#7C5CE6` | chevrons `</>` |
| `JSON` | `--c-json` | `#FB7B53` | `#DC5A2E` | braces `{}` |
| `NUMBER` | `--c-num` | `#5CC1CE` | `#1C8B9B` | hash |
| `COLOR` | `--c-color` | `#F5A524` | `#C77F1A` | **the swatch itself** |
| `PATH` / `FILE` | `--c-file` | `#5B9DFF` | `#2F6FE0` | document / folder |
| `IMAGE` | `--c-image` | `#E879C6` | `#C44BA0` | **the thumbnail itself** |
| `SECRET` (`is_sensitive`) | `--c-secret` | `#F2616B` | `#D64545` | lock |

**Tile rule:** the tile background is `content-color @ 14%`, the glyph is the content color at full strength. Two kinds render their *content* instead of a glyph: `COLOR` shows the actual swatch, `IMAGE` shows the actual thumbnail.

---

## 4. Typography

Two families. Inter for UI, JetBrains Mono for anything machine-shaped (clip previews, IPs, fingerprints, timestamps, code, hex, counts).

| Role | Family | Size | Weight | Line-height | Used for |
|---|---|---|---|---|---|
| Title | Inter | 21–24px | 700 | 1.2 | About, onboarding, mobile screen title |
| Section | Inter | 13–15px | 600 | 1.3 | view headers, settings group labels |
| Body / row title | Inter or **Mono** | 13–14px | 400–500 | 1.45 | clip preview, list rows |
| Meta | Inter | 11–12px | 400 | 1.4 | "Code · VS Code · 2h", device metadata values |
| Micro / eyebrow | Mono | 9.5–10.5px | 500–600 | 1 | chips, badges, group headers (UPPERCASE, `letter-spacing:.06–.1em`) |

- Mono numerics use `font-variant-numeric: tabular-nums` so timestamps/RTT/IPs don't shift width.
- Mono fallback stack: `ui-monospace, 'SF Mono', monospace`. Inter fallback: `-apple-system, system-ui, sans-serif`.

---

## 5. Spacing, radius, elevation

### Spacing scale (px)
`2 · 4 · 6 · 8 · 11 · 14 · 16 · 20 · 24`
Row vertical padding ~9–11. Card padding 14–15. Section gutters 20–24. The 11/14 steps give the calm, slightly-airy density.

### Radius (fixed — no skin)

| Token | Value | Used for |
|---|---|---|
| `--r-chip` | 7px | small chips, status pills, tiles |
| `--r-pill` | 999px | transport/filter pills, toggles |
| `--r-ctl` | 8px | buttons |
| `--r-input` | 9px | inputs / search |
| `--r-card` | 13px | cards, banners, modals |
| `--r-window` | 12px | desktop window corners |
| (mobile frame) | 40px | phone bezel · 30px tab bar · 22px quick-paste sheet |

### Elevation

Borders do most of the lifting; shadows are a faint assist, never a drop-shadow slab.

| Token | Dark | Light | Used for |
|---|---|---|---|
| `--sh1` | `0 1px 2px rgba(0,0,0,.30)` | `0 1px 2px rgba(20,22,30,.06)` | cards, raised rows |
| `--sh2` | `0 8px 24px -6px rgba(0,0,0,.45)` | `0 8px 24px -8px rgba(20,22,30,.12)` | popups, tab bar, menus |
| `--sh3` | `0 24px 64px -12px rgba(0,0,0,.60)` | `0 24px 64px -12px rgba(20,22,30,.18)` | modals, quick-paste sheet, phone |

---

## 6. Motion

Quiet, short, purposeful. No aurora canvas, no levitating cards, no parallax (all removed with Liquid-Glass).

| Token | Value | Used for |
|---|---|---|
| `--dur-fast` | 120ms | hover, press, toggle |
| `--dur` | 200ms | enter/exit, list insert, modal |
| `--dur-theme` | 300ms | theme/accent crossfade |
| `--ease` | `cubic-bezier(.2,.8,.2,1)` | everything |

- Theme switch crossfades `background` + `color` over `--dur-theme`.
- Online presence dot: a single soft glow (no expanding pulse ring).
- **Reduced motion:** all three durations collapse to `0ms`; the presence glow and any transform are disabled. This is automatic via `@media (prefers-reduced-motion: reduce)` (web) / `MotionSpec.reduced` (Android).

---

## 7. Accessibility

- **Contrast:** AA minimum — 4.5:1 body text, 3:1 large text & UI affordances. The text ramp in §3.3 is verified on `--panel` in both themes.
- **Focus:** every interactive element shows a `:focus-visible` ring — `2px solid var(--accent)`, `2px` offset. Never remove outlines without replacing them.
- **Hit targets:** ≥ 28px desktop, ≥ 44px mobile.
- **Sensitive data:** masked by **blur**, never deletion. Reveal is explicit and (optionally) warned. The masked text still occupies its real width.
- **Color is never the only signal:** online/offline = dot color **+** label; destructive = red **+** the word "Revoke"/"Unpair"; content-type = color **+** glyph **+** the type word in meta.
- **Keyboard:** all rows, chips, and actions are real focusable controls (`<button>` / proper Compose semantics), not click-only `<div>`s.

---

## 8. Iconography

- Line icons, ~1.6px stroke, 24×24 viewBox, `currentColor`, rounded joins/caps.
- Rendered at 1em and **always** given an explicit box (`width/height`) so an SVG never balloons to its intrinsic 300×150. *(This was a real bug; the rule is: every inline icon has a fixed size.)*
- Glyph reference: lucide-style set on web (`lucide-react`), the matching set in `NavIcons.kt` on Android.

---

## 9. Components

Each spec lists anatomy, the tokens it uses, and its states. Identical across platforms unless noted.

### 9.1 Buttons

| Variant | Fill | Text | Border | Use |
|---|---|---|---|---|
| **primary** | `--accent` | `--on-accent` | none | the one main action (Pair device, Save) |
| **secondary** | `--elevated` | `--text` | `--border` | neutral actions |
| **ghost** | transparent | `--dim` → `--text` on hover | none | low-emphasis (Dismiss, Cancel) |
| **danger** | `err @ 9%` | `--err` | `err @ 40%` | Unpair, Revoke, Delete |

Sizes: default `padding 7px 13px / 13px`; `sm` `5px 10px / 11.5px`. Radius `--r-ctl`. Hover = +overlay; press = `--pressed`; disabled = 45% opacity, no pointer.

### 9.2 Toggle & segmented control

- **Toggle:** 38×22 pill, track `--accent` (on) / `--raised-2` (off), 18px knob, `--dur-fast` slide. Radius `--r-pill`.
- **Segmented:** container `--card` + `--border`, 2px inset; active segment `--raised` + `--text` (500), inactive `--dim`. Radius `--r-ctl`, segments `--r-chip`. Used for the **Theme** (Light/Dark) switch.

### 9.3 Inputs / search

`--elevated` fill, `--border` (→ `--accent` on focus + focus ring), `--r-input`, leading search glyph in `--faint`, placeholder `--faint`. Mono font when the field holds machine input (Relay URL, etc.).

### 9.4 Chips & badges

| Chip | Shape | Color | Where |
|---|---|---|---|
| **content-type tile** | `--r-chip` square, 32–36px | bg `c-* @ 14%`, glyph `c-*` | every clip row |
| **type tag** (meta word) | text only | `c-*`, 500 | "**Code** · VS Code · 2h" |
| **transport** P2P | `--r-pill`, hairline | `--c-url` (sky/teal) on `@14%` | device header |
| **transport** Cloud | `--r-pill`, hairline | `--accent` on `@14%` | device header |
| **This Mac / This phone** | `--r-pill`, hairline | `--accent-2` on `accent @14%` | own-device header |
| **Verified** | `--r-chip`, hairline + dot | `--ok` on `@12%` | every paired peer |
| **count / result %** | text, mono | `--faint` | search results, headers |

Micro-type is uppercase mono, `9.5–10.5px`, `letter-spacing .06em`.

### 9.5 List row (clip) — anatomy

The core object. Left→right:

```
[ tile ]  preview (1 line, ellipsized)            [ pin ★ ] [ actions ]
          type · source app · relative time · origin device
```

- **tile** — §9.4 content-type tile (or swatch/thumbnail for COLOR/IMAGE).
- **preview** — `--text`, mono for code/url/path/json/number/color/secret, sans for text/email. Single line, `text-overflow: ellipsis`.
- **meta** — `--faint`, 11.5px; the type word is tinted `c-*`. Fields: `kind · sourceApp(app_bundle_id) · relTime(wall_time) · originDevice` (origin shown only when not this device).
- **pin** — pinned items show a `--c-color` star (fixed 13px). Pinned items group above Today.
- **actions** — appear on hover (desktop) / swipe or long-press (mobile): Pin, Delete (`--err`). Sensitive rows show a **Reveal** affordance instead of plain preview.
- **states:** hover `--hover`; selected `--selected` + accent left-edge; multi-select shows a checkbox in the tile's place.

### 9.6 Date group header

Sticky, `--faint`, uppercase mono 10px, `letter-spacing .1em`: `PINNED` · `TODAY` · `YESTERDAY` · `EARLIER`. No background slab — just the label over the list surface.

### 9.7 Card & **device card** (full spec)

Card: `--card` fill, `--border`, `--r-card`, `--sh1`, padding 14–15.

The **device card** is the reference layout for "show every field, same set for every peer." Two shapes:

**Own device ("This Mac" / "This phone")** — header: `●online` + name + `This Mac` pill. Then an aligned 2-column metadata grid:

| Label | Source |
|---|---|
| Model | `OwnDeviceInfo.device_model` |
| OS | `os_version` |
| Version | `app_version` |
| Local IP | `local_ip` |
| Public IP | `public_ip` |
| Fingerprint | `fingerprint` → `first16…last8`, tap-to-copy (copies full 64-hex) |

No actions (it's you).

**Paired peer** — header: `●online/offline` + name + transport pill (P2P/Cloud). Then a `Verified` badge, then the **same** grid for every peer:

| Label | Source | Notes |
|---|---|---|
| Model | `PairedDevice.model` | |
| OS | `os_version` | |
| Version | `app_version` | |
| Local IP | `local_ip` (or parsed from `address`) | |
| Public IP | `public_ip` | |
| Paired | `added_at` | absolute date |
| Last sync | `last_sync_at` | relative ≤24h, absolute beyond |
| RTT | `latency_ms` | `— ` when no live P2P link |

Footer: full-width **Unpair** + **Revoke**, both `danger`, equal width, separated by a top hairline.

**Grid mechanics:** CSS grid `auto / 1fr`, `gap 3px 14px`, baseline-aligned. Labels `--dim` 11px; values `--faint` 11px mono `tabular-nums`, `word-break: break-all`. A field row is hidden only when its value is genuinely absent — but a fully-synced peer shows all eight, so peers read as identical in weight.

> Cards are **natural height** (`align-items: start` in the grid) — a shorter card (own device, 6 rows) must not stretch to a taller neighbor and leave an internal gap.

### 9.8 Banner (conditional)

Full-width strip at the top of the content zone. Appears **only** when actionable. Anatomy: `[icon] message (problem + fix, in the app's voice) [action(s)]`, vertically centered (`align-items: center`).

| Variant | Tint | Example |
|---|---|---|
| warn | `--warn` | needs Accessibility access → *Open Settings* |
| error | `--err` | background service stopped → *Restart* |
| info | `--info` | app/daemon version mismatch → *Restart app* / *Dismiss* |
| success | `--ok` | accessibility granted (auto-dismiss) |

Dismissible only where it's safe to ignore.

### 9.9 Modal / confirm

Centered, `--panel`, `--border`, `--r-card`, `--sh3`, over `--scrim`. Title (600), body (`--dim`), actions right-aligned (`ghost` cancel + `primary`/`danger` confirm). Destructive confirms name the device and use `danger`.

### 9.10 Empty states

Centered, generous (min-height ~300), `--faint`. A line icon + one-line headline + one-line hint. Variants: empty history, no search results, no paired devices.

### 9.11 Sidebar (desktop)

`--panel`, fixed nav: History · Devices · Settings · Logs · About. Active item = `--selected` + `--text`; inactive `--dim`. Footer (sync status / version) pinned to the bottom via `margin-top:auto`.

### 9.12 Tab bar (mobile) — floating pill

The Android shell. A floating, frosted **pill** (`--r-pill`/30px) inset 12px from the screen edges, 10–12px above the bottom inset. Background `card @ 90%` + `backdrop-filter: blur(22px)` + `--border` + `--sh2`. Three tabs: **Clips · Devices · Settings**. Active tab: icon sits in an `accent @ 18%` rounded "ti" pill, label + icon `--accent`; inactive `--faint`. A `--bg` gradient fade sits under the bar so content scrolls away cleanly.

### 9.13 Popup (desktop) / Quick-paste sheet (mobile)

- **Desktop popup** — compact, separate window: search field + condensed clip rows. Capped height; keyboard-first.
- **Mobile quick-paste** — a bottom sheet (`--panel`, `--r-card` top, `--sh3`) over a dimmed app: drag handle + search + condensed rows; tap a clip to paste. Summoned from a quick-settings tile / share target. This is Android's answer to the popup window.

---

## 10. Platform implementation — Web

Drop in this file as `crates/copypaste-ui/src/styles/tokens.css`. Delete `skin.css`. Remove every `[data-palette]` / `[data-skin]` block from the other CSS files.

```css
/* ============================================================
   CopyPaste tokens — single source of truth.
   Axes: data-theme = dark|light   ×   data-accent = indigo|blue|teal|green|amber|rose
   No skins, no palettes.
   ============================================================ */

:root, :root[data-theme="dark"]{
  color-scheme: dark;
  --bg:#0E0F14; --panel:#16181F; --elevated:#1E2027; --card:#1E2027; --raised:#282B33; --raised-2:#33373F;
  --border:#33363F; --divider:#24262D;
  --text:#E7E9EE; --dim:#9CA1AC; --faint:#8F94A0; --mute:#5C616B;
  --hover:rgba(255,255,255,.045); --pressed:rgba(255,255,255,.075);
  --selected:color-mix(in srgb,var(--accent) 16%,transparent); --scrim:rgba(0,0,0,.55);
  --ok:#4FB866; --warn:#E0A33F; --err:#E5645F; --info:#5B9DFF;
  --err-strong:var(--err); --info-strong:var(--info); --ok-strong:var(--ok);
  --c-text:#8B93A5; --c-url:#34D1BF; --c-code:#A78BFA; --c-image:#E879C6; --c-mail:#4ED98A;
  --c-color:#F5A524; --c-num:#5CC1CE; --c-path:#5B9DFF; --c-file:#5B9DFF; --c-json:#FB7B53; --c-secret:#F2616B;
  --sh1:0 1px 2px rgba(0,0,0,.30); --sh2:0 8px 24px -6px rgba(0,0,0,.45); --sh3:0 24px 64px -12px rgba(0,0,0,.60);
}

:root[data-theme="light"]{
  color-scheme: light;
  --bg:#F5F6F8; --panel:#FFFFFF; --elevated:#FFFFFF; --card:#FFFFFF; --raised:#EFF1F4; --raised-2:#E2E5EA;
  --border:#E1E4E9; --divider:#ECEEF1;
  --text:#1A1C22; --dim:#565B66; --faint:#6E7380; --mute:#A2A7B1;
  --hover:rgba(15,18,26,.045); --pressed:rgba(15,18,26,.075);
  --selected:color-mix(in srgb,var(--accent) 12%,transparent); --scrim:rgba(20,22,30,.28);
  --ok:#1FA85B; --warn:#C77F1A; --err:#D64545; --info:#2563EB;
  --err-strong:#B93434; --info-strong:#1D4ED8; --ok-strong:#157A42;
  --c-text:#6A7282; --c-url:#0E9E8C; --c-code:#7C5CE6; --c-image:#C44BA0; --c-mail:#1FA85B;
  --c-color:#C77F1A; --c-num:#1C8B9B; --c-path:#2F6FE0; --c-file:#2F6FE0; --c-json:#DC5A2E; --c-secret:#D64545;
  --sh1:0 1px 2px rgba(20,22,30,.06); --sh2:0 8px 24px -8px rgba(20,22,30,.12); --sh3:0 24px 64px -12px rgba(20,22,30,.18);
}

/* accents (both themes) */
:root[data-accent="indigo"]{--accent:#6E5BFF;--accent-2:#9C8FFF;--on-accent:#fff}
:root[data-accent="blue"]  {--accent:#3B82F6;--accent-2:#7CB0FF;--on-accent:#fff}
:root[data-accent="teal"]  {--accent:#13B8A6;--accent-2:#5FE0D2;--on-accent:#06302C}
:root[data-accent="green"] {--accent:#46C56A;--accent-2:#84E29A;--on-accent:#062A12}
:root[data-accent="amber"] {--accent:#F5A524;--accent-2:#FFC56B;--on-accent:#2A1B05}
:root[data-accent="rose"]  {--accent:#F43F7E;--accent-2:#FF85AC;--on-accent:#fff}
:root:not([data-accent])   {--accent:#6E5BFF;--accent-2:#9C8FFF;--on-accent:#fff}

/* light-theme accent tuning (AA on white) */
:root[data-theme="light"][data-accent="indigo"]{--accent:#5B49E0}
:root[data-theme="light"][data-accent="blue"]  {--accent:#2563EB}
:root[data-theme="light"][data-accent="teal"]  {--accent:#0E9E8C;--on-accent:#fff}
:root[data-theme="light"][data-accent="green"] {--accent:#1FA85B;--on-accent:#fff}
:root[data-theme="light"][data-accent="amber"] {--accent:#C77F1A;--on-accent:#fff}
:root[data-theme="light"][data-accent="rose"]  {--accent:#E11D6B}

:root{
  --f-ui:'Inter',-apple-system,system-ui,sans-serif;
  --f-mono:'JetBrains Mono',ui-monospace,'SF Mono',monospace;
  --r-chip:7px; --r-pill:999px; --r-ctl:8px; --r-input:9px; --r-card:13px; --r-window:12px;
  --s-1:2px;--s-2:4px;--s-3:6px;--s-4:8px;--s-5:11px;--s-6:14px;--s-7:16px;--s-8:20px;--s-9:24px;
  --dur-fast:120ms; --dur:200ms; --dur-theme:300ms; --ease:cubic-bezier(.2,.8,.2,1);
}
@media (prefers-reduced-motion: reduce){ :root{--dur-fast:0ms;--dur:0ms;--dur-theme:0ms} }
```

**Wiring (`App.tsx`)** — reduce to two attributes:

```tsx
useEffect(() => {
  const el = document.documentElement;
  el.dataset.theme  = theme;   // "dark" | "light"
  el.dataset.accent = accent;  // "indigo" | … | "rose"
}, [theme, accent]);
```

Delete `data-palette`, `data-skin`, `data-density`, `data-motion`, `data-contrast` writes.

---

## 11. Platform implementation — Android

Replace `Palette.kt` + `Skin.kt` with one semantic token holder + an accent enum. `Theme.kt` collapses to `isDark × accent`.

```kotlin
// ui/theme/Color.kt — semantic tokens (dark + light). No palettes, no skins.
package com.copypaste.android.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

@Immutable
data class CpColors(
    val bg: Color, val panel: Color, val elevated: Color, val raised: Color, val raised2: Color,
    val border: Color, val divider: Color,
    val text: Color, val dim: Color, val faint: Color, val mute: Color,
    val ok: Color, val warn: Color, val err: Color, val info: Color,
    val okStrong: Color, val errStrong: Color, val infoStrong: Color,
    val cText: Color, val cUrl: Color, val cCode: Color, val cImage: Color, val cMail: Color,
    val cColor: Color, val cNum: Color, val cPath: Color, val cFile: Color, val cJson: Color, val cSecret: Color,
)

val DarkColors = CpColors(
    bg = Color(0xFF0E0F14), panel = Color(0xFF16181F), elevated = Color(0xFF1E2027),
    raised = Color(0xFF282B33), raised2 = Color(0xFF33373F),
    border = Color(0xFF33363F), divider = Color(0xFF24262D),
    text = Color(0xFFE7E9EE), dim = Color(0xFF9CA1AC), faint = Color(0xFF8F94A0), mute = Color(0xFF5C616B),
    ok = Color(0xFF4FB866), warn = Color(0xFFE0A33F), err = Color(0xFFE5645F), info = Color(0xFF5B9DFF),
    okStrong = Color(0xFF4FB866), errStrong = Color(0xFFE5645F), infoStrong = Color(0xFF5B9DFF),
    cText = Color(0xFF8B93A5), cUrl = Color(0xFF34D1BF), cCode = Color(0xFFA78BFA), cImage = Color(0xFFE879C6),
    cMail = Color(0xFF4ED98A), cColor = Color(0xFFF5A524), cNum = Color(0xFF5CC1CE),
    cPath = Color(0xFF5B9DFF), cFile = Color(0xFF5B9DFF), cJson = Color(0xFFFB7B53), cSecret = Color(0xFFF2616B),
)

val LightColors = CpColors(
    bg = Color(0xFFF5F6F8), panel = Color(0xFFFFFFFF), elevated = Color(0xFFFFFFFF),
    raised = Color(0xFFEFF1F4), raised2 = Color(0xFFE2E5EA),
    border = Color(0xFFE1E4E9), divider = Color(0xFFECEEF1),
    text = Color(0xFF1A1C22), dim = Color(0xFF565B66), faint = Color(0xFF6E7380), mute = Color(0xFFA2A7B1),
    ok = Color(0xFF1FA85B), warn = Color(0xFFC77F1A), err = Color(0xFFD64545), info = Color(0xFF2563EB),
    okStrong = Color(0xFF157A42), errStrong = Color(0xFFB93434), infoStrong = Color(0xFF1D4ED8),
    cText = Color(0xFF6A7282), cUrl = Color(0xFF0E9E8C), cCode = Color(0xFF7C5CE6), cImage = Color(0xFFC44BA0),
    cMail = Color(0xFF1FA85B), cColor = Color(0xFFC77F1A), cNum = Color(0xFF1C8B9B),
    cPath = Color(0xFF2F6FE0), cFile = Color(0xFF2F6FE0), cJson = Color(0xFFDC5A2E), cSecret = Color(0xFFD64545),
)

// onDark/onLight = text laid on a filled accent; variant = accent-2 for tinted surfaces.
enum class AccentColor(
    val dark: Color, val light: Color, val onDark: Color, val onLight: Color, val variant: Color,
) {
    INDIGO(Color(0xFF6E5BFF), Color(0xFF5B49E0), Color.White,        Color.White, Color(0xFF9C8FFF)),
    BLUE  (Color(0xFF3B82F6), Color(0xFF2563EB), Color.White,        Color.White, Color(0xFF7CB0FF)),
    TEAL  (Color(0xFF13B8A6), Color(0xFF0E9E8C), Color(0xFF06302C),  Color.White, Color(0xFF5FE0D2)),
    GREEN (Color(0xFF46C56A), Color(0xFF1FA85B), Color(0xFF062A12),  Color.White, Color(0xFF84E29A)),
    AMBER (Color(0xFFF5A524), Color(0xFFC77F1A), Color(0xFF2A1B05),  Color.White, Color(0xFFFFC56B)),
    ROSE  (Color(0xFFF43F7E), Color(0xFFE11D6B), Color.White,        Color.White, Color(0xFFFF85AC));
    fun base(isDark: Boolean) = if (isDark) dark else light
    fun on(isDark: Boolean)   = if (isDark) onDark else onLight
}
```

```kotlin
// ui/theme/Theme.kt — collapse to isDark × accent. Provide tokens + a Material3 scheme.
val LocalCpColors = staticCompositionLocalOf { DarkColors }
val LocalAccent   = staticCompositionLocalOf { AccentColor.INDIGO }

@Composable
fun CopyPasteTheme(
    isDark: Boolean = isSystemInDarkTheme(),
    accent: AccentColor = AccentColor.INDIGO,
    content: @Composable () -> Unit,
) {
    val c = if (isDark) DarkColors else LightColors
    val scheme = (if (isDark) darkColorScheme() else lightColorScheme()).copy(
        primary    = accent.base(isDark),
        onPrimary  = accent.on(isDark),
        background  = c.bg,  onBackground = c.text,
        surface     = c.panel, onSurface = c.text,
        surfaceVariant = c.elevated, outline = c.border,
        error = c.err,
    )
    CompositionLocalProvider(LocalCpColors provides c, LocalAccent provides accent) {
        MaterialTheme(colorScheme = scheme, typography = CpTypography, shapes = CpShapes, content = content)
    }
}
```

`Shapes.kt` → fixed radii from §5 (no skin radius). Keep `Type.kt`, `MotionSpec.kt` (drop the cinematic/calm split — keep `reduced`), `NavIcons.kt`, `Components.kt` (de-skin: read from `LocalCpColors`/`LocalAccent`).

> **Parity:** the field names in `CpColors` / `AccentColor` are the new cross-platform contract. The web `tokens.css` variable names map 1:1 (e.g. `cText` ↔ `--c-text`). Update the CI parity test to compare these instead of the old palette/skin registries.

---

## 12. Migration: old → new

What gets **deleted** vs **rewritten**. (Inventory verified against the repo.)

### Web — `crates/copypaste-ui/`

| Path | Action |
|---|---|
| `src/lib/skins.ts` | **delete** (skin registry: classic/quiet/vapor) |
| `src/lib/liquid-tokens.ts` | **delete** (accent2/3, glass params, glow, aurora, motionScale) |
| `src/styles/skin.css` | **delete** |
| `src/styles/tokens.css` | **rewrite** → §10 (remove all 10 `[data-palette]` blocks + contrast profiles) |
| `src/styles/components.css`, `animations.css`, `base.css` | **edit** — strip `[data-skin]`/`[data-palette]` selectors, `--skin-r-*` radius refs → fixed radii, remove aurora/glass keyframes |
| `src/App.tsx` | **edit** — write only `data-theme` + `data-accent`; delete density/contrast/motion/palette/skin |
| `src/store.ts` | **edit** — theming state → `{ theme, accent, translucency?, maskSensitive? }`; drop `palette`, `skin`, `density`, `contrast`, `motion` (+ keep the v1→v2 migration shim that resets stale keys) |
| `src/views/SettingsView/tabs/DisplayTab.tsx` | **rewrite** — Theme segmented + Accent picker (+ Translucency toggle); delete palette grid / skin selector / density / contrast / motion |
| `src/views/HistoryView.tsx`, `HistoryView/HistoryRow.tsx`, `popup/Popup.tsx` | **edit** — remove skin-aware classes & `--skin-r-*`; use tokens directly |
| `src/components/DeviceCard.tsx`, others using `--skin-r-chip`/`--skin-r-ctl` | **edit** — swap to `--r-chip`/`--r-ctl` |

### Android — `android/app/src/main/java/com/copypaste/android/`

| Path | Action |
|---|---|
| `ui/theme/Palette.kt` | **delete** (multi-palette ramps, LiquidTokens, AuroraDef) |
| `ui/theme/Skin.kt` | **delete** (classic/quiet/vapor structural skins) |
| `ui/theme/Color.kt` | **rewrite** → §11 (`CpColors` + `AccentColor`) |
| `ui/theme/Theme.kt` | **rewrite** → §11 (`isDark × accent`) |
| `ui/theme/Shapes.kt` | **edit** — fixed radii, no skin |
| `ui/theme/Components.kt` | **edit** — read `LocalCpColors`/`LocalAccent`; drop glass/aurora |
| `ui/theme/MotionSpec.kt` | **edit** — keep `reduced`; drop cinematic/calm split |
| `DisplayTab.kt`, `SettingsActivity.kt` | **edit** — Theme + Accent picker; delete palette/skin/density/contrast/motion |
| any composable referencing `LocalSkin`/`LocalPalette`/`auroraCanvas` | **edit** — re-point to new locals; delete aurora background |

### Tests / CI

- Update or remove: skin-parity (`lib/skins.ts` ↔ `Skin.kt`), palette-drift, liquid-glass/skin-token tests, `SettingsParityTest`, any `data-skin`/`data-palette` assertions, aurora tests.
- Add: a token-parity test comparing `tokens.css` variable names ↔ `CpColors`/`AccentColor` fields, and a contrast (AA) check for the text ramp in both themes.

### Definition of done

- [ ] No occurrence of `data-palette`, `data-skin`, `data-density`, `data-motion`, `data-contrast`, `skin`, `palette`, `aurora`, `liquid` in either app (outside changelog/history).
- [ ] Settings → Appearance shows **only** Theme + Accent (+ optional Translucency, Mask sensitive).
- [ ] Every screen on both platforms renders correctly in `dark`/`light` × all 6 accents.
- [ ] All UI strings English. AA passes. Reduced-motion respected. No unsized SVG.
- [ ] Both apps build; test suites green.

---

*This guide is the single source of truth. If an implementation detail isn't here, prefer the calmest option consistent with §1.*
