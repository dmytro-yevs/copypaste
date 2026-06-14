# Cross-Platform Parity Spec — Apple macOS Tahoe "Liquid Glass"

Canonical, single-source design contract for **web/desktop (copypaste-ui)** and
**Android (Compose)**. Target aesthetic: Apple macOS 26/27 (Tahoe) "Liquid Glass" —
**greyish, translucent, frosted, light-first**. Behaviour and appearance must be
**identical across platforms, adapted to platform conventions** (mouse/hover on
desktop, touch/press on phone; sidebar on desktop, bottom tabs on phone).

This doc resolves every "pick one" from the parity audit. When an agent must
choose a value, it uses the number HERE — not its platform's old value.

---

## 0. Theme & default

- **Light-first.** Default theme is **light** on both platforms.
  - Web: `<html data-theme="light">` is the default (set in `index.html`); the
    saved pref re-syncs via `App.tsx`. Dark is an override block.
  - Android: default to **light**; a Settings control lets the user pick
    System / Light / Dark (parity with web's theme control). Follow-OS only when
    the user selects "System".
- Both platforms ship a user-facing **theme control** (System / Light / Dark).

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
§1 tokens · §3 type roles · §4 radii · §6 chip color table · §9 nav model.
Any agent touching these uses the values above verbatim.
