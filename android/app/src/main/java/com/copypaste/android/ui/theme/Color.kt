package com.copypaste.android.ui.theme

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

val IdeSelection = Color(0xFF3D8BFF).copy(alpha = 0.16f)  // selected row — accent/16 tint (§3)
val IdeHover     = Color(0xFFFFFFFF).copy(alpha = 0.045f) // surface hover (§3)
val IdeMultiSel  = Color(0xFF3D8BFF).copy(alpha = 0.20f)  // multi-select fill (§3)

// ── Text hierarchy ────────────────────────────────────────────────────────

val IdeText      = Color(0xFFE8EAED)   // §0 canonical primary text
val IdeDim       = Color(0xFF9DA0A8)   // secondary / subdued text
val IdeFaint     = Color(0xFF82868F)   // PARITY-SPEC §1 tertiaryLabel — WCAG-AA fix (was #6B6F78, failed AA)

// ── Ghost text / decorative icon tokens (PARITY-SPEC §1) ───────────────────
// Mirror web's --ide-ghost / --ide-ghost-deco. Ghost = secondary metadata text;
// ghost-deco = 24px+ decorative icons (lower contrast, purely ornamental).
val IdeGhost     = Color.White.copy(alpha = 0.46f)  // dark: white@0.46
val IdeGhostDeco = Color.White.copy(alpha = 0.33f)  // dark: white@0.33 (decorative)

// ── Brand / accent ────────────────────────────────────────────────────────

val IdeAccent    = Color(0xFF3D8BFF)   // §0 canonical accent blue
val IdeAccentOn  = Color(0xFFFFFFFF)   // text on accent surfaces
val IdeAccentDim = Color(0xFF3D8BFF).copy(alpha = 0.12f)  // accent container tint

// ── §3 Semantic colours ───────────────────────────────────────────────────

val IdeSuccess     = Color(0xFF5FAD65)  // success / green
val IdeSuccessDim  = Color(0xFF5FAD65).copy(alpha = 0.10f)

val IdeWarning     = Color(0xFFD9A343)  // warning / amber (pinned rows, degraded)
val IdeWarningDim  = Color(0xFFD9A343).copy(alpha = 0.10f)

val IdeDanger      = Color(0xFFE05C5C)  // destructive / error
val IdeDangerDim   = Color(0xFFE05C5C).copy(alpha = 0.10f)

val IdeInfo        = Color(0xFF56B6C2)  // url / info (teal)
val IdeInfoDim     = Color(0xFF56B6C2).copy(alpha = 0.12f)

val IdeViolet      = Color(0xFFC678DD)  // image / code (purple)
val IdeVioletDim   = Color(0xFFC678DD).copy(alpha = 0.12f)

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
val LightFaint     = Color(0xFF8A8A8E)   // tertiaryLabel (§1 WCAG-AA value)

// Ghost text / decorative icons — light variant (PARITY-SPEC §1)
val LightGhost     = Color(0xFF3C3C43).copy(alpha = 0.55f)  // rgba(60,60,67,0.55)
val LightGhostDeco = Color(0xFF3C3C43).copy(alpha = 0.32f)  // rgba(60,60,67,0.32)

// Brand — Apple systemBlue
val LightPrimary            = Color(0xFF007AFF)   // §1 accent (systemBlue)
val LightOnPrimary          = Color(0xFFFFFFFF)
val LightPrimaryContainer   = Color(0xFF007AFF).copy(alpha = 0.12f)
val LightOnPrimaryContainer = Color(0xFF0063D1)   // accent-hover, on tint

// Semantic — Apple system colors (§1)
val LightSecondary            = Color(0xFFFF9500)  // warning / systemOrange
val LightOnSecondary          = Color(0xFFFFFFFF)
val LightSecondaryContainer   = Color(0xFFFF9500).copy(alpha = 0.12f)
val LightOnSecondaryContainer = Color(0xFF3E2000)

val LightDanger    = Color(0xFFFF3B30)   // systemRed
val LightDangerDim = Color(0xFFFF3B30).copy(alpha = 0.10f)

val LightSuccess   = Color(0xFF34C759)   // systemGreen
val LightInfo      = Color(0xFF32ADE6)   // systemTeal/cyan
val LightViolet    = Color(0xFFAF52DE)   // systemPurple

// Error containers for light
val LightErrorContainer    = Color(0xFFFF3B30).copy(alpha = 0.10f)
val LightOnErrorContainer  = Color(0xFFFF3B30)
