package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class AndroidCaptureCommandsTest {
    @Before
    fun resetForegroundCapturePreference() {
        AndroidCaptureState.setForegroundCaptureEnabled(RuntimeEnvironment.getApplication(), false)
    }

    @Test
    fun refusedForegroundAdmissionRestoresThePreviousPreference() {
        val context = RuntimeEnvironment.getApplication()
        var completed: Boolean? = null

        applyForegroundCapturePreference(
            context,
            true,
            refresh = { completion -> completion(false) },
            complete = { completed = it },
        )

        assertEquals(false, completed)
        assertFalse(AndroidCaptureState.foregroundCaptureEnabled(context))
    }

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
            captureGrantCommands(packageName).map { "adb shell ${it.joinToString(" ")}" },
            adbCaptureGrantCommands(packageName),
        )
    }
}
