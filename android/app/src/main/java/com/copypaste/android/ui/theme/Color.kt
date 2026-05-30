package com.copypaste.android.ui.theme

import androidx.compose.ui.graphics.Color

// ---------------------------------------------------------------------------
// JetBrains "New UI" v0.5.3 palette — mirrors the macOS desktop UI tokens
// defined in crates/copypaste-ui/tailwind.config.js.
//
// v0.5.3 changes vs v0.5.2:
//   • bg/panel shifted darker (more depth, better contrast)
//   • accent retunned to #3592ff (slightly more electric blue)
//   • danger/success/warning brightened slightly for legibility
//   • new warningDim + accentDim surface tints
//   • raised surface tier added between elevated and hover
//
// Desktop reference tokens (tailwind ide.*):
//   bg         #16171a   panel      #1e2024   elevated   #26282d
//   raised     #2d2f34   border     #383b42   divider    #2e3035
//   selection  #1e3d72   hover      #22252a   text       #dfe1e5
//   dim        #9da0a8   faint      #6b6f78   accent     #3592ff
//   accentHov  #5aacff   accentDim  #1a3661   danger     #f07171
//   success    #63c174   warning    #e5a93a   warningDim #3a2900
// ---------------------------------------------------------------------------

// ── Surface hierarchy (darkest → most elevated) ───────────────────────────

val IdeBg        = Color(0xFF16171A)   // root background (outermost)
val IdePanel     = Color(0xFF1E2024)   // primary surface (list bg, nav bar)
val IdeElevated  = Color(0xFF26282D)   // cards, inputs, popovers
val IdeRaised    = Color(0xFF2D2F34)   // hover / pressed on elevated

// ── Borders & dividers ────────────────────────────────────────────────────

val IdeBorder    = Color(0xFF383B42)   // outline borders (hairline 1dp)
val IdeDivider   = Color(0xFF2E3035)   // row separators (subtler than border)

// ── Interaction states ────────────────────────────────────────────────────

val IdeSelection = Color(0xFF1E3D72)   // selected row — deep blue tint
val IdeHover     = Color(0xFF22252A)   // pressed / ripple surface

// ── Text hierarchy ────────────────────────────────────────────────────────

val IdeText      = Color(0xFFDFE1E5)   // primary text
val IdeDim       = Color(0xFF9DA0A8)   // secondary / subdued text
val IdeFaint     = Color(0xFF6B6F78)   // placeholder / timestamp

// ── Brand / semantic ──────────────────────────────────────────────────────

val IdeAccent    = Color(0xFF3592FF)   // primary action blue (v0.5.3 retune)
val IdeAccentOn  = Color(0xFFFFFFFF)   // text on accent surfaces
val IdeAccentDim = Color(0xFF1A3661)   // accent surface tint (badges, chips)
val IdeDanger    = Color(0xFFF07171)   // destructive / error (brighter)
val IdeSuccess   = Color(0xFF63C174)   // success / green (brighter)
val IdeWarning   = Color(0xFFE5A93A)   // warning / amber
val IdeWarningDim= Color(0xFF3A2900)   // warning surface tint (pinned rows)

// ── Error container (for sensitive-item badge) ────────────────────────────

val IdeErrorContainer     = Color(0xFF4A1A1A)
val IdeOnErrorContainer   = Color(0xFFF07171)   // matches IdeDanger

// ── Dark scheme overrides (always-dark app) ───────────────────────────────

val DarkPrimary            = IdeAccent
val DarkOnPrimary          = IdeAccentOn
val DarkPrimaryContainer   = Color(0xFF1A3D7A)   // deep blue container
val DarkOnPrimaryContainer = Color(0xFFB0CAFF)   // muted blue text on container

val DarkSecondary            = IdeWarning
val DarkOnSecondary          = Color(0xFF1A1200)
val DarkSecondaryContainer   = IdeWarningDim
val DarkOnSecondaryContainer = Color(0xFFFFD98B)
