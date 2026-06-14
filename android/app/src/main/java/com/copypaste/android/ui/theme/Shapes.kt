package com.copypaste.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.dp

// ---------------------------------------------------------------------------
// Corner-radius scale — mirrors the Liquid Glass styleguide CSS radius tokens
// (--radius-chip 7, --radius-ctl 9, --radius-card 14).
//
//   extraSmall  7 dp  — chips, badges, transport pills (styleguide --radius-chip)
//   small       9 dp  — buttons, text fields, controls (styleguide --radius-ctl)
//   medium     14 dp  — cards / grouped sections (styleguide --radius-card)
//   large      16 dp  — hero surfaces / modals (QR card, dialogs, onboarding hero)
//   extraLarge 24 dp  — full-bleed sheets
//
// These feed MaterialTheme so every default Material3 component (Card, Button,
// TextField) picks up the cohesive radii without per-call-site overrides.
// Named convenience aliases mirror the CSS custom-property names so call sites
// read intent (RadiusChip / RadiusControl / RadiusCard).
// ---------------------------------------------------------------------------

/** Styleguide --radius-chip — chips, badges, transport pills. */
val RadiusChip    = RoundedCornerShape(7.dp)

/** Styleguide --radius-ctl — buttons, text fields, inline controls. */
val RadiusControl = RoundedCornerShape(9.dp)

/** Styleguide --radius-card — cards, grouped sections (the primary rounded card). */
val RadiusCard    = RoundedCornerShape(14.dp)

val CopyPasteShapes = Shapes(
    extraSmall = RadiusChip,     // 7 dp — chip
    small      = RadiusControl,  // 9 dp — control
    medium     = RadiusCard,     // 14 dp — card
    large      = RoundedCornerShape(16.dp),
    extraLarge = RoundedCornerShape(24.dp),
)
