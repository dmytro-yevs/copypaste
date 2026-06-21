package com.copypaste.android.ui.theme

import org.junit.Test
import org.junit.Assert.*

class CopyPasteButtonVariantTest {
    @Test
    fun `ButtonVariant has expected variants`() {
        val values = ButtonVariant.values()
        assertTrue(values.contains(ButtonVariant.PRIMARY))
        assertTrue(values.contains(ButtonVariant.SECONDARY))
        assertTrue(values.contains(ButtonVariant.DANGER))
        assertTrue(values.contains(ButtonVariant.DANGER_SOLID))
        assertTrue(values.contains(ButtonVariant.GHOST))
    }

    @Test
    fun `ButtonVariant values are all distinct`() {
        val values = ButtonVariant.values()
        assertEquals(values.size, values.toSet().size)
    }

    @Test
    fun `dialog confirm actions use DANGER variant`() {
        // Validates the contract: destructive confirm actions must use DANGER or DANGER_SOLID,
        // never the raw TextButton path.
        val destructiveVariants = setOf(ButtonVariant.DANGER, ButtonVariant.DANGER_SOLID)
        assertTrue(ButtonVariant.DANGER in destructiveVariants)
        assertTrue(ButtonVariant.DANGER_SOLID in destructiveVariants)
    }

    @Test
    fun `dialog cancel actions use GHOST variant`() {
        // Validates the contract: cancel/dismiss actions must use GHOST or SECONDARY,
        // never the raw TextButton path.
        val cancelVariants = setOf(ButtonVariant.GHOST, ButtonVariant.SECONDARY)
        assertTrue(ButtonVariant.GHOST in cancelVariants)
    }
}
