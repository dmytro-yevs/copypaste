package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.SkinElevation
import com.copypaste.android.ui.theme.SkinMaterial
import com.copypaste.android.ui.theme.SkinNavActive
import com.copypaste.android.ui.theme.SkinRowTreatment
import com.copypaste.android.ui.theme.SkinShadowCard
import com.copypaste.android.ui.theme.SkinShadowFloat
import com.copypaste.android.ui.theme.skinTokens
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for the Skin axis token registry (A-F4).
 *
 * Validates:
 *  1. Skin registry returns a bundle for every enum value (no crash).
 *  2. Classic tokens reproduce the current hardcoded values (frozen — §3 rule 1).
 *  3. Quiet tokens specify FLAT material and zero glass parameters.
 *  4. Vapor tokens specify GLASS material with boosted blur/saturation and tint.
 *  5. Per-composable mapping logic (card radius, shadow presence) is exercised
 *     to match the logic in Components.kt A-F4 changes.
 *  6. tok.glow, tok.motionScale are in valid ranges.
 *
 * These are pure-function tests — no Android SDK, no Compose runtime needed.
 */
class SkinTokensTest {

    // ── 1. Registry completeness ───────────────────────────────────────────────

    @Test
    fun `skinTokens resolves for every Skin enum entry`() {
        Skin.entries.forEach { skin ->
            // Must not throw.
            val tok = skinTokens(skin)
            // Sanity: returned object is not the wrong skin's tokens.
            // (We assert concrete values per skin below.)
            assertTrue("skinTokens($skin) must return a valid bundle", tok.glassBlurDp.value >= 0f)
        }
    }

    @Test
    fun `Skin enum has exactly three entries`() {
        assertEquals("Skin must have exactly 3 entries (CLASSIC, QUIET, VAPOR)", 3, Skin.entries.size)
    }

    @Test
    fun `Skin DEFAULT is CLASSIC`() {
        assertEquals(Skin.CLASSIC, Skin.DEFAULT)
    }

    // ── 2. Classic token values match the current hardcoded implementation ─────

    @Test
    fun `Classic material is GLASS`() {
        assertEquals(SkinMaterial.GLASS, skinTokens(Skin.CLASSIC).material)
    }

    @Test
    fun `Classic glassBlurDp is 28`() {
        // Current GlassTier_GLASS.blur = 28.dp; tok.glassBlurDp must match.
        assertEquals(28f, skinTokens(Skin.CLASSIC).glassBlurDp.value, 0.01f)
    }

    @Test
    fun `Classic fillAlpha is 0_62`() {
        assertEquals(0.62f, skinTokens(Skin.CLASSIC).fillAlpha, 0.001f)
    }

    @Test
    fun `Classic tintAlpha is 0 (no accent wash)`() {
        assertEquals(0f, skinTokens(Skin.CLASSIC).tintAlpha, 0.001f)
    }

    @Test
    fun `Classic elevation is GLASS_FLOAT`() {
        assertEquals(SkinElevation.GLASS_FLOAT, skinTokens(Skin.CLASSIC).elevation)
    }

    @Test
    fun `Classic shadowCard is E2`() {
        assertEquals(SkinShadowCard.E2, skinTokens(Skin.CLASSIC).shadowCard)
    }

    @Test
    fun `Classic shadowFloat is E3`() {
        assertEquals(SkinShadowFloat.E3, skinTokens(Skin.CLASSIC).shadowFloat)
    }

    @Test
    fun `Classic radiusControl is 9dp`() {
        // RadiusControl in Shapes_kt = 9dp; tok matches.
        assertEquals(9f, skinTokens(Skin.CLASSIC).radiusControl.value, 0.01f)
    }

    @Test
    fun `Classic radiusChip is 7dp`() {
        assertEquals(7f, skinTokens(Skin.CLASSIC).radiusChip.value, 0.01f)
    }

