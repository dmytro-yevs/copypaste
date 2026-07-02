package com.copypaste.android.ui.theme

import androidx.compose.animation.animateColor
import androidx.compose.animation.core.Transition
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.updateTransition
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.graphics.Color

// ---------------------------------------------------------------------------
// STYLEGUIDE `--dur-theme` (300ms) crossfade (android-material3-redesign task
// 1.6): an EXPLICIT, labeled Compose Transition over every CpColors field plus
// the accent-derived primary/onPrimary pair, keyed on (isDark, accent) — a
// theme OR accent switch fades every token instead of snapping. Collapses to
// an instant (0ms) swap under reduced motion via [cpMotionDuration] (§4).
//
// Kept in its own file (not inline in Theme.kt/CopyPasteTheme) because the
// field-by-field wiring is mechanical and would otherwise dominate the theme
// builder's line count.
// ---------------------------------------------------------------------------

/** One labeled color leg of the theme-crossfade [Transition], reading [select] from the target theme's static [CpColors]. */
@Composable
private fun Transition<Boolean>.crossfadeColor(
    label: String,
    durationMs: Int,
    select: (CpColors) -> Color,
): Color {
    val value by animateColor(
        transitionSpec = { tween(durationMs) },
        label = label,
    ) { dark -> select(if (dark) DarkColors else LightColors) }
    return value
}

/**
 * Crossfades every [CpColors] field across an isDark change over `--dur-theme`
 * (or instantly under [reduced] motion). Independent of [AccentColor] — none
 * of CpColors' fields depend on the accent axis.
 */
@Composable
internal fun animateCpColorsCrossfade(isDark: Boolean, reduced: Boolean): CpColors {
    val transition = updateTransition(targetState = isDark, label = "CpColors crossfade")
    val d = cpMotionDuration(CpMotion.THEME_MS, reduced)
    return CpColors(
        bg = transition.crossfadeColor("bg", d) { it.bg },
        panel = transition.crossfadeColor("panel", d) { it.panel },
        elevated = transition.crossfadeColor("elevated", d) { it.elevated },
        card = transition.crossfadeColor("card", d) { it.card },
        raised = transition.crossfadeColor("raised", d) { it.raised },
        raised2 = transition.crossfadeColor("raised2", d) { it.raised2 },
        border = transition.crossfadeColor("border", d) { it.border },
        divider = transition.crossfadeColor("divider", d) { it.divider },
        text = transition.crossfadeColor("text", d) { it.text },
        dim = transition.crossfadeColor("dim", d) { it.dim },
        faint = transition.crossfadeColor("faint", d) { it.faint },
        mute = transition.crossfadeColor("mute", d) { it.mute },
        hover = transition.crossfadeColor("hover", d) { it.hover },
        pressed = transition.crossfadeColor("pressed", d) { it.pressed },
        scrim = transition.crossfadeColor("scrim", d) { it.scrim },
        ok = transition.crossfadeColor("ok", d) { it.ok },
        warn = transition.crossfadeColor("warn", d) { it.warn },
        err = transition.crossfadeColor("err", d) { it.err },
        info = transition.crossfadeColor("info", d) { it.info },
        errStrong = transition.crossfadeColor("errStrong", d) { it.errStrong },
        infoStrong = transition.crossfadeColor("infoStrong", d) { it.infoStrong },
        okStrong = transition.crossfadeColor("okStrong", d) { it.okStrong },
        cText = transition.crossfadeColor("cText", d) { it.cText },
        cUrl = transition.crossfadeColor("cUrl", d) { it.cUrl },
        cMail = transition.crossfadeColor("cMail", d) { it.cMail },
        cNum = transition.crossfadeColor("cNum", d) { it.cNum },
        cCode = transition.crossfadeColor("cCode", d) { it.cCode },
        cJson = transition.crossfadeColor("cJson", d) { it.cJson },
        cColor = transition.crossfadeColor("cColor", d) { it.cColor },
        cFile = transition.crossfadeColor("cFile", d) { it.cFile },
        cImage = transition.crossfadeColor("cImage", d) { it.cImage },
        cSecret = transition.crossfadeColor("cSecret", d) { it.cSecret },
    )
}

/** Crossfades the accent-derived (primary, onPrimary) pair across an isDark OR accent change. */
@Composable
internal fun animateAccentCrossfade(isDark: Boolean, accent: AccentColor, reduced: Boolean): Pair<Color, Color> {
    val transition = updateTransition(targetState = isDark to accent, label = "Accent crossfade")
    val d = cpMotionDuration(CpMotion.THEME_MS, reduced)
    val primary by transition.animateColor(transitionSpec = { tween(d) }, label = "primary") { (dark, acc) -> acc.base(dark) }
    val onPrimary by transition.animateColor(transitionSpec = { tween(d) }, label = "onPrimary") { (dark, acc) -> acc.on(dark) }
    return primary to onPrimary
}
