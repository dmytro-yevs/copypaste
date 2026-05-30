# CopyPaste Design System v2 — "Quiet Precision"

Status: APPROVED DIRECTION (user: "відполірувати настільки, щоб в рамочку хотіли ставити; максимально зручний інтерфейс"). Target: Raycast-grade instant feel, Linear-grade restraint, Things-grade warmth. Keep Darcula-blue base; tighten everything to exact values. These tokens are the new source of truth.

## 0. Token-drift fix (DO FIRST — free, unblocks everything)

Web and Android palettes have DIVERGED and must be reconciled to ONE ramp before anything else:

| Token | Web (index.css/tailwind) | Android (Color.kt) | NEW canonical |
|---|---|---|---|
| bg | #13141a | #1E1F22 | #13141A |
| panel | #1e1f26 | #2B2D30 | #1B1C22 |
| elevated | #272930 | #313438 | #23252D |
| accent | #3592ff | #3574F0 | #3D8BFF |
| text | #e8eaed | #DFE1E5 | #E8EAED |

Pick one ramp, write once, mirror verbatim. Android Color.kt comment block citing #1e1f22/#3574f0 is stale + misleading.

## 1. Typography
Fonts — web: `-apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter var", system-ui`; mono: `"SF Mono", ui-monospace, "JetBrains Mono", Menlo`. Android: system sans + bundle JetBrains Mono FontFamily for mono (none today). sp↔px 1:1.

| Role | Size | Weight | LH | Tracking | Color |
|---|---|---|---|---|---|
| View title | 13 | 590 | 18 | +0.2 | text |
| Section label | 11 | 600 | 14 | +0.6 UPPER | faint |
| Body/label | 13 | 400 | 18 | 0 | text/dim |
| Clip text | 13 | 420 | 17 | 0 | text |
| Metadata/time | 11 | 450 | 14 | +0.1 | faint |
| Mono content | 12 | 400 | 16 | 0 | dim |
| Keycap hint | 10.5 | 500 | 1 | +0.3 | faint |

Never < 10.5px. Timestamps/numbers `tabular-nums`. Kill one-off `text-[10px]`/`text-xl` (AboutView app-name = single 18px exception).

## 2. Spacing & grid
Base 4px. Scale 2/4/6/8/12/16/20/24/32. Radii: 2 (highlight) · 4 (chips/keycaps) · 6 (inputs/buttons) · 10 (cards) · 14 (popup) · full (toggles/dots). Row heights via `prefs.density`: comfortable text 34px (default), compact 28px; image row = imageMaxHeight+12/+8. List h-pad 12px; card pad 12×10; view pad 16px; sidebar 208px; header 44px everywhere.

## 3. Color & elevation
Semantic set (base / container @~10% / use): accent #3D8BFF / rgba(61,139,255,.12) · success #5FAD65 / .10 · warning #D9A343 / .10 (pinned/degraded/expiry) · danger #E05C5C / .10 · info/url #56B6C2 / .12 · code/violet #C678DD / .12 (image+code).
Elevation: E0 none · E1 `0 1px 2px rgba(0,0,0,.40)`+1px border · E2 `0 2px 8px rgba(0,0,0,.45),0 1px 2px rgba(0,0,0,.35)` · E3 popup `0 12px 40px rgba(0,0,0,.55),0 2px 8px rgba(0,0,0,.40)`+inset top highlight `inset 0 1px 0 rgba(255,255,255,.06)`.
Focus ring (`:focus-visible` only): `0 0 0 1px var(--bg),0 0 0 3px rgba(61,139,255,.45)`.
States (tokenize — kill `bg-white/10` magic): hover rgba(255,255,255,.045) · selected rgba(61,139,255,.16)+2px accent left-bar · pressed .07 · multi-sel rgba(61,139,255,.20).
Translucency (`prefs.translucency`, ON default): ON container rgba(19,20,26,.72)+blur(30px) saturate(180%); OFF solid. Single `.surface-glass`/`.surface-solid` pair. Android: alpha swap + RenderEffect blur API31+, solid fallback <31.

## 4. THE POPUP (top priority — make it Raycast)
720×440, radius 14, E3, glass, frameless. Entrance scale .97→1 + opacity + translateY 4→0, 160ms cubic-bezier(.16,1,.3,1); exit 110ms ease-in. Respect prefers-reduced-motion.
Search bar 44px: Lucide search 16px, input 15px, right "23 of 50" count, bottom hairline.
Row (34px, 8px gap, 12px h-pad): (1) source-app icon 16px rounded-4 [needs daemon source_bundle_id; fallback type glyph] (2) content-type chip 14px tinted (text=accent T, url=link, image=violet frame, code=</>) — promote ContentIcon to shared component (3) primary label 13px ellipsis; fuzzy highlight = accent color+bg rgba(61,139,255,.16) radius2, DROP the bold weight (causes width shift) (4) right cluster fixed-width: rel time 11px tabular-nums · pin (warning, if pinned) · Cmd1-9 keycap on first 9 rows when no query.
Selected row: selection fill + 2px accent left-bar + keycap brightens; highlight GLIDES 120ms. Image row: thumb + violet chip + time, rounded-4 overflow-hidden + 1px border.
Keys: ⌘1-9 paste Nth · ↑↓ nav · ⏎ paste · ⌘⏎ plain (future) · Esc close. Footer real keycap pills.
Empty: clipboard 28px "Nothing copied yet" / search-x "No matches for '{q}'" / plug-zap "Clipboard service offline"+Restart.