    @Test
    fun `Classic radiusCard token is 12dp matching current card rendering (CopyPaste-xxjt)`() {
        // CopyPaste-xxjt: Classic token corrected from 14dp to 12dp to match the
        // frozen Classic rendering (PG-57, RadiusCard). Components.kt now reads
        // tok.radiusCard uniformly — no per-skin-id branch needed.
        assertEquals(
            "Classic radiusCard token must be 12dp (byte-identical to pre-skin rendering)",
            12f, skinTokens(Skin.CLASSIC).radiusCard.value, 0.01f,
        )
    }

    @Test
    fun `Classic radiusModal is 16dp matching GlassAlertDialog hardcode`() {
        // GlassAlertDialog previously used RoundedCornerShape(16.dp) — must match.
        assertEquals(16f, skinTokens(Skin.CLASSIC).radiusModal.value, 0.01f)
    }

    @Test
    fun `Classic rowTreatment is CARD`() {
        assertEquals(SkinRowTreatment.CARD, skinTokens(Skin.CLASSIC).rowTreatment)
    }

    @Test
    fun `Classic navActive is FILL_GLOW`() {
        assertEquals(SkinNavActive.FILL_GLOW, skinTokens(Skin.CLASSIC).navActive)
    }

    @Test
    fun `Classic background is AURORA`() {
        assertEquals(SkinBackground.AURORA, skinTokens(Skin.CLASSIC).background)
    }

    @Test
    fun `Classic glow is 0_62`() {
        assertEquals(0.62f, skinTokens(Skin.CLASSIC).glow, 0.001f)
    }

    @Test
    fun `Classic motionScale is 1_3 (cinematic)`() {
        assertEquals(1.3f, skinTokens(Skin.CLASSIC).motionScale, 0.001f)
    }

    // ── 3. Quiet tokens — flat, no glass ──────────────────────────────────────

    @Test
    fun `Quiet material is FLAT`() {
        assertEquals(SkinMaterial.FLAT, skinTokens(Skin.QUIET).material)
    }

    @Test
    fun `Quiet glassBlurDp is 0 (no blur for flat material)`() {
        assertEquals(0f, skinTokens(Skin.QUIET).glassBlurDp.value, 0.01f)
    }

    @Test
    fun `Quiet fillAlpha is 1_0 (opaque)`() {
        assertEquals(1.0f, skinTokens(Skin.QUIET).fillAlpha, 0.001f)
    }

    @Test
    fun `Quiet sheen is 0`() {
        assertEquals(0f, skinTokens(Skin.QUIET).sheen, 0.001f)
    }

    @Test
    fun `Quiet tintAlpha is 0`() {
        assertEquals(0f, skinTokens(Skin.QUIET).tintAlpha, 0.001f)
    }

    @Test
    fun `Quiet elevation is NONE`() {
        assertEquals(SkinElevation.NONE, skinTokens(Skin.QUIET).elevation)
    }

    @Test
    fun `Quiet shadowCard is NONE`() {
        assertEquals(SkinShadowCard.NONE, skinTokens(Skin.QUIET).shadowCard)
    }

    @Test
    fun `Quiet shadowFloat is E1`() {
        assertEquals(SkinShadowFloat.E1, skinTokens(Skin.QUIET).shadowFloat)
    }

    @Test
    fun `Quiet radiusControl is 7dp`() {
        assertEquals(7f, skinTokens(Skin.QUIET).radiusControl.value, 0.01f)
    }

    @Test
    fun `Quiet radiusCard is 10dp`() {
        assertEquals(10f, skinTokens(Skin.QUIET).radiusCard.value, 0.01f)
    }

    @Test
    fun `Quiet radiusModal is 12dp`() {
        assertEquals(12f, skinTokens(Skin.QUIET).radiusModal.value, 0.01f)
    }

    @Test
    fun `Quiet glow is 0`() {
        assertEquals(0f, skinTokens(Skin.QUIET).glow, 0.001f)
    }

    @Test
    fun `Quiet motionScale is 1_0 (balanced)`() {
        assertEquals(1.0f, skinTokens(Skin.QUIET).motionScale, 0.001f)
    }

    @Test
    fun `Quiet rowTreatment is LINE`() {
        assertEquals(SkinRowTreatment.LINE, skinTokens(Skin.QUIET).rowTreatment)
    }

