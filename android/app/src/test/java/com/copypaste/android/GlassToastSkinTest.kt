package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinElevation
import com.copypaste.android.ui.theme.SkinMaterial
import com.copypaste.android.ui.theme.skinTokens
import com.copypaste.android.ui.glassToastRadiusDp
import com.copypaste.android.ui.glassToastShadowElevationDp
import com.copypaste.android.ui.syncSheetEffectiveTranslucent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the skin-aware GlassToast and SyncStatusBadge logic (A-C9).
 *
 * Verifies:
 *  1. GlassToast shape radius: CLASSIC = 10dp (frozen), QUIET = tok.radiusControl,
 *     VAPOR = tok.radiusControl.
 *  2. GlassToast shadow elevation: GLASS_FLOAT skins → 6dp, NONE elevation (Quiet) → 0dp.
 *  3. SyncStatusBadge sheet effective-translucency gate: GLASS material + user pref = true,
 *     FLAT material = false regardless of user pref.
 *
 * All tests are pure Kotlin — no Android SDK, no Compose runtime.
 */
class GlassToastSkinTest {

    // ── 1. GlassToast shape radius ──────────────────────────────────────────

    /**
     * CLASSIC must remain 10dp (byte-identical — the current hardcoded value).
     * Note: tok.radiusControl for CLASSIC is 9dp, but the toast was 10dp before
     * skins were introduced. The frozen-Classic rule overrides the token here.
     */
    @Test
    fun `GlassToast radius is 10dp for CLASSIC (byte-identical frozen value)`() {
        assertEquals(10f, glassToastRadiusDp(Skin.CLASSIC), 0.01f)
    }

    /** QUIET uses tok.radiusControl = 7dp (flat, reduced radii). */
    @Test
    fun `GlassToast radius is 7dp for QUIET (tok_radiusControl)`() {
        val expected = skinTokens(Skin.QUIET).radiusControl.value
        assertEquals(7f, expected, 0.01f)
        assertEquals(expected, glassToastRadiusDp(Skin.QUIET), 0.01f)
    }

    /** VAPOR uses tok.radiusControl = 12dp (larger, refined glass). */
    @Test
    fun `GlassToast radius is 12dp for VAPOR (tok_radiusControl)`() {
        val expected = skinTokens(Skin.VAPOR).radiusControl.value
        assertEquals(12f, expected, 0.01f)
        assertEquals(expected, glassToastRadiusDp(Skin.VAPOR), 0.01f)
    }

    /** Verify the non-Classic path always matches tok.radiusControl. */
    @Test
    fun `GlassToast radius matches tok_radiusControl for non-Classic skins`() {
        listOf(Skin.QUIET, Skin.VAPOR).forEach { skin ->
            val tok = skinTokens(skin)
            assertEquals(
                "$skin radius must equal tok.radiusControl",
                tok.radiusControl.value,
                glassToastRadiusDp(skin),
                0.01f,
            )
        }
    }

    // ── 2. GlassToast shadow elevation ─────────────────────────────────────

    /**
     * CLASSIC uses GLASS_FLOAT elevation → 6dp shadow elevation.
     * The 6dp constant mirrors the pre-skin hardcoded value; frozen for CLASSIC.
     */
    @Test
    fun `GlassToast shadow elevation is 6dp for CLASSIC (GLASS_FLOAT, frozen)`() {
        assertEquals(SkinElevation.GLASS_FLOAT, skinTokens(Skin.CLASSIC).elevation)
        assertEquals(6f, glassToastShadowElevationDp(Skin.CLASSIC), 0.01f)
    }

    /** QUIET has NONE elevation → 0dp shadow (flat material, no shadow). */
    @Test
    fun `GlassToast shadow elevation is 0dp for QUIET (NONE elevation)`() {
        assertEquals(SkinElevation.NONE, skinTokens(Skin.QUIET).elevation)
        assertEquals(0f, glassToastShadowElevationDp(Skin.QUIET), 0.01f)
    }

