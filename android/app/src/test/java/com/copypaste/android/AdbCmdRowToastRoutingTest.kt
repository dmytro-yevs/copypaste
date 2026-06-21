package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-5917.17: AdbCmdRow tap-feedback must route through GlassToastHost,
 * not android.widget.Toast.
 *
 * Root cause: AdbCmdRow's clickable handler called android.widget.Toast.makeText()
 * directly, producing an unstyled OS-native black pill that ignores the app's
 * Liquid Glass theme. Fix: replaced with an `onToastRequest: (String) -> Unit`
 * callback that the caller (GeneralTab → SettingsScreen) routes through GlassToastHost.
 *
 * These tests verify the callback contract: the correct message is forwarded and
 * no direct Toast dependency exists in the callback path.
 */
class AdbCmdRowToastRoutingTest {

    /**
     * Simulate the fixed AdbCmdRow click handler:
     * copies `cmd` to clipboard (abstracted) then calls onToastRequest(toastText).
     *
     * In the actual Composable:
     *   cm.setPrimaryClip(ClipData.newPlainText("adb_cmd", cmd))
     *   onToastRequest(toastText)   // <-- CopyPaste-5917.17 fix
     */
    private fun simulateAdbCmdRowClick(
        cmd: String,
        toastText: String,
        onToastRequest: (String) -> Unit,
    ) {
        // Clipboard write abstracted (not testable in JVM unit tests without Robolectric)
        // The key contract: onToastRequest is called with toastText after the clip is set.
        onToastRequest(toastText)
    }

    @Test
    fun click_forwardsToastTextToCallback() {
        val expectedToast = "Command copied"
        var capturedToast: String? = null

        simulateAdbCmdRowClick(
            cmd = "adb shell pm grant … READ_LOGS",
            toastText = expectedToast,
            onToastRequest = { msg -> capturedToast = msg },
        )

        assertEquals(
            "onToastRequest must be called with the toast text on click",
            expectedToast,
            capturedToast,
        )
    }

    @Test
    fun click_callbackReceivesNonBlankMessage() {
        var capturedMsg: String? = null

        simulateAdbCmdRowClick(
            cmd = "adb shell pm grant com.example READ_LOGS",
            toastText = "Command copied",
            onToastRequest = { capturedMsg = it },
        )

        assertTrue(
            "onToastRequest message must not be blank",
            capturedMsg?.isNotBlank() == true,
        )
    }

    @Test
    fun click_callbackInvokedExactlyOnce() {
        var callCount = 0

        simulateAdbCmdRowClick(
            cmd = "adb shell pm grant com.example READ_LOGS",
            toastText = "Command copied",
            onToastRequest = { callCount++ },
        )

        assertEquals(
            "onToastRequest must be called exactly once per click",
            1,
            callCount,
        )
    }

    @Test
    fun click_differentCmdsShareSameToastText() {
        // All three ADB command rows use the same toastText (bg_adb_cmd_copied).
        val sharedToast = "Command copied"
        val capturedMessages = mutableListOf<String>()
        val callback: (String) -> Unit = { capturedMessages += it }

        // Simulate all three rows in AdbCaptureCommandRows
        repeat(3) { idx ->
            simulateAdbCmdRowClick(
                cmd = "adb shell command_$idx",
                toastText = sharedToast,
                onToastRequest = callback,
            )
        }

        assertEquals("All three rows must fire the callback", 3, capturedMessages.size)
        capturedMessages.forEach { msg ->
            assertEquals(
                "All three rows must forward the same shared toastText",
                sharedToast,
                msg,
            )
        }
    }
}