    @Test
    fun `Quiet background is FLAT`() {
        assertEquals(SkinBackground.FLAT, skinTokens(Skin.QUIET).background)
    }

    // ── 4. Vapor tokens — refined glass ────────────────────────────────────────

    @Test
    fun `Vapor material is GLASS`() {
        assertEquals(SkinMaterial.GLASS, skinTokens(Skin.VAPOR).material)
    }

    @Test
    fun `Vapor glassBlurDp is 34 (higher than Classic 28)`() {
        val vaporBlur = skinTokens(Skin.VAPOR).glassBlurDp.value
        assertEquals(34f, vaporBlur, 0.01f)
        assertTrue("Vapor blur must exceed Classic blur", vaporBlur > skinTokens(Skin.CLASSIC).glassBlurDp.value)
    }

    @Test
    fun `Vapor saturation is 1_7 (higher than Classic 1_45)`() {
        val vaporSat = skinTokens(Skin.VAPOR).saturation
        assertEquals(1.7f, vaporSat, 0.001f)
        assertTrue("Vapor saturation must exceed Classic", vaporSat > skinTokens(Skin.CLASSIC).saturation)
    }

    @Test
    fun `Vapor fillAlpha is 0_50 (more transparent than Classic)`() {
        val vaporFill = skinTokens(Skin.VAPOR).fillAlpha
        assertEquals(0.50f, vaporFill, 0.001f)
        assertTrue("Vapor fillAlpha must be less than Classic", vaporFill < skinTokens(Skin.CLASSIC).fillAlpha)
    }

    @Test
    fun `Vapor sheen dark default is 0_16`() {
        // tok.sheen stores the dark default; light (.70) is handled in Components.kt.
        assertEquals(0.16f, skinTokens(Skin.VAPOR).sheen, 0.001f)
    }

    @Test
    fun `Vapor tintAlpha is 0_14 (accent wash)`() {
        assertEquals(0.14f, skinTokens(Skin.VAPOR).tintAlpha, 0.001f)
    }

    @Test
    fun `Vapor elevation is GLASS_FLOAT`() {
        assertEquals(SkinElevation.GLASS_FLOAT, skinTokens(Skin.VAPOR).elevation)
    }

    @Test
    fun `Vapor shadowCard is NONE (sheen provides definition instead)`() {
        assertEquals(SkinShadowCard.NONE, skinTokens(Skin.VAPOR).shadowCard)
    }

    @Test
    fun `Vapor shadowFloat is E3 (deep float shadow)`() {
        assertEquals(SkinShadowFloat.E3, skinTokens(Skin.VAPOR).shadowFloat)
    }

    @Test
    fun `Vapor radiusControl is 12dp (larger than Classic 9dp)`() {
        val vaporCtl = skinTokens(Skin.VAPOR).radiusControl.value
        assertEquals(12f, vaporCtl, 0.01f)
        assertTrue("Vapor radiusControl must exceed Classic", vaporCtl > skinTokens(Skin.CLASSIC).radiusControl.value)
    }

    @Test
    fun `Vapor radiusCard is 16dp`() {
        assertEquals(16f, skinTokens(Skin.VAPOR).radiusCard.value, 0.01f)
    }

    @Test
    fun `Vapor radiusModal is 16dp (matches Classic)`() {
        assertEquals(16f, skinTokens(Skin.VAPOR).radiusModal.value, 0.01f)
    }

    @Test
    fun `Vapor glow is 0_45`() {
        assertEquals(0.45f, skinTokens(Skin.VAPOR).glow, 0.001f)
    }

    @Test
    fun `Vapor rowTreatment is INSET`() {
        assertEquals(SkinRowTreatment.INSET, skinTokens(Skin.VAPOR).rowTreatment)
    }

    @Test
    fun `Vapor rowGap is 3dp`() {
        assertEquals(3f, skinTokens(Skin.VAPOR).rowGap.value, 0.01f)
    }

    @Test
    fun `Vapor background is TINT_BLOB`() {
        assertEquals(SkinBackground.TINT_BLOB, skinTokens(Skin.VAPOR).background)
    }

