package com.copypaste.android.ui.theme

import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * android-design-system frozen `CpDimensions`/`CpSpacing`/`CpShapes`/`CpElevation`
 * value-inspection tests (task 1.9) — the tables are normative (no ranges), so
 * every value is asserted exactly.
 */
class CpDimensionsTest {

    @Test
    fun `CpShapes fixed radii are constant per STYLEGUIDE section 5`() {
        assertEquals(7.dp, CpShapes.chip)
        assertEquals(8.dp, CpShapes.ctl)
        assertEquals(9.dp, CpShapes.input)
        assertEquals(13.dp, CpShapes.card)
        assertEquals(999.dp, CpShapes.pill)
    }

    @Test
    fun `CpSpacing matches the tokens css 2 4 6 8 11 14 16 20 24 scale`() {
        assertEquals(2.dp, CpSpacing.s1)
        assertEquals(4.dp, CpSpacing.s2)
        assertEquals(6.dp, CpSpacing.s3)
        assertEquals(8.dp, CpSpacing.s4)
        assertEquals(11.dp, CpSpacing.s5)
        assertEquals(14.dp, CpSpacing.s6)
        assertEquals(16.dp, CpSpacing.s7)
        assertEquals(20.dp, CpSpacing.s8)
        assertEquals(24.dp, CpSpacing.s9)
    }

    @Test
    fun `CpElevation exposes an Android approximation for each shadow tier`() {
        assertEquals(1.dp, CpElevation.sh1)
        assertEquals(8.dp, CpElevation.sh2)
        assertEquals(24.dp, CpElevation.sh3)
    }

    @Test
    fun `CpDimensions component geometry matches the frozen table exactly`() {
        assertEquals(32.dp, CpDimensions.tileSm)
        assertEquals(36.dp, CpDimensions.tileMd)
        assertEquals(18.dp, CpDimensions.glyphBox)
        assertEquals(24.dp, CpDimensions.navGlyph)
        assertEquals(20.dp, CpDimensions.iconMeta)
        assertEquals(38.dp, CpDimensions.toggleW)
        assertEquals(22.dp, CpDimensions.toggleH)
        assertEquals(18.dp, CpDimensions.toggleKnob)
        assertEquals(50.dp, CpDimensions.navPillW)
        assertEquals(38.dp, CpDimensions.navPillH)
        assertEquals(220.dp, CpDimensions.qr)
        assertEquals(16.dp, CpDimensions.qrQuietZone)
        assertEquals(44.dp, CpDimensions.sasCell)
        assertEquals(48.dp, CpDimensions.touchMin)
        assertEquals(12.dp, CpDimensions.navBottomClearance)
        assertEquals(600.dp, CpDimensions.widthCompactMax)
        assertEquals(840.dp, CpDimensions.widthMediumMax)
    }
}
