package com.copypaste.android

import com.copypaste.android.ui.syncSheetEffectiveTranslucent
import com.copypaste.android.ui.syncSheetGlassTier
import com.copypaste.android.ui.theme.GlassTier
import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinMaterial
import com.copypaste.android.ui.theme.skinTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for CopyPaste-ohki: frosted LiquidGlassSurface wrapping in
 * SyncStatusSheet for glass skins.
 *
 * Verifies:
 *  1. [syncSheetGlassTier] always returns [GlassTier.STRONG], mirroring GlassAlertDialog.
 *  2. The glass-wrap gate ([syncSheetEffectiveTranslucent]) correctly enables the wrap
 *     for GLASS-material skins and disables it for FLAT-material skins (Quiet).
 *  3. The chosen tier (STRONG) is appropriate for a modal sheet surface.
 *
 * All tests are pure Kotlin — no Android SDK, no Compose runtime.
 */
class SyncStatusSheetGlassTest {

    // ── 1. syncSheetGlassTier ──────────────────────────────────────────────

    /**
     * The sync status sheet always uses GlassTier.STRONG for its frosted fill
     * when wrapping in LiquidGlassSurface, mirroring GlassAlertDialog (CopyPaste-ohki).
     */
    @Test
    fun `syncSheetGlassTier returns STRONG to mirror GlassAlertDialog`() {
        assertEquals(GlassTier.STRONG, syncSheetGlassTier())
    }

    /**
     * STRONG tier blur (40dp) is greater than CARD/GLASS tier blur (28dp),
     * confirming the sheet is visually distinct as a modal surface.
     */
    @Test
    fun `STRONG tier has higher blur than CARD and GLASS tiers`() {
        assertTrue(
            "STRONG blur must exceed CARD blur",
            GlassTier.STRONG.blur > GlassTier.CARD.blur,
        )
        assertTrue(
            "STRONG blur must exceed GLASS blur",
            GlassTier.STRONG.blur > GlassTier.GLASS.blur,
        )
    }

    // ── 2. Glass-wrap gate — GLASS-material skins ──────────────────────────

    /**
     * CLASSIC (GLASS material) + pref ON → wrap is active.
     */
    @Test
    fun `Glass wrap enabled for CLASSIC with pref on`() {
        assertEquals(SkinMaterial.GLASS, skinTokens(Skin.CLASSIC).material)
        assertTrue(
            "CLASSIC + pref=true must enable glass wrap",
            syncSheetEffectiveTranslucent(Skin.CLASSIC, userPrefTranslucent = true),
        )
    }

    /**
     * VAPOR (GLASS material) + pref ON → wrap is active.
     */
    @Test
    fun `Glass wrap enabled for VAPOR with pref on`() {
        assertEquals(SkinMaterial.GLASS, skinTokens(Skin.VAPOR).material)
        assertTrue(
            "VAPOR + pref=true must enable glass wrap",
            syncSheetEffectiveTranslucent(Skin.VAPOR, userPrefTranslucent = true),
        )
    }

    /**
     * GLASS-material skins + pref OFF → no wrap (user explicitly chose opaque).
     */
    @Test
    fun `Glass wrap disabled for GLASS skins when pref off`() {
        listOf(Skin.CLASSIC, Skin.VAPOR).forEach { skin ->
            assertFalse(
                "$skin + pref=false must disable glass wrap",
                syncSheetEffectiveTranslucent(skin, userPrefTranslucent = false),
            )
        }
    }

    // ── 3. Glass-wrap gate — FLAT-material skins ───────────────────────────

    /**
     * QUIET (FLAT material) must never wrap in LiquidGlassSurface regardless of pref
     * — the opaque column layout is correct for flat skins.
     */
    @Test
    fun `Glass wrap never enabled for QUIET (FLAT material) even with pref on`() {
        assertEquals(SkinMaterial.FLAT, skinTokens(Skin.QUIET).material)
        assertFalse(
            "QUIET + pref=true must NOT enable glass wrap (FLAT material gate)",
            syncSheetEffectiveTranslucent(Skin.QUIET, userPrefTranslucent = true),
        )
    }

    @Test
    fun `Glass wrap never enabled for QUIET with pref off`() {
        assertFalse(
            "QUIET + pref=false must NOT enable glass wrap",
            syncSheetEffectiveTranslucent(Skin.QUIET, userPrefTranslucent = false),
        )
    }

    // ── 4. STRONG tier invariants ──────────────────────────────────────────

    /**
     * STRONG tier light alpha is floored at 0.92 (styleguide .surface-strong),
     * ensuring the modal sheet is near-opaque enough to keep text legible.
     */
    @Test
    fun `STRONG tier light alpha is 0_92 (surface-strong floor)`() {
        assertEquals(0.92f, GlassTier.STRONG.lightAlphaTop, 0.001f)
        assertEquals(0.92f, GlassTier.STRONG.lightAlphaBottom, 0.001f)
    }

    /**
     * STRONG tier dark alpha is 0.86, higher than the GLASS/CARD 0.55 baseline,
     * so the dark-theme modal stands out over the scrim.
     */
    @Test
    fun `STRONG tier dark alpha is above GLASS_CARD baseline`() {
        assertTrue(
            "STRONG dark alpha must exceed GLASS tier dark alpha",
            GlassTier.STRONG.darkAlpha > GlassTier.GLASS.darkAlpha,
        )
    }
}
