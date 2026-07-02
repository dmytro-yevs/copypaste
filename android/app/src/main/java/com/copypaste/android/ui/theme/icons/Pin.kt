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

/** Lucide "Pin" glyph — 24x24 viewBox, stroke-width 2, round caps/joins. */
val Pin: ImageVector by lazy {
    ImageVector.Builder(
        name = "Lucide.Pin",
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
                moveTo(12f, 17f)
                lineTo(12f, 22f)
            }
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(5f, 17f)
                horizontalLineToRelative(14f)
                verticalLineToRelative(-1.76f)
                arcToRelative(2f, 2f, 0f, false, false, -1.11f, -1.79f)
                lineToRelative(-1.78f, -0.9f)
                arcTo(2f, 2f, 0f, false, true, 15f, 10.76f)
                verticalLineTo(6f)
                horizontalLineToRelative(1f)
                arcToRelative(2f, 2f, 0f, false, false, 0f, -4f)
                horizontalLineTo(8f)
                arcToRelative(2f, 2f, 0f, false, false, 0f, 4f)
                horizontalLineToRelative(1f)
                verticalLineToRelative(4.76f)
                arcToRelative(2f, 2f, 0f, false, true, -1.11f, 1.79f)
                lineToRelative(-1.78f, 0.9f)
                arcTo(2f, 2f, 0f, false, false, 5f, 15.24f)
                close()
            }
    }.build()
}
