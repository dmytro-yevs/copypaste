package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Test

class ShizukuSettingsServiceTest {
    @Test
    fun clipCascadeGrantCommandsMatchThePublishedSetup() {
        assertEquals(
            listOf(
                listOf("pm", "grant", "com.copypaste.app", "android.permission.READ_LOGS"),
                listOf("cmd", "appops", "set", "com.copypaste.app", "SYSTEM_ALERT_WINDOW", "allow"),
            ),
            clipCascadeGrantCommands("com.copypaste.app"),
        )
    }

    @Test
    fun clipCascadeRefreshCommandMatchesThePublishedSetup() {
        assertEquals(
            listOf("am", "force-stop", "com.copypaste.app"),
            clipCascadeRefreshCommand("com.copypaste.app"),
        )
    }

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

    @Test
    fun persistentCaptureStateCommandsMatchClipCascadePlusResidencyState() {
        val commands = persistentCaptureStateCommands("com.copypaste.app")
        assertEquals(
            listOf(
                listOf("pm", "grant", "com.copypaste.app", "android.permission.READ_LOGS"),
                listOf("cmd", "appops", "set", "com.copypaste.app", "SYSTEM_ALERT_WINDOW", "allow"),
                listOf("cmd", "appops", "set", "com.copypaste.app", "RUN_IN_BACKGROUND", "allow"),
                listOf("cmd", "appops", "set", "com.copypaste.app", "RUN_ANY_IN_BACKGROUND", "allow"),
                listOf("am", "set-inactive", "com.copypaste.app", "false"),
                listOf("am", "set-standby-bucket", "com.copypaste.app", "active"),
            ),
            commands,
        )
        commands.flatten().forEach { token ->
            org.junit.Assert.assertFalse(token.contains("READ_CLIPBOARD_IN_BACKGROUND"))
            org.junit.Assert.assertFalse(token.contains("READ_CLIPBOARD"))
        }
    }
}
