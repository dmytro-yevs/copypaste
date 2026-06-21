package com.copypaste.android.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

// ---------------------------------------------------------------------------
// Design System v2 "Quiet Precision" — canonical palette §0
//
// Single source of truth: mirrors crates/copypaste-ui/tailwind.config.js
// exactly. Any drift between this file and tailwind.config.js is a bug.
//
// Canonical ramp (reconciled 2026-05-30 per DESIGN-SYSTEM-v2.md §0):
//   bg        #13141A   panel     #1B1C22   elevated  #23252D
//   raised    #2D2F34   border    #383B42   divider   #2E3035
//   text      #E8EAED   dim       #9DA0A8   faint     #6B6F78
//   accent    #3D8BFF   success   #5FAD65   warning   #D9A343
//   danger    #E05C5C   info/url  #56B6C2   violet    #C678DD
// ---------------------------------------------------------------------------

// ── Surface hierarchy (darkest → most elevated) ───────────────────────────

val IdeBg        = Color(0xFF13141A)   // §0 canonical bg (root window / darkest)
val IdePanel     = Color(0xFF1B1C22)   // §0 canonical panel (primary surface: list bg, nav bar)
val IdeElevated  = Color(0xFF23252D)   // §0 canonical elevated (cards, inputs)
val IdeRaised    = Color(0xFF2D2F34)   // hover / pressed on elevated

// ── Borders & dividers ────────────────────────────────────────────────────

val IdeBorder    = Color(0xFF383B42)   // outline borders (hairline 1dp)
val IdeDivider   = Color(0xFF2E3035)   // row separators (subtler than border)

// ── Interaction states ────────────────────────────────────────────────────

val IdeSelection = Color(0xFF4D8DFF).copy(alpha = 0.16f)  // selected row — accent/16 tint (§3); parity: #4D8DFF matches IdeAccent Liquid-Blue
val IdeHover     = Color(0xFFFFFFFF).copy(alpha = 0.045f) // surface hover (§3)
val IdeMultiSel  = Color(0xFF4D8DFF).copy(alpha = 0.20f)  // multi-select fill (§3); parity: #4D8DFF matches IdeAccent Liquid-Blue

// ── Text hierarchy ────────────────────────────────────────────────────────

val IdeText      = Color(0xFFE8EAED)   // §0 canonical primary text
val IdeDim       = Color(0xFF9DA0A8)   // secondary / subdued text
val IdeFaint     = Color(0xFF82868F)   // PARITY-SPEC §1 tertiaryLabel — WCAG-AA fix (was #6B6F78, failed AA)
val IdeMute      = Color(0xFF6B6F78)   // styleguide --ide-mute (dark): control-track / disabled grey (distinct from faint text)

// ── Ghost text / decorative icon tokens (PARITY-SPEC §1) ───────────────────
// Mirror web's --ide-ghost / --ide-ghost-deco. Ghost = secondary metadata text;
// ghost-deco = 24px+ decorative icons (lower contrast, purely ornamental).
val IdeGhost     = Color.White.copy(alpha = 0.46f)  // dark: white@0.46
val IdeGhostDeco = Color.White.copy(alpha = 0.33f)  // dark: white@0.33 (decorative)

// ── Brand / accent ────────────────────────────────────────────────────────

val IdeAccent     = Color(0xFF4D8DFF)   // §0 canonical accent blue (matches CSS liquid-blue, parity spj2)
// #080C16 achieves ≥4.5:1 WCAG AA on IdeAccent (#4D8DFF, ratio 6.11:1)
// and on all mid-to-light blue accent hues; white was 3.20:1 (AA fail).
val IdeAccentOn   = Color(0xFF080C16)   // text on accent surfaces
val IdeAccentDim  = Color(0xFF4D8DFF).copy(alpha = 0.12f)  // accent container tint
val IdeAccentPress = Color(0xFF2F7AE8)  // primary-button press (dark): a touch deeper than accent

// ── §3 Semantic colours ───────────────────────────────────────────────────

val IdeSuccess     = Color(0xFF5FAD65)  // success / green
val IdeSuccessDim  = Color(0xFF5FAD65).copy(alpha = 0.10f)

val IdeWarning     = Color(0xFFD9A343)  // warning / amber (pinned rows, degraded)
val IdeWarningDim  = Color(0xFFD9A343).copy(alpha = 0.10f)

