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

/** Lucide "Copy" glyph — 24x24 viewBox, stroke-width 2, round caps/joins. */
val Copy: ImageVector by lazy {
    ImageVector.Builder(
        name = "Lucide.Copy",
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
                moveTo(10f, 8f)
                horizontalLineTo(20f)
                arcTo(2f, 2f, 0f, false, true, 22f, 10f)
                verticalLineTo(20f)
                arcTo(2f, 2f, 0f, false, true, 20f, 22f)
                horizontalLineTo(10f)
                arcTo(2f, 2f, 0f, false, true, 8f, 20f)
                verticalLineTo(10f)
                arcTo(2f, 2f, 0f, false, true, 10f, 8f)
                close()
            }
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(4f, 16f)
                curveToRelative(-1.1f, 0f, -2f, -0.9f, -2f, -2f)
                verticalLineTo(4f)
                curveToRelative(0f, -1.1f, 0.9f, -2f, 2f, -2f)
                horizontalLineToRelative(10f)
                curveToRelative(1.1f, 0f, 2f, 0.9f, 2f, 2f)
            }
    }.build()
}
