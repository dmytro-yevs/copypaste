package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.SkinNavActive
import com.copypaste.android.ui.theme.SkinRowTreatment
import com.copypaste.android.ui.theme.skinTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for HistoryActivity skin treatment (A-C1).
 *
 * Verifies the token-driven branching logic for:
 *   (a) Background canvas mode  — tok.background
 *   (b) Row treatment            — tok.rowTreatment
 *   (c) Nav active indicator     — tok.navActive
 *
 * Classic MUST produce values identical to the pre-skin implementation so these
 * tests also act as a "frozen classic" regression guard.
 *
 * Pure-function, no Android runtime required.
 */
class HistorySkinTest {

    // ── Helpers that mirror the branching added in HistoryActivity ─────────────

    /**
     * Mirrors the background mode decision in HistoryScreen's Scaffold modifier.
     * Returns a string tag describing which background is active — NOT a Compose
     * Modifier (uninstantiable in JVM tests), so the logic is testable in isolation.
     *
     * Values:
     *   "aurora"    — animated full aurora canvas (Classic + translucent + paintCanvas)
     *   "tint_blob" — single static accent-tint blob (Vapor + translucent + paintCanvas)
     *   "solid"     — plain solid background (Quiet/FLAT skin, or non-translucent)
     *   "none"      — paintCanvasBackdrop=false (embedded in MainShell)
     */
    private fun backgroundMode(
        skin: Skin,
        translucent: Boolean,
        paintCanvasBackdrop: Boolean,
    ): String {
        if (!paintCanvasBackdrop) return "none"
        if (!translucent) return "solid"
        val tok = skinTokens(skin)
        return when (tok.background) {
            SkinBackground.AURORA    -> "aurora"
            SkinBackground.FLAT      -> "solid"
            SkinBackground.TINT_BLOB -> "tint_blob"
        }
    }

    /**
     * Mirrors the row-gap decision in HistoryList's LazyColumn verticalArrangement.
     * Returns the gap in dp for the skin (0 for CARD/LINE, tok.rowGap for INSET).
     */
    private fun rowGapDp(skin: Skin): Float = skinTokens(skin).rowGap.value

    /**
     * Mirrors whether a HorizontalDivider is rendered after each row.
     * CARD and LINE both show dividers; INSET uses spacing instead of dividers.
     */
    private fun showRowDivider(skin: Skin): Boolean =
        skinTokens(skin).rowTreatment != SkinRowTreatment.INSET

    /**
     * Mirrors the DeviceChip nav active indicator style.
     * Returns a string tag for the active-chip visual treatment.
     *
     *   "fill_glow"  — filled accent pill (Classic)
     *   "tint"       — lightweight tinted background (Quiet)
     *   "glass_ring" — frosted glass chip with outline ring (Vapor)
     */
    private fun navActiveStyle(skin: Skin): String = when (skinTokens(skin).navActive) {
        SkinNavActive.FILL_GLOW  -> "fill_glow"
        SkinNavActive.TINT       -> "tint"
        SkinNavActive.GLASS_RING -> "glass_ring"
    }

    // ── (a) Background ─────────────────────────────────────────────────────────

    @Test
    fun `Classic background is aurora when translucent and paintCanvas`() {
        assertEquals("aurora", backgroundMode(Skin.CLASSIC, translucent = true, paintCanvasBackdrop = true))
    }

    @Test
    fun `Classic background without translucency is solid`() {
        assertEquals("solid", backgroundMode(Skin.CLASSIC, translucent = false, paintCanvasBackdrop = true))
    }

    @Test
    fun `Classic background when not painting canvas returns none`() {
        // Embedded in MainShell — the shell already paints the backdrop.
        assertEquals("none", backgroundMode(Skin.CLASSIC, translucent = true, paintCanvasBackdrop = false))
    }

    @Test
    fun `Quiet background is solid regardless of translucency pref`() {
        // Quiet skin has FLAT background — no aurora even when translucency is on.
        assertEquals("solid", backgroundMode(Skin.QUIET, translucent = true, paintCanvasBackdrop = true))
        assertEquals("solid", backgroundMode(Skin.QUIET, translucent = false, paintCanvasBackdrop = true))
    }

    @Test
    fun `Vapor background is tint_blob when translucent and paintCanvas`() {
        assertEquals("tint_blob", backgroundMode(Skin.VAPOR, translucent = true, paintCanvasBackdrop = true))
    }

    @Test
    fun `Vapor background without translucency is solid`() {
        assertEquals("solid", backgroundMode(Skin.VAPOR, translucent = false, paintCanvasBackdrop = true))
    }

