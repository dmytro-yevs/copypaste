package com.copypaste.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.dp

// ---------------------------------------------------------------------------
// Corner-radius scale — mirrors the macOS desktop UI, which uses a tight 6 px
// IDE radius for inline controls and rounded ~12 dp cards for elevated panels.
//
//   extraSmall  4 dp  — chips, badges, inline action pills (matches History
//                       ActionChip 4 dp)
//   small       6 dp  — buttons, text fields, list affordances (macOS ide 6 px)
//   medium     12 dp  — cards / grouped sections (the primary "rounded card")
//   large      16 dp  — hero surfaces (QR card, onboarding hero)
//   extraLarge 24 dp  — full-bleed sheets
//
// These feed MaterialTheme so every default Material3 component (Card, Button,
// TextField) picks up the cohesive radii without per-call-site overrides.
// ---------------------------------------------------------------------------

val CopyPasteShapes = Shapes(
    extraSmall = RoundedCornerShape(4.dp),
    small      = RoundedCornerShape(6.dp),
    medium     = RoundedCornerShape(12.dp),
    large      = RoundedCornerShape(16.dp),
    extraLarge = RoundedCornerShape(24.dp),
)