    /** VAPOR has GLASS_FLOAT elevation → 6dp shadow (same as CLASSIC). */
    @Test
    fun `GlassToast shadow elevation is 6dp for VAPOR (GLASS_FLOAT)`() {
        assertEquals(SkinElevation.GLASS_FLOAT, skinTokens(Skin.VAPOR).elevation)
        assertEquals(6f, glassToastShadowElevationDp(Skin.VAPOR), 0.01f)
    }

    /** Shadow elevation is never negative for any skin. */
    @Test
    fun `GlassToast shadow elevation is non-negative for all skins`() {
        Skin.entries.forEach { skin ->
            assertTrue("$skin shadow elevation must be >= 0", glassToastShadowElevationDp(skin) >= 0f)
        }
    }

    // ── 3. SyncStatusBadge sheet effective-translucency gate ───────────────

    /**
     * GLASS material + user pref ON → effectively translucent.
     * The sheet container becomes transparent so the LiquidGlassSurface
     * draws the frosted glass effect.
     */
    @Test
    fun `Sheet is effectively translucent for GLASS skin with user pref on`() {
        listOf(Skin.CLASSIC, Skin.VAPOR).forEach { skin ->
            assertEquals(SkinMaterial.GLASS, skinTokens(skin).material)
            assertTrue(
                "$skin with userPref=true must be effectively translucent",
                syncSheetEffectiveTranslucent(skin = skin, userPrefTranslucent = true),
            )
        }
    }

    /**
     * GLASS material + user pref OFF → not effectively translucent.
     * The user's explicit "no translucency" override applies even on glass skins.
     */
    @Test
    fun `Sheet is not effectively translucent for GLASS skin with user pref off`() {
        listOf(Skin.CLASSIC, Skin.VAPOR).forEach { skin ->
            assertFalse(
                "$skin with userPref=false must not be effectively translucent",
                syncSheetEffectiveTranslucent(skin = skin, userPrefTranslucent = false),
            )
        }
    }

    /**
     * FLAT material (Quiet) → never translucent, regardless of user pref.
     * Mirrors LiquidGlassSurface's effectiveTranslucent gate in Components.kt.
     */
    @Test
    fun `Sheet is never effectively translucent for QUIET (FLAT material)`() {
        assertEquals(SkinMaterial.FLAT, skinTokens(Skin.QUIET).material)
        assertFalse(
            "QUIET with userPref=true must NOT be effectively translucent (FLAT material)",
            syncSheetEffectiveTranslucent(skin = Skin.QUIET, userPrefTranslucent = true),
        )
        assertFalse(
            "QUIET with userPref=false must NOT be effectively translucent",
            syncSheetEffectiveTranslucent(skin = Skin.QUIET, userPrefTranslucent = false),
        )
    }

    // ── 4. Cross-skin invariants ────────────────────────────────────────────

    /** All skins return a positive radius (never zero, never negative). */
    @Test
    fun `GlassToast radius is positive for all skins`() {
        Skin.entries.forEach { skin ->
            assertTrue("$skin radius must be positive", glassToastRadiusDp(skin) > 0f)
        }
    }

    /**
     * The Classic toast radius (10dp) is larger than the Quiet radius (7dp),
     * reflecting the larger control radius of the glass aesthetic vs. the flat one.
     */
    @Test
    fun `Classic toast radius is larger than Quiet radius`() {
        assertTrue(
            "Classic radius (10dp) must exceed Quiet radius (7dp)",
            glassToastRadiusDp(Skin.CLASSIC) > glassToastRadiusDp(Skin.QUIET),
        )
    }

    /** Vapor has the largest toast radius (12dp > Classic 10dp > Quiet 7dp). */
    @Test
    fun `Vapor toast radius is largest`() {
        assertTrue(
            "VAPOR radius must exceed CLASSIC",
            glassToastRadiusDp(Skin.VAPOR) > glassToastRadiusDp(Skin.CLASSIC),
        )
        assertTrue(
            "VAPOR radius must exceed QUIET",
            glassToastRadiusDp(Skin.VAPOR) > glassToastRadiusDp(Skin.QUIET),
        )
    }
}