    // ── 5. Component mapping logic (mirrors A-F4 Components.kt logic) ──────────

    /**
     * Mirrors the CopyPasteCard shadow logic in Components.kt A-F4:
     *   showCardShadow = translucent && tok.shadowCard == SkinShadowCard.E2
     */
    @Test
    fun `Classic card shows shadow when translucent (E2 shadowCard)`() {
        val tok = skinTokens(Skin.CLASSIC)
        val translucent = true
        val showShadow = translucent && tok.shadowCard == SkinShadowCard.E2
        assertTrue("Classic card must show shadow when translucent", showShadow)
    }

    @Test
    fun `Quiet card never shows card shadow (NONE shadowCard)`() {
        val tok = skinTokens(Skin.QUIET)
        val showShadow = true && tok.shadowCard == SkinShadowCard.E2
        assertFalse("Quiet card must never show card shadow", showShadow)
    }

    @Test
    fun `Vapor card never shows card shadow (NONE shadowCard)`() {
        val tok = skinTokens(Skin.VAPOR)
        val showShadow = true && tok.shadowCard == SkinShadowCard.E2
        assertFalse("Vapor card must never show card shadow", showShadow)
    }

    /**
     * Mirrors the CopyPasteTopBar / GlassAlertDialog shadow logic:
     *   showFloatShadow = translucent && tok.elevation == SkinElevation.GLASS_FLOAT
     */
    @Test
    fun `Classic and Vapor show float shadow (GLASS_FLOAT elevation)`() {
        listOf(Skin.CLASSIC, Skin.VAPOR).forEach { skin ->
            val tok = skinTokens(skin)
            val showShadow = true && tok.elevation == SkinElevation.GLASS_FLOAT
            assertTrue("$skin must show float shadow", showShadow)
        }
    }

    @Test
    fun `Quiet omits float shadow (NONE elevation)`() {
        val tok = skinTokens(Skin.QUIET)
        val showShadow = true && tok.elevation == SkinElevation.GLASS_FLOAT
        assertFalse("Quiet must not show float shadow (NONE elevation)", showShadow)
    }

    /**
     * Mirrors the LiquidGlassSurface FLAT-material gate in Components.kt A-F4:
     *   effectiveTranslucent = translucent && tok.material == SkinMaterial.GLASS
     */
    @Test
    fun `Quiet surface is always non-translucent regardless of user pref (FLAT material)`() {
        val tok = skinTokens(Skin.QUIET)
        val userPrefTranslucent = true
        val effectiveTranslucent = userPrefTranslucent && tok.material == SkinMaterial.GLASS
        assertFalse("Quiet material=FLAT must produce effectiveTranslucent=false", effectiveTranslucent)
    }

    @Test
    fun `Classic and Vapor respect user translucency pref (GLASS material)`() {
        listOf(Skin.CLASSIC, Skin.VAPOR).forEach { skin ->
            val tok = skinTokens(skin)
            val userPrefTranslucent = true
            val effectiveTranslucent = userPrefTranslucent && tok.material == SkinMaterial.GLASS
            assertTrue("$skin with translucency on must be effectively translucent", effectiveTranslucent)
        }
    }

    // ── 6. Invariants across all skins ────────────────────────────────────────

    @Test
    fun `glow is in 0_to_1 range for all skins`() {
        Skin.entries.forEach { skin ->
            val glow = skinTokens(skin).glow
            assertTrue("$skin glow must be in [0, 1]", glow in 0f..1f)
        }
    }

    @Test
    fun `motionScale is positive for all skins`() {
        Skin.entries.forEach { skin ->
            assertTrue("$skin motionScale must be > 0", skinTokens(skin).motionScale > 0f)
        }
    }

    @Test
    fun `all radius tokens are positive dp values`() {
        Skin.entries.forEach { skin ->
            val tok = skinTokens(skin)
            assertTrue("$skin radiusControl must be positive", tok.radiusControl.value > 0f)
            assertTrue("$skin radiusChip must be positive", tok.radiusChip.value > 0f)
            assertTrue("$skin radiusCard must be positive", tok.radiusCard.value > 0f)
            assertTrue("$skin radiusModal must be positive", tok.radiusModal.value > 0f)
        }
    }

