package com.copypaste.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Shapes
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * STYLEGUIDE §5 fixed corner radii — constant across both themes, no per-skin
 * variation (android-design-system "CpShapes fixed corner radii" requirement).
 */
object CpShapes {
    val chip: Dp = 7.dp
    val ctl: Dp = 8.dp
    val input: Dp = 9.dp
    val card: Dp = 13.dp
    val pill: Dp = 999.dp
}

/** M3 [Shapes] built from [CpShapes] — fed into `MaterialTheme(shapes = ...)`. */
val CopyPasteShapes = Shapes(
    extraSmall = RoundedCornerShape(CpShapes.chip),
    small = RoundedCornerShape(CpShapes.ctl),
    medium = RoundedCornerShape(CpShapes.input),
    large = RoundedCornerShape(CpShapes.card),
    extraLarge = RoundedCornerShape(CpShapes.pill),
)
