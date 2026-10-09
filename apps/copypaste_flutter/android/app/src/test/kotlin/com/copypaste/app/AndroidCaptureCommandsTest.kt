package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidCaptureCommandsTest {
    @Test
    fun fullGrantSetMatchesShizukuAndAdbSetup() {
        val packageName = "com.copypaste.app"

        assertEquals(
            listOf(
                listOf("pm", "grant", packageName, "android.permission.READ_LOGS"),
                listOf("cmd", "appops", "set", packageName, "SYSTEM_ALERT_WINDOW", "allow"),
                listOf("cmd", "appops", "set", packageName, "RUN_IN_BACKGROUND", "allow"),
                listOf("cmd", "appops", "set", packageName, "RUN_ANY_IN_BACKGROUND", "allow"),
                listOf("am", "set-inactive", packageName, "false"),
                listOf("am", "set-standby-bucket", packageName, "active"),
            ),
            captureGrantCommands(packageName),
        )
        assertEquals(
            (captureGrantCommands(packageName) + screenshotSourceGrantCommands(packageName)).map { "adb shell ${it.joinToString(" ")}" },
            adbCaptureGrantCommands(packageName),
        )
    }
}
