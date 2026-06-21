package com.copypaste.android

import com.copypaste.android.ui.theme.GLASS_ALPHA_DARK
import com.copypaste.android.ui.theme.GLASS_ALPHA_LIGHT
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Rationale enforcement tests for CopyPaste-2ji4: the dark glass alpha on Android
 * intentionally differs from the web styleguide's ".40" value.
 *
 * The web uses a CSS gradient for the CARD tier:
 *   light  → linear-gradient(rgba(255,255,255,0.58), rgba(255,255,255,0.40))
 *   dark   → no per-tier dark gradient published in the styleguide
 * The "dark .40" sometimes cited refers to the CARD-tier light gradient's BOTTOM
 * alpha — NOT a flat dark glass value.  Applying 0.40f flat on Android dark would
 * fail WCAG-AA contrast on the dark surfaces (#1E202A background).
 *
 * Android uses a single flat Color (no gradient support via Modifier.background),
 * so GLASS_ALPHA_DARK = 0.55f is calibrated to the perceptual midpoint of the
 * published dark glass spec while maintaining text contrast.
 *
 * These tests pin the value and document the deliberate divergence so a future
 * reviewer does not silently "fix" 0.55f → 0.40f (which would be incorrect).
 *
 * Pure-JVM, no Android SDK needed.  Run with: ./gradlew :app:testDebugUnitTest
 */
class GlassDarkAlphaRationaleTest {

    /**
     * Dark alpha must be exactly 0.55f — the value calibrated for Android dark
     * surfaces to maintain WCAG-AA contrast.  NOT 0.40f (that is the web CARD-tier
     * light gradient bottom, not a dark value).
     */
    @Test
    fun `GLASS_ALPHA_DARK is 0_55f — calibrated for Android dark contrast not web gradient bottom`() {
        assertEquals(
            "GLASS_ALPHA_DARK must be 0.55f (Android flat dark alpha, calibrated for WCAG-AA on #1E202A). " +
                "Do NOT change to 0.40f — that is the web CARD-tier light gradient bottom alpha " +
                "and would fail contrast on dark surfaces (CopyPaste-2ji4).",
            0.55f,
            GLASS_ALPHA_DARK,
            0.001f,
        )
    }

    /**
     * The web "dark .40" is the CARD-tier light gradient BOTTOM — confirm Android
     * dark alpha is strictly greater to show they serve different purposes.
     */
    @Test
    fun `GLASS_ALPHA_DARK exceeds web CARD-tier light gradient bottom 0_40`() {
        val webCardLightBottom = 0.40f
        assertTrue(
            "GLASS_ALPHA_DARK (${GLASS_ALPHA_DARK}) must exceed the web CARD-tier light gradient " +
                "bottom (${webCardLightBottom}) — they are different values for different purposes. " +
                "Android dark uses a flat 0.55f; web uses 0.58→0.40 gradient on LIGHT only (CopyPaste-2ji4).",
            GLASS_ALPHA_DARK > webCardLightBottom,
        )
    }

    /**
     * Light alpha must remain more opaque than dark to maintain the relative
     * hierarchy: light glass is a warm near-white tint and needs a higher alpha
     * to be perceptible over light backgrounds.
     */
    @Test
    fun `GLASS_ALPHA_LIGHT exceeds GLASS_ALPHA_DARK — light glass is more opaque`() {
        assertTrue(
            "Light glass alpha ($GLASS_ALPHA_LIGHT) must exceed dark glass alpha " +
                "($GLASS_ALPHA_DARK) — light surfaces require a higher fill alpha " +
                "to be perceptible over bright backgrounds (CopyPaste-2ji4)",
            GLASS_ALPHA_LIGHT > GLASS_ALPHA_DARK,
        )
    }

    /**
     * Both alphas must be valid (0, 1) exclusive — a 0 or 1 would mean invisible or
     * opaque, defeating the glass effect.
     */
    @Test
    fun `both glass alphas are in the open interval (0, 1)`() {
        assertTrue(
            "GLASS_ALPHA_DARK must be strictly between 0 and 1",
            GLASS_ALPHA_DARK > 0f && GLASS_ALPHA_DARK < 1f,
        )
        assertTrue(
            "GLASS_ALPHA_LIGHT must be strictly between 0 and 1",
            GLASS_ALPHA_LIGHT > 0f && GLASS_ALPHA_LIGHT < 1f,
        )
    }
}
