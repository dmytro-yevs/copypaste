package com.copypaste.android.ui.shell

import com.copypaste.android.R
import com.copypaste.android.ui.theme.icons.LucideIcons
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * android-navigation-chrome "Shell hosts three tabs" scenario: the nav is
 * exactly Clips/Devices/Settings, in that order, each with its cross-platform-
 * parity-pinned Lucide icon (history/monitor-smartphone/settings-2). Referenced
 * by `GeneralTab.kt`'s comments explaining why About/Logs route via Settings
 * instead of getting a fourth/fifth tab.
 */
class NavTabTest {

    @Test
    fun `exactly three tabs in Clips, Devices, Settings order`() {
        assertEquals(
            listOf(NavTab.CLIPS, NavTab.DEVICES, NavTab.SETTINGS),
            NavTab.entries,
        )
    }

    @Test
    fun `each tab has its parity-pinned label resource`() {
        assertEquals(R.string.title_history, NavTab.CLIPS.labelRes)
        assertEquals(R.string.title_devices, NavTab.DEVICES.labelRes)
        assertEquals(R.string.title_settings, NavTab.SETTINGS.labelRes)
    }

    @Test
    fun `each tab has its parity-pinned Lucide icon`() {
        assertEquals(LucideIcons.NavHistory, NavTab.CLIPS.icon)
        assertEquals(LucideIcons.NavDevices, NavTab.DEVICES.icon)
        assertEquals(LucideIcons.NavSettings, NavTab.SETTINGS.icon)
    }
}