val IdeDanger      = Color(0xFFE05C5C)  // destructive / error
val IdeDangerDim   = Color(0xFFE05C5C).copy(alpha = 0.10f)

val IdeInfo        = Color(0xFF6E9BF0)  // url / info — liquid-blue identity blue-purple (parity §A CSS liquid-blue dark info)
val IdeInfoDim     = Color(0xFF6E9BF0).copy(alpha = 0.12f)

val IdeViolet      = Color(0xFF9E7BFF)  // image / code — blue-purple (parity §A CSS liquid-blue dark violet)
val IdeVioletDim   = Color(0xFF9E7BFF).copy(alpha = 0.12f)

// ── Error container (for sensitive-item badge) ────────────────────────────

val IdeErrorContainer     = Color(0xFF4A1A1A)
val IdeOnErrorContainer   = IdeDanger

// ── Dark scheme overrides ─────────────────────────────────────────────────

val DarkPrimary            = IdeAccent
val DarkOnPrimary          = IdeAccentOn
val DarkPrimaryContainer   = Color(0xFF1A3D7A)   // deep blue container
val DarkOnPrimaryContainer = Color(0xFFB0CAFF)   // muted blue text on container

val DarkSecondary            = IdeWarning
val DarkOnSecondary          = Color(0xFF1A1200)
val DarkSecondaryContainer   = IdeWarningDim
val DarkOnSecondaryContainer = Color(0xFFFFD98B)

// ── Light scheme colours — Apple macOS Tahoe "Liquid Glass" (PARITY-SPEC §1) ──
// LIGHT is the default theme. Values come verbatim from the canonical Apple
// system palette in docs/PARITY-SPEC.md §1 and mirror
// crates/copypaste-ui/src/index.css :root[data-theme="light"].

// Surface hierarchy — Apple greys
val LightBg        = Color(0xFFE3E3E8)   // window canvas — greyish (systemGray5)
val LightPanel     = Color(0xFFF2F2F5)   // sidebar / list — frosted near-white
val LightElevated  = Color(0xFFFFFFFF)   // cards, inputs
val LightRaised    = Color(0xFFECECF0)   // hover / pressed on elevated

// Borders & dividers
val LightBorder    = Color(0xFFD3D3D8)   // hairline separators
val LightDivider   = Color(0xFFE2E2E6)   // row separators

// Text hierarchy (Apple label colors)
val LightText      = Color(0xFF1D1D1F)   // labelColor
val LightDim       = Color(0xFF5B5B60)   // secondaryLabel
val LightFaint     = Color(0xFF6C6C72)   // styleguide --ide-faint 108/108/114 — AA-safe 4.6:1 on white (was #8A8A8E ~2.9:1, failed AA)
val LightMute      = Color(0xFF8E8E94)   // styleguide --ide-mute 142/142/148 — control-track / disabled grey (distinct from faint text)

// Ghost text / decorative icons — light variant (PARITY-SPEC §1)
val LightGhost     = Color(0xFF3C3C43).copy(alpha = 0.55f)  // rgba(60,60,67,0.55)
val LightGhostDeco = Color(0xFF3C3C43).copy(alpha = 0.32f)  // rgba(60,60,67,0.32)

// Brand — styleguide accent (systemBlue family)
val LightPrimary            = Color(0xFF007AFF)   // §1 accent (systemBlue)
val LightOnPrimary          = Color(0xFFFFFFFF)
val LightPrimaryContainer   = Color(0xFF007AFF).copy(alpha = 0.12f)
val LightOnPrimaryContainer = Color(0xFF0063D1)   // accent-hover, on tint
val LightAccentPress        = Color(0xFF0070EB)   // styleguide --ide-accent-press 0/112/235 — primary-button press

// Semantic — styleguide :root[data-theme=light] ramp (AA-darkened on white)
val LightSecondary            = Color(0xFFFF9500)  // warning / systemOrange (Material secondary slot)
val LightOnSecondary          = Color(0xFFFFFFFF)
val LightSecondaryContainer   = Color(0xFFFF9500).copy(alpha = 0.12f)
val LightOnSecondaryContainer = Color(0xFF3E2000)

// Pinned-row / COLOR / NUMBER / PATH badge amber — styleguide --ide-badge-warning #D9A343.
val LightWarning   = Color(0xFFD9A343)   // styleguide pinned/amber 217/163/67 (was systemOrange #FF9500)