    @Test
    fun `fillAlpha is in 0_to_1 range for all skins`() {
        Skin.entries.forEach { skin ->
            val alpha = skinTokens(skin).fillAlpha
            assertTrue("$skin fillAlpha must be in [0, 1]", alpha in 0f..1f)
        }
    }

    @Test
    fun `sheen is in 0_to_1 range for all skins`() {
        Skin.entries.forEach { skin ->
            val sheen = skinTokens(skin).sheen
            assertTrue("$skin sheen must be in [0, 1]", sheen in 0f..1f)
        }
    }

    @Test
    fun `tintAlpha is in 0_to_1 range for all skins`() {
        Skin.entries.forEach { skin ->
            val tint = skinTokens(skin).tintAlpha
            assertTrue("$skin tintAlpha must be in [0, 1]", tint in 0f..1f)
        }
    }

    // ── 7. CopyPaste-0kbq: sheenLight token ───────────────────────────────────

    @Test
    fun `Classic sheenLight is 0_45 (light specular for Classic surface)`() {
        // Classic light sheen was hardcoded 0.45f in Components.kt; now a token.
        assertEquals(0.45f, skinTokens(Skin.CLASSIC).sheenLight, 0.001f)
    }

    @Test
    fun `Quiet sheenLight is 0 (flat skin has no sheen)`() {
        assertEquals(0f, skinTokens(Skin.QUIET).sheenLight, 0.001f)
    }

    @Test
    fun `Vapor sheenLight is 0_70 (bright light specular for Vapor surface)`() {
        // Vapor light sheen was hardcoded 0.70f in Components.kt; now a token.
        assertEquals(0.70f, skinTokens(Skin.VAPOR).sheenLight, 0.001f)
    }

    @Test
    fun `sheenLight is in 0_to_1 range for all skins`() {
        Skin.entries.forEach { skin ->
            val sl = skinTokens(skin).sheenLight
            assertTrue("$skin sheenLight must be in [0, 1]", sl in 0f..1f)
        }
    }

    @Test
    fun `sheenLight is_gte sheen for all skins (light sheen never less than dark sheen)`() {
        // Design invariant: light sheen is always at least as bright as dark sheen.
        Skin.entries.forEach { skin ->
            val tok = skinTokens(skin)
            assertTrue(
                "$skin sheenLight (${tok.sheenLight}) must be >= sheen (${tok.sheen})",
                tok.sheenLight >= tok.sheen,
            )
        }
    }

    // ── 8. CopyPaste-fuxf: glassBlurStrongDp token ────────────────────────────

    @Test
    fun `Classic glassBlurStrongDp is 40dp (mirrors web glassBlurStrong)`() {
        // Mirrors GlassTier.STRONG.blur = 40.dp and web glassBlurStrong.
        assertEquals(40f, skinTokens(Skin.CLASSIC).glassBlurStrongDp.value, 0.01f)
    }

    @Test
    fun `Quiet glassBlurStrongDp is 0dp (flat skin has no blur)`() {
        assertEquals(0f, skinTokens(Skin.QUIET).glassBlurStrongDp.value, 0.01f)
    }

    @Test
    fun `Vapor glassBlurStrongDp is 44dp (boosted strong blur for Vapor)`() {
        assertEquals(44f, skinTokens(Skin.VAPOR).glassBlurStrongDp.value, 0.01f)
    }

    @Test
    fun `glassBlurStrongDp is_gte glassBlurDp for all skins (strong blur at least as large as base blur)`() {
        // Design invariant: strong/floating blur should be at least as large as base blur.
        Skin.entries.forEach { skin ->
            val tok = skinTokens(skin)
            assertTrue(
                "$skin glassBlurStrongDp (${tok.glassBlurStrongDp}) must be >= glassBlurDp (${tok.glassBlurDp})",
                tok.glassBlurStrongDp.value >= tok.glassBlurDp.value,
            )
        }
    }
}
