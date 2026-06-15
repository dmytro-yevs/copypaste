package com.copypaste.android.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

// ---------------------------------------------------------------------------
// NavIcons — SF-like thin-stroke nav ImageVectors (CopyPaste-dm51 parity)
//
// 24×24 grid, fill=none, stroke=currentColor, strokeWidth=1.85,
// strokeLineCap/Join=Round. Paths mirror the web NavIcons.tsx (SF Symbols):
//   History  → clock.arrow.circlepath   (clock face + CCW refresh arc)
//   Devices  → laptopcomputer.and.iphone (laptop + phone)
//   Settings → gear / gearshape         (8-tooth gear)
//
// Usage: pass NavIcons.History / NavIcons.Devices / NavIcons.Settings as the
// imageVector in Icon(), or reference from MainActivity's NavTab enum.
//
// Tint is always driven by the call site (Icon tint param) — these ImageVectors
// use a placeholder Black stroke that is replaced by the runtime tint.
// All are aria-hidden equivalents — the parent button carries the a11y label.
// ---------------------------------------------------------------------------

private val NoFill = SolidColor(Color.Transparent)
private val PlaceholderStroke = SolidColor(Color.Black)
private const val SW = 1.85f

object NavIcons {

    // -------------------------------------------------------------------------
    // History — clock face + counter-clockwise refresh arc
    // Mirrors NavIcons.tsx HistoryIcon: outer circle, hour-hand polyline,
    // CCW arc M5.64 5.64 A 8.48 8.48 0 0 0 3.5 12, arrowhead 1.5,10→3.5,12→5.5,10
    // -------------------------------------------------------------------------
    val History: ImageVector by lazy {
        ImageVector.Builder(
            name = "NavIcons.History",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            // Outer clock circle (cx=12, cy=12, r=8.5)
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = SW,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                // Circle via two arcs
                moveTo(12f, 3.5f)
                arcTo(8.5f, 8.5f, 0f, false, true, 12f, 20.5f)
                arcTo(8.5f, 8.5f, 0f, false, true, 12f, 3.5f)
                close()
            }
            // Hour hand: 12,7.5 → 12,12 → 15,14.5
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = SW,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(12f, 7.5f)
                lineTo(12f, 12f)
                lineTo(15f, 14.5f)
            }
            // CCW arc: M5.64 5.64 A 8.48 8.48 0 0 0 3.5 12
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = SW,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(5.64f, 5.64f)
                arcTo(8.48f, 8.48f, 0f, false, false, 3.5f, 12f)
            }
            // Arrowhead: 1.5,10 → 3.5,12 → 5.5,10
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = SW,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(1.5f, 10f)
                lineTo(3.5f, 12f)
                lineTo(5.5f, 10f)
            }
        }.build()
    }

    // -------------------------------------------------------------------------
    // Devices — laptop body + base ledge + phone body + home indicator dot
    // Mirrors NavIcons.tsx DevicesIcon.
    // -------------------------------------------------------------------------
    val Devices: ImageVector by lazy {
        ImageVector.Builder(
            name = "NavIcons.Devices",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            // Laptop body: rect x=2 y=4 w=15 h=11 rx=1.5
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = SW,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(3.5f, 4f)
                arcTo(1.5f, 1.5f, 0f, false, false, 2f, 5.5f)
                lineTo(2f, 13.5f)
                arcTo(1.5f, 1.5f, 0f, false, false, 3.5f, 15f)
                lineTo(15.5f, 15f)
                arcTo(1.5f, 1.5f, 0f, false, false, 17f, 13.5f)
                lineTo(17f, 5.5f)
                arcTo(1.5f, 1.5f, 0f, false, false, 15.5f, 4f)
                close()
            }
            // Laptop base ledge: M1 15 h17 l.5 1.5 H.5 Z
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = SW,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(1f, 15f)
                lineTo(18f, 15f)
                lineTo(18.5f, 16.5f)
                lineTo(0.5f, 16.5f)
                close()
            }
            // Phone body: rect x=18.5 y=8 w=4.5 h=8 rx=1
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = SW,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(19.5f, 8f)
                arcTo(1f, 1f, 0f, false, false, 18.5f, 9f)
                lineTo(18.5f, 15f)
                arcTo(1f, 1f, 0f, false, false, 19.5f, 16f)
                lineTo(22f, 16f)
                arcTo(1f, 1f, 0f, false, false, 23f, 15f)
                lineTo(23f, 9f)
                arcTo(1f, 1f, 0f, false, false, 22f, 8f)
                close()
            }
            // Phone home indicator: line x1=20.75 y1=14.75 x2=20.75 y2=14.76
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = 2f, // slightly thicker per NavIcons.tsx
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(20.75f, 14.75f)
                lineTo(20.75f, 14.76f)
            }
        }.build()
    }

    // -------------------------------------------------------------------------
    // Settings — inner circle + 8-tooth gear outline
    // Mirrors NavIcons.tsx SettingsIcon (inner circle r=3, gear path).
    // -------------------------------------------------------------------------
    val Settings: ImageVector by lazy {
        ImageVector.Builder(
            name = "NavIcons.Settings",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            // Inner circle: cx=12 cy=12 r=3
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = SW,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(12f, 9f)
                arcTo(3f, 3f, 0f, false, true, 12f, 15f)
                arcTo(3f, 3f, 0f, false, true, 12f, 9f)
                close()
            }
            // Gear outer teeth — standard 8-tooth gear outline from NavIcons.tsx.
            // Path traced from the SVG path data (see web NavIcons.tsx SettingsIcon).
            path(
                fill = NoFill,
                stroke = PlaceholderStroke,
                strokeLineWidth = SW,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
                pathFillType = PathFillType.NonZero,
            ) {
                moveTo(19.14f, 12.94f)
                arcTo(7.25f, 7.25f, 0f, false, false, 19.2f, 12f)
                arcTo(7.25f, 7.25f, 0f, false, false, 19.13f, 11.06f)
                lineTo(21.16f, 9.48f)
                arcTo(0.49f, 0.49f, 0f, false, false, 21.28f, 8.86f)
                lineTo(19.36f, 5.54f)
                arcTo(0.49f, 0.49f, 0f, false, false, 18.77f, 5.33f)
                lineTo(16.37f, 6.29f)
                arcTo(7.11f, 7.11f, 0f, false, false, 14.76f, 5.35f)
                lineTo(14.4f, 2.81f)
                arcTo(0.48f, 0.48f, 0f, false, false, 13.92f, 2.4f)
                lineTo(10.08f, 2.4f)
                arcTo(0.48f, 0.48f, 0f, false, false, 9.6f, 2.81f)
                lineTo(9.24f, 5.35f)
                arcTo(7.11f, 7.11f, 0f, false, false, 7.63f, 6.29f)
                lineTo(5.23f, 5.33f)
                arcTo(0.49f, 0.49f, 0f, false, false, 4.64f, 5.54f)
                lineTo(2.72f, 8.86f)
                arcTo(0.48f, 0.48f, 0f, false, false, 2.84f, 9.48f)
                lineTo(4.87f, 11.06f)
                arcTo(7.34f, 7.34f, 0f, false, false, 4.8f, 12f)
                lineTo(2.77f, 13.58f)
                arcTo(0.48f, 0.48f, 0f, false, false, 2.65f, 14.2f)
                lineTo(4.57f, 17.52f)
                arcTo(0.49f, 0.49f, 0f, false, false, 5.17f, 17.73f)
                lineTo(7.57f, 16.77f)
                arcTo(7.11f, 7.11f, 0f, false, false, 9.18f, 17.71f)
                lineTo(9.54f, 20.25f)
                arcTo(0.48f, 0.48f, 0f, false, false, 10.02f, 20.6f)
                lineTo(13.98f, 20.6f)
                arcTo(0.48f, 0.48f, 0f, false, false, 14.46f, 20.25f)
                lineTo(14.82f, 17.71f)
                arcTo(7.11f, 7.11f, 0f, false, false, 16.43f, 16.77f)
                lineTo(18.83f, 17.73f)
                arcTo(0.49f, 0.49f, 0f, false, false, 19.43f, 17.52f)
                lineTo(21.35f, 14.2f)
                arcTo(0.48f, 0.48f, 0f, false, false, 21.23f, 13.58f)
                close()
            }
        }.build()
    }
}