val LightDanger    = Color(0xFFD7281E)   // styleguide --ide-danger 215/40/30 (AA-darkened, was #FF3B30)
val LightDangerDim = Color(0xFFD7281E).copy(alpha = 0.10f)

val LightSuccess   = Color(0xFF288C46)   // styleguide --ide-success 40/140/70 (AA-darkened, was #34C759)
val LightInfo      = Color(0xFF1478AA)   // styleguide --ide-sky 20/120/170 (URL/IMAGE, was systemTeal #32ADE6)
val LightViolet    = Color(0xFF805AD5)   // styleguide --ide-violet 128/90/213 (CODE, was systemPurple #AF52DE)

// Error containers for light
val LightErrorContainer    = Color(0xFFD7281E).copy(alpha = 0.10f)
val LightOnErrorContainer  = Color(0xFFD7281E)

// ---------------------------------------------------------------------------
// IdeColors — the full theme-adaptive token set (PARITY-SPEC §1), the Android
// mirror of the web --ide-* CSS custom properties. Screens historically used
// the top-level dark `Ide*` constants directly (hardcoded dark); to make them
// light-first they read `LocalIdeColors.current.<token>` instead, which carries
// the ACTIVE ramp (light or dark) provided by CopyPasteTheme.
//
// Material's colorScheme only covers primary/surface/error slots; this holder
// adds the semantic (success/info/violet/warning) + ghost + interaction tokens
// the desktop app uses, so every screen themes identically to web.
// ---------------------------------------------------------------------------
@Immutable
data class IdeColors(
    val bg: Color, val panel: Color, val elevated: Color, val raised: Color,
    val border: Color, val divider: Color,
    val text: Color, val dim: Color, val faint: Color, val mute: Color,
    val ghost: Color, val ghostDeco: Color,
    val accent: Color, val accentOn: Color, val accentDim: Color, val accentPress: Color,
    val selection: Color, val hover: Color,
    val success: Color, val successDim: Color,
    val warning: Color, val warningDim: Color,
    val danger: Color, val dangerDim: Color,
    val info: Color, val infoDim: Color,
    val violet: Color, val violetDim: Color,
)

/** Dark ramp — the canonical Design System v2 dark values. */
val DarkIdeColors = IdeColors(
    bg = IdeBg, panel = IdePanel, elevated = IdeElevated, raised = IdeRaised,
    border = IdeBorder, divider = IdeDivider,
    text = IdeText, dim = IdeDim, faint = IdeFaint, mute = IdeMute,
    ghost = IdeGhost, ghostDeco = IdeGhostDeco,
    accent = IdeAccent, accentOn = IdeAccentOn, accentDim = IdeAccentDim, accentPress = IdeAccentPress,
    selection = IdeSelection, hover = IdeHover,
    success = IdeSuccess, successDim = IdeSuccessDim,
    warning = IdeWarning, warningDim = IdeWarningDim,
    danger = IdeDanger, dangerDim = IdeDangerDim,
    info = IdeInfo, infoDim = IdeInfoDim,
    violet = IdeViolet, violetDim = IdeVioletDim,
)

/** Light ramp — Apple macOS Tahoe "Liquid Glass" (PARITY-SPEC §1). */
val LightIdeColors = IdeColors(
    bg = LightBg, panel = LightPanel, elevated = LightElevated, raised = LightRaised,
    border = LightBorder, divider = LightDivider,
    text = LightText, dim = LightDim, faint = LightFaint, mute = LightMute,
    ghost = LightGhost, ghostDeco = LightGhostDeco,
    accent = LightPrimary, accentOn = LightOnPrimary,
    accentDim = LightPrimary.copy(alpha = 0.12f), accentPress = LightAccentPress,
    selection = LightPrimary.copy(alpha = 0.14f),
    hover = Color.Black.copy(alpha = 0.04f),
    success = LightSuccess, successDim = LightSuccess.copy(alpha = 0.14f),
    // styleguide pins pinned-row/COLOR/NUMBER/PATH badge amber to #D9A343, not systemOrange.
    warning = LightWarning, warningDim = LightWarning.copy(alpha = 0.14f),
    danger = LightDanger, dangerDim = LightDanger.copy(alpha = 0.12f),
    info = LightInfo, infoDim = LightInfo.copy(alpha = 0.14f),
    violet = LightViolet, violetDim = LightViolet.copy(alpha = 0.14f),
)