    @Test
    fun `background token matches Skin token registry for all skins`() {
        assertEquals(SkinBackground.AURORA,    skinTokens(Skin.CLASSIC).background)
        assertEquals(SkinBackground.FLAT,      skinTokens(Skin.QUIET).background)
        assertEquals(SkinBackground.TINT_BLOB, skinTokens(Skin.VAPOR).background)
    }

    // ── (b) Row treatment ──────────────────────────────────────────────────────

    @Test
    fun `Classic row treatment is CARD`() {
        assertEquals(SkinRowTreatment.CARD, skinTokens(Skin.CLASSIC).rowTreatment)
    }

    @Test
    fun `Classic shows row divider (CARD treatment)`() {
        assertTrue("Classic CARD rows must show dividers", showRowDivider(Skin.CLASSIC))
    }

    @Test
    fun `Classic row gap is 0dp (CARD treatment — flush rows)`() {
        assertEquals(0f, rowGapDp(Skin.CLASSIC), 0.01f)
    }

    @Test
    fun `Quiet row treatment is LINE`() {
        assertEquals(SkinRowTreatment.LINE, skinTokens(Skin.QUIET).rowTreatment)
    }

    @Test
    fun `Quiet shows row divider (LINE treatment)`() {
        assertTrue("Quiet LINE rows must show dividers", showRowDivider(Skin.QUIET))
    }

    @Test
    fun `Quiet row gap is 0dp (LINE treatment — dividers, no gap)`() {
        assertEquals(0f, rowGapDp(Skin.QUIET), 0.01f)
    }

    @Test
    fun `Vapor row treatment is INSET`() {
        assertEquals(SkinRowTreatment.INSET, skinTokens(Skin.VAPOR).rowTreatment)
    }

    @Test
    fun `Vapor hides row divider (INSET treatment uses gap instead)`() {
        assertFalse("Vapor INSET rows must NOT show dividers", showRowDivider(Skin.VAPOR))
    }

    @Test
    fun `Vapor row gap is 3dp (INSET treatment)`() {
        assertEquals(3f, rowGapDp(Skin.VAPOR), 0.01f)
    }

    @Test
    fun `INSET is the only treatment that suppresses dividers`() {
        // CARD and LINE both use dividers; only INSET uses spacing.
        for (skin in Skin.entries) {
            val tok = skinTokens(skin)
            val expectedDivider = tok.rowTreatment != SkinRowTreatment.INSET
            assertEquals(
                "$skin divider expectation mismatch",
                expectedDivider,
                showRowDivider(skin),
            )
        }
    }

    // ── (c) Nav active indicator ───────────────────────────────────────────────

    @Test
    fun `Classic nav active is fill_glow (filled accent pill)`() {
        assertEquals("fill_glow", navActiveStyle(Skin.CLASSIC))
    }

    @Test
    fun `Quiet nav active is tint (lightweight tinted background)`() {
        assertEquals("tint", navActiveStyle(Skin.QUIET))
    }

    @Test
    fun `Vapor nav active is glass_ring (frosted chip with outline ring)`() {
        assertEquals("glass_ring", navActiveStyle(Skin.VAPOR))
    }

    @Test
    fun `nav active token matches Skin token registry for all skins`() {
        assertEquals(SkinNavActive.FILL_GLOW,  skinTokens(Skin.CLASSIC).navActive)
        assertEquals(SkinNavActive.TINT,       skinTokens(Skin.QUIET).navActive)
        assertEquals(SkinNavActive.GLASS_RING, skinTokens(Skin.VAPOR).navActive)
    }

    // ── Classic frozen-state regression ────────────────────────────────────────

    @Test
    fun `Classic produces aurora background not tint_blob or solid`() {
        val mode = backgroundMode(Skin.CLASSIC, translucent = true, paintCanvasBackdrop = true)
        assertEquals(
            "Classic must use aurora background (byte-identical to pre-skin)",
            "aurora",
            mode,
        )
        assertFalse("Classic must NOT use tint_blob", mode == "tint_blob")
        assertFalse("Classic must NOT use solid",     mode == "solid")
    }

    @Test
    fun `Classic produces fill_glow nav active not tint or glass_ring`() {
        val style = navActiveStyle(Skin.CLASSIC)
        assertEquals(
            "Classic nav active must be fill_glow (byte-identical to pre-skin)",
            "fill_glow",
            style,
        )
        assertFalse("Classic must NOT use tint",       style == "tint")
        assertFalse("Classic must NOT use glass_ring", style == "glass_ring")
    }

    @Test
    fun `Classic shows dividers not INSET gap treatment`() {
        assertTrue(
            "Classic must show row dividers (byte-identical to pre-skin)",
            showRowDivider(Skin.CLASSIC),
        )
        assertEquals(
            "Classic row gap must be 0dp (byte-identical to pre-skin)",
            0f,
            rowGapDp(Skin.CLASSIC),
            0.01f,
        )
    }
}
