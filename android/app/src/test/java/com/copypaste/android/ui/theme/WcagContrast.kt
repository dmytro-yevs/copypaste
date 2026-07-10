package com.copypaste.android.ui.theme

import androidx.compose.ui.graphics.Color
import kotlin.math.pow

/**
 * WCAG relative-luminance contrast helper for token/AA tests (android-design-system
 * task 1.5). Matches spec.md's worked onError example exactly: linearize sRGB,
 * `L = 0.2126R + 0.7152G + 0.0722B`, `ratio = (L1 + 0.05) / (L2 + 0.05)`.
 */
internal object WcagContrast {
    private fun linearize(channel: Float): Double {
        val c = channel.toDouble()
        return if (c <= 0.03928) c / 12.92 else ((c + 0.055) / 1.055).pow(2.4)
    }

    private fun relativeLuminance(color: Color): Double =
        0.2126 * linearize(color.red) + 0.7152 * linearize(color.green) + 0.0722 * linearize(color.blue)

    /** Contrast ratio between two OPAQUE colors, per WCAG 2.x (1:1 .. 21:1). */
    fun ratio(a: Color, b: Color): Double {
        val la = relativeLuminance(a)
        val lb = relativeLuminance(b)
        val hi = maxOf(la, lb)
        val lo = minOf(la, lb)
        return (hi + 0.05) / (lo + 0.05)
    }

    /**
     * Alpha-composites [fg] over [bg] (both treated as opaque; [fg]'s own alpha
     * drives the blend) — the "post-alpha-compositing" math required for tokens
     * that are semi-transparent tints (e.g. errStrong-on-tint scenarios).
     */
    fun compositeOver(fg: Color, bg: Color): Color {
        val a = fg.alpha
        return Color(
            red = fg.red * a + bg.red * (1 - a),
            green = fg.green * a + bg.green * (1 - a),
            blue = fg.blue * a + bg.blue * (1 - a),
            alpha = 1f,
        )
    }
}
