// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by scripts/generate-lucide-icons.sh from lucide-icons/lucide
// (ISC license) at the pinned SHA recorded in that script's header.
// Regenerate with: ./scripts/generate-lucide-icons.sh
package com.copypaste.android.ui.theme.icons

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

// Lucide glyphs render with fill=none / stroke=currentColor; the tint below is
// a build-time placeholder — every call site supplies the real tint via
// Icon(tint = ...), matching the fill=none/stroke=currentColor SVG contract.
private val NoFill = SolidColor(Color.Transparent)
private val PlaceholderStroke = SolidColor(Color.Black)

/** Lucide "CheckCircle" glyph — 24x24 viewBox, stroke-width 2, round caps/joins. */
val CheckCircle: ImageVector by lazy {
    ImageVector.Builder(
        name = "Lucide.CheckCircle",
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply {
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(22f, 11.08f)
                verticalLineTo(12f)
                arcToRelative(10f, 10f, 0f, true, true, -5.93f, -9.14f)
            }
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(22f, 4f)
                lineTo(12f, 14.01f)
                lineTo(9f, 11.01f)
            }
    }.build()
}
