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
val IdeFaint     = Color(0xFF6B6F78)   // placeholder / timestamp / hero-icon

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

// ── Light scheme colours ──────────────────────────────────────────────────
// Mirrors :root[data-theme="light"] in crates/copypaste-ui/src/index.css.
// All WCAG AA contrast ratios verified against their backgrounds.

// Surface hierarchy — light ramp
val LightBg        = Color(0xFFECEEF2)   // root / lightest layer
val LightPanel     = Color(0xFFF5F6F8)   // primary surface
val LightElevated  = Color(0xFFEEF0F4)   // cards, inputs
val LightRaised    = Color(0xFFE4E6EB)   // hover / pressed

// Borders & dividers
val LightBorder    = Color(0xFFC8CAD0)
val LightDivider   = Color(0xFFD8DAE0)

// Text hierarchy — all WCAG AA on LightPanel (#F5F6F8)
val LightText      = Color(0xFF1A1C20)   // 13.8:1 — AAA
val LightDim       = Color(0xFF4B505A)   //  6.2:1 — AA
val LightFaint     = Color(0xFF6B7280)   //  4.6:1 — AA

// Brand — darkened for light surfaces; 5.2:1 on LightElevated
val LightPrimary            = Color(0xFF1A5FCC)
val LightOnPrimary          = Color(0xFFFFFFFF)
val LightPrimaryContainer   = Color(0xFFD6E4FF)   // light blue tint container
val LightOnPrimaryContainer = Color(0xFF002060)   // dark navy on container

// Semantic
val LightSecondary            = Color(0xFFA0610A)  // warning amber — AA on light
val LightOnSecondary          = Color(0xFFFFFFFF)
val LightSecondaryContainer   = Color(0xFFFFE0B2)
val LightOnSecondaryContainer = Color(0xFF3E2000)

val LightDanger    = Color(0xFFC0392B)   // destructive / error
val LightDangerDim = Color(0xFFC0392B).copy(alpha = 0.09f)

// Error containers for light
val LightErrorContainer    = Color(0xFFFFDAD6)
val LightOnErrorContainer  = Color(0xFF8B1A1A)
