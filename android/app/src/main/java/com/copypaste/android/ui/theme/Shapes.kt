package com.copypaste.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.dp

// ---------------------------------------------------------------------------
// Corner-radius scale — mirrors the Liquid Glass styleguide CSS radius tokens
// (--radius-chip 7, --radius-ctl 9, --radius-card 12).
//
//   extraSmall  7 dp  — chips, badges, transport pills (styleguide --radius-chip)
//   small       9 dp  — buttons, text fields, controls (styleguide --radius-ctl)
//   medium     12 dp  — cards / grouped sections (styleguide --radius-card,
//                       parity with macOS 12 px — PARITY-SPEC §4 / PG-57)
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

/**
 * Styleguide --radius-card — cards, grouped sections (the primary rounded card).
 * 12 dp matches the macOS 12 px token (PARITY-SPEC §4, PG-57). Was 14 dp.
 */
val RadiusCard    = RoundedCornerShape(12.dp)

val CopyPasteShapes = Shapes(
    extraSmall = RadiusChip,     // 7 dp — chip
    small      = RadiusControl,  // 9 dp — control
    medium     = RadiusCard,     // 12 dp — card (parity with macOS 12 px, PG-57)
    large      = RoundedCornerShape(16.dp),
    extraLarge = RoundedCornerShape(24.dp),
)
