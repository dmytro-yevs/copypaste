package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * Verifies the bottom-navigation tab set after the About tab was moved into
 * Settings → General. The nav bar must contain exactly three tabs (History,
 * Pair, Settings) and must NOT contain About or Logs entries — both are
 * reachable via the Settings screen with NavIcons.About / NavIcons.Logs icons
 * (CopyPaste-5917.77). This is an intentional Android/macOS routing difference:
 * macOS uses a 5-tab nav (History, Devices, Settings, About, Logs) while
 * Android routes About and Logs through Settings to keep the bottom bar compact.
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
        assertFalse("NavTab.ABOUT must not exist; About is reachable via Settings (CopyPaste-5917.77)", hasAbout)
    }

    @Test
    fun `bottom nav does not contain a Logs tab`() {
        // CopyPaste-5917.77: Logs uses NavIcons.Logs in Settings → Diagnostics row,
        // not a bottom-nav tab. Intentional difference from macOS 5-tab nav.
        val hasLogs = NavTab.entries.any { it.name == "LOGS" }
        assertFalse("NavTab.LOGS must not exist; Logs is reachable via Settings (CopyPaste-5917.77)", hasLogs)
    }

    @Test
    fun `bottom nav tabs are Clips Devices Settings in that order`() {
        val names = NavTab.entries.map { it.name }
        assertEquals(listOf("CLIPS", "DEVICES", "SETTINGS"), names)
    }
}
