package com.copypaste.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.dp

// ---------------------------------------------------------------------------
// Corner-radius scale — fixed (STYLEGUIDE §5). No skin-driven radius.
//
//   --r-chip   7px  — chips, badges, status pills, tiles
//   --r-ctl    8px  — buttons
//   --r-input  9px  — inputs / search
//   --r-card  13px  — cards, banners, modals
//   --r-pill  999px — transport/filter pills, toggles
//
// These feed MaterialTheme so every default Material3 component (Card, Button,
// TextField) picks up the cohesive radii without per-call-site overrides.
// ---------------------------------------------------------------------------

/** §5 --r-chip — chips, badges, status pills, tiles. */
val RadiusChip    = RoundedCornerShape(7.dp)

/** §5 --r-ctl — buttons, inline controls. */
val RadiusControl = RoundedCornerShape(8.dp)

/** §5 --r-input — inputs / search fields. */
val RadiusInput   = RoundedCornerShape(9.dp)

/** §5 --r-card — cards, banners, modals (the primary rounded card). */
val RadiusCard    = RoundedCornerShape(13.dp)

/** §5 --r-pill — transport/filter pills, toggles. */
val RadiusPill    = RoundedCornerShape(percent = 50)

val CopyPasteShapes = Shapes(
    extraSmall = RadiusChip,     // 7 dp — chip
    small      = RadiusControl,  // 8 dp — control
    medium     = RadiusCard,     // 13 dp — card
    large      = RadiusCard,     // 13 dp — banners / modals
    extraLarge = RoundedCornerShape(22.dp),
)
