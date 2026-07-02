package com.copypaste.android.ui.theme

import android.animation.ValueAnimator
import android.content.Context
import android.provider.Settings as AndroidSettings
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.snap
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext

/**
 * STYLEGUIDE §6 motion tokens. Durations only — [CpMotion.reduced] is NOT a
 * user-facing setting (android-design-system "no in-app motion toggle"
 * requirement); it is derived exclusively from the system animator-duration
 * scale (accessibility "Remove animations" / developer-options 0×).
 */
object CpMotion {
    const val FAST_MS: Int = 120
    const val DEFAULT_MS: Int = 200
    const val THEME_MS: Int = 300

    /** STYLEGUIDE §6 easing curve — `cubic-bezier(.2,.8,.2,1)`. */
    val Ease = CubicBezierEasing(0.2f, 0.8f, 0.2f, 1f)
}

/**
 * Resolves [durationMs] to 0 when [reduced] system motion is active.
 * Pure function — no Compose/Context dependency — so callers and tests can
 * evaluate it directly.
 */
fun cpMotionDuration(durationMs: Int, reduced: Boolean): Int = if (reduced) 0 else durationMs

/**
 * Chooses between [spec] and a hard [snap] when [reduced] is true. Reduced
 * motion MUST disable a spring entirely (springs have no "duration" to zero —
 * damping/stiffness still animate at 0ms), not merely shrink its duration to
 * 0ms — callers building a nav-pill/spring transition select the
 * [AnimationSpec] through this gate rather than zeroing spring parameters.
 */
fun <T> cpMotionSpec(reduced: Boolean, spec: () -> AnimationSpec<T>): AnimationSpec<T> =
    if (reduced) snap() else spec()

/**
 * Non-composable check — true when the OS "remove animations" / animator
 * duration scale is 0 (accessibility setting) or animators are globally
 * disabled (developer options). Covers both surfaces the system exposes;
 * there is no in-app equivalent (STYLEGUIDE §2/§6).
 */
fun isSystemReducedMotion(context: Context): Boolean {
    if (!ValueAnimator.areAnimatorsEnabled()) return true
    val scale = try {
        AndroidSettings.Global.getFloat(
            context.contentResolver,
            AndroidSettings.Global.ANIMATOR_DURATION_SCALE,
        )
    } catch (_: AndroidSettings.SettingNotFoundException) {
        1f
    }
    return scale == 0f
}

/** Composable convenience over [isSystemReducedMotion], stable for the composition's lifetime. */
@Composable
fun rememberCpMotionReduced(): Boolean {
    val context = LocalContext.current
    return remember(context) { isSystemReducedMotion(context) }
}
