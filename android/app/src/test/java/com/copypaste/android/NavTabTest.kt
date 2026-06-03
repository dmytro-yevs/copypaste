package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * Verifies the bottom-navigation tab set after the About tab was moved into
 * Settings → General. The nav bar must contain exactly three tabs (History,
 * Pair, Settings) and must NOT contain an About entry — About is now reachable
 * only via the Settings screen.
 *
 * These are pure-JVM tests; no Android SDK or emulator required.
 */
class NavTabTest {

    @Test
    fun `bottom nav has exactly three tabs`() {
        assertEquals(
            "Expected exactly 3 nav tabs (Clips, Devices, Settings)",
            3,
            NavTab.entries.size
        )
    }

    @Test
    fun `bottom nav does not contain an About tab`() {
        val hasAbout = NavTab.entries.any { it.name == "ABOUT" }
        assertFalse("NavTab.ABOUT must be removed; About is now reachable via Settings", hasAbout)
    }

    @Test
    fun `bottom nav tabs are Clips Devices Settings in that order`() {
        val names = NavTab.entries.map { it.name }
        assertEquals(listOf("CLIPS", "DEVICES", "SETTINGS"), names)
    }
}
