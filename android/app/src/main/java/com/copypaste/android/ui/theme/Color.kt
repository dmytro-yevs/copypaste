package com.copypaste.android.ui.theme

import androidx.compose.ui.graphics.Color

// ---------------------------------------------------------------------------
// JetBrains "New UI" / Darcula-inspired palette — mirrors the macOS desktop
// UI tokens defined in crates/copypaste-ui/tailwind.config.js.
//
// Desktop reference tokens (tailwind ide.*):
//   bg         #1e1f22   panel      #2b2d30   elevated   #313438
//   border     #393b40   divider    #43454a   selection  #2e436e
//   hover      #34373b   text       #dfe1e5   dim        #9da0a8
//   faint      #6f737a   accent     #3574f0   danger     #db5c5c
//   success    #5fad65   warning    #d9a343
// ---------------------------------------------------------------------------

// ── Surface hierarchy (light dark for readability; Android uses "dark" layer names) ─

val IdeBg        = Color(0xFF1E1F22)   // outermost window / root background
val IdePanel     = Color(0xFF2B2D30)   // primary surface (list background)
val IdeElevated  = Color(0xFF313438)   // elevated cards, chips, inputs
val IdeBorder    = Color(0xFF393B40)   // separator / outline
val IdeDivider   = Color(0xFF43454A)   // lighter divider between rows

// ── Interaction states ───────────────────────────────────────────────────────

val IdeSelection = Color(0xFF2E436E)   // selected row background (blue tint)
val IdeHover     = Color(0xFF34373B)   // pressed / ripple surface

// ── Text ─────────────────────────────────────────────────────────────────────

val IdeText      = Color(0xFFDFE1E5)   // primary text
val IdeDim       = Color(0xFF9DA0A8)   // secondary / subdued text
val IdeFaint     = Color(0xFF6F737A)   // placeholder / timestamp

// ── Brand / semantic ─────────────────────────────────────────────────────────

val IdeAccent    = Color(0xFF3574F0)   // primary action blue (matches macOS accent)
val IdeAccentOn  = Color(0xFFFFFFFF)   // text on accent surfaces
val IdeDanger    = Color(0xFFDB5C5C)   // destructive / error
val IdeSuccess   = Color(0xFF5FAD65)   // success / green
val IdeWarning   = Color(0xFFD9A343)   // warning / amber (pinned rows)

// ── Error container (for sensitive-item badge) ───────────────────────────────

val IdeErrorContainer     = Color(0xFF4A1A1A)
val IdeOnErrorContainer   = Color(0xFFDB5C5C)

// ── Dark scheme overrides (used on all Android builds; the app is always dark) ─

val DarkPrimary            = IdeAccent
val DarkOnPrimary          = IdeAccentOn
val DarkPrimaryContainer   = Color(0xFF1A3A7A)   // deep blue container
val DarkOnPrimaryContainer = Color(0xFFAEC6F5)   // muted blue text

val DarkSecondary            = IdeWarning
val DarkOnSecondary          = Color(0xFF1A1200)
val DarkSecondaryContainer   = Color(0xFF3A2800)
val DarkOnSecondaryContainer = Color(0xFFFFD98B)
