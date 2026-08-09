package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Test

class ShizukuSettingsServiceTest {
    @Test
    fun suppressionUsesFixedSettingArguments() {
        assertEquals(
            listOf(
                "settings",
                "put",
                "secure",
                "clipboard_show_access_notifications",
                "0",
            ),
            clipboardNotificationCommand(true),
        )
    }

    @Test
    fun restoringNotificationsUsesOne() {
        assertEquals("1", clipboardNotificationCommand(false).last())
    }
}