## 5. History list
Row (comfortable 34px, respect density): checkbox 16px reserved (hover 0→60%) · pin (if pinned) · ContentIcon 14px (+ optional TEXT/URL/IMAGE/CODE pill in comfortable) · content previewLines-clamped (URL: host in text, rest dim) · right cluster fixed min-4.5rem: timestamp always (tabular-nums) + hover-reveal icon-only pin/delete. Copy-on-click (keep) + 90ms success flash + toast. Multi-select bulk bar → NEUTRAL E2 (not amber). Density drives height/padding/chip-label.

## 6. Settings
Tabs (General/Display/Sync/Shortcuts/Storage/Advanced) with animated active underline (slide 180ms). Cards E1 10px 1px border; rows divided by hairlines, min-h 36px, label min-w-160 dim, control right; help 11px faint max-w-320.
LIMIT SLIDERS — replace ALL Storage number inputs with stepped sliders snapping to fixed arrays + safe-high final step:
- Max text: [1,2,5,10,15,25,50,100] MB → "100 MB (max)"
- Max image: [5,10,25,64,128,256,512] MB → "512 MB (max)"
- Max file: [64MB,128,256,512,1GB,2GB] → "2 GB (max)"
- Local quota: [1,2,5,10,25,50] GB → "50 GB (max)"
- Max items: [100,250,500,1000,2500,5000,10000,∞] → final Unlimited (sentinel ~100000)
- Sensitive auto-wipe: [10s,30s,60s,5m,15m,1h]
- Image quality: keep 1-100 slider
Slider: track 4px elevated, fill accent, thumb 14px white E2 + accent focus-ring, tick marks per step, value label fixed 80px right ("15 MB"). Snap to indices (never unsafe). Save on release + "Saved" badge. ARRAYS MUST INCLUDE/EXCEED current core defaults (text 15MiB, image 64MiB) so existing configs snap cleanly.
density + translucency toggles = first two rows of Display. One Toggle/Select/Checkbox sharing §3 focus ring.

## 7. Devices
Rich cards E1 10px 12px-pad. Header: status dot + name 13px-medium + badges. Online pulse 8px dot success+expanding-ring keyframe 2s when online / faint offline / warning reconnecting [needs daemon last-heartbeat; fallback addr+seen<60s]. Transport chip P2P(info)/Cloud(accent) 10px upper. This-device badge accent. Fields: Model·OS·App ver·Local IP·Last seen (rel, tabular). Fingerprint mono full+copy on own, 16…8 truncated+hover-copy on peers. Per-card sync line "Synced 2m ago"/"Syncing…"/"Paused (Wi-Fi only)". Unpair/Revoke hover-reveal (Revoke danger). QR countdown = thin determinate drain bar (warning <20s).

## 8. Motion
Tokens (`:root` + Android `object Motion`): instant 90 / fast 130 / base 180 / slow 240ms. Eases: out cubic-bezier(.16,1,.3,1) · standard (.2,0,0,1) · in (.4,0,1,1). Current global 120ms = name it fast.
Hover 130ms standard. Press scale .98 90ms. List stagger on mount only (18ms/row cap 10, 160ms each) — NEVER on filter (instant). Selection glide = single abs-positioned layer animating top/height 130ms. Toast bottom-center E2 10px, slide-up+fade, 2.5s, neutral panel + 6px semantic dot, one at a time. Copy flash success 90ms.

## 9. Empty states & icons
ONE set: Lucide (matches existing Feather glyphs 1.5px/24-grid). Replace emoji (▣🔒⚑↗•) + inline SVG. Sizes 14 inline/16 control/18 nav/28 hero, stroke 1.5, currentColor. Empty pattern: hero 28px faint (never accent) + title 13 dim + sentence 11 faint + optional E1 action. Copy per §4/§9 list.

## 10. Platform parity (Android Compose)
Reconcile Color.kt to §0. Type.kt mirrors tiers + add tnum + JetBrains Mono family. Shapes: align card radius (recommend 12 both). Elevation via explicit shadow()+border (tonal disabled — keep). Translucency RenderEffect API31+. Popup has no Android equivalent (no global hotkey) → apply row anatomy/chips/empty to History/share-target; ⌘1-9 desktop-only. History: combinedClickable tap/long-press (present). Settings: Material3 Slider(steps=n-2) same arrays/labels/Unlimited. Devices: infiniteTransition pulse ring. Motion: object Motion + CubicBezierEasing + animate*AsState + AnimatedVisibility toast. Icons: lucide-compose ImageVector replacing Material filled.

## Execution order (low→high risk, max perceived quality first)
1. Token reconcile + naming (§0/§3/§8) — zero behavior, unblocks all, fixes drift.
2. Lucide icon sweep (§9) — pure visual.
3. Popup redesign (§4) — highest payoff, self-contained.
4. Empty states + toast (§8/§9).
5. History row + density (§5/§2) — touches rowHeightFor virtualizer.
6. Settings stepped sliders (§6) — data-layer; Unlimited sentinel ↔ core.
7. Devices rich cards + pulse (§7) — needs daemon liveness.
8. Selection glide + stagger (§8) — defer-able.
9. Android parity (§10) — mirror 1-8 after web settles.

## Release risks
- rowHeightFor coupling: row-height change must update virtualizer prefix-sum + rendered height together.
- Popup hide/focus: animate on SHOW only; on hide fire hide_popup immediately (don't await exit anim — reintroduces focus flicker; isHidingRef guard V-10/11/12).
- Unlimited sentinel must map to core-accepted unbounded; step arrays must include/exceed current defaults (text 15MiB/image 64MiB).
- Translucency default ON: every surface needs BOTH glass+solid classes or toggle leaves gaps; Android blur API31+ only, <31 solid mandatory.
- Source-app icon depends on daemon source_bundle_id — ship type-chip fallback, add icon when data lands; don't block popup on it.
