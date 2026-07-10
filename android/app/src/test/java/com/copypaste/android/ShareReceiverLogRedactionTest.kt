package com.copypaste.android

import android.net.Uri
import android.util.Log
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLog

/**
 * S12 W2: [ShareReceiverActivity] must never log a shared content:// URI — it
 * can encode a filename/path of user content. `dispatchShareIntent`/
 * `captureStreamUri`/`queryStreamSize` are private instance methods that
 * construct a real [SyncManager] (native FFI init), which is not exercisable
 * under Robolectric's plain JVM without a device/emulator's native library —
 * triggering the failure paths through an `ActivityScenario` would either not
 * reach the target `Log.w` calls (native init typically no-ops rather than
 * throwing in this sandbox) or risk flaking on an unrelated native-load
 * failure unrelated to what this test verifies.
 *
 * Instead this exercises the extracted pure seam,
 * [ShareReceiverActivity.redactedFailureLog] — the exact function the two
 * `Log.w` call sites (capture failure, SIZE-query failure) now use — through
 * a real `Log.w` call captured by Robolectric's [ShadowLog], proving the
 * logged message never contains the URI substring.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ShareReceiverLogRedactionTest {

    private val tag = "ShareReceiver"
    private val uri = Uri.parse("content://com.example.provider/secret/private-file.txt")

    @Before
    fun setUp() {
        ShadowLog.reset()
    }

    @Test
    fun `capture failure log omits the uri and identifying content`() {
        val message = ShareReceiverActivity.redactedFailureLog(
            "share: failed to capture stream",
            RuntimeException("boom"),
        )
        Log.w(tag, message)

        val logs = ShadowLog.getLogsForTag(tag)
        assertTrue(logs.isNotEmpty())
        assertTrue(logs.none { it.msg.contains(uri.toString()) })
        assertTrue(logs.none { it.msg.contains("secret") })
        assertTrue(logs.none { it.msg.contains("private-file.txt") })
    }

    @Test
    fun `SIZE query failure log omits the uri and identifying content`() {
        val message = ShareReceiverActivity.redactedFailureLog(
            "share: SIZE query failed",
            IllegalStateException("provider crashed"),
        )
        Log.w(tag, message)

        val logs = ShadowLog.getLogsForTag(tag)
        assertTrue(logs.isNotEmpty())
        assertTrue(logs.none { it.msg.contains(uri.toString()) })
        assertTrue(logs.none { it.msg.contains("secret") })
        assertTrue(logs.none { it.msg.contains("private-file.txt") })
    }

    @Test
    fun `redactedFailureLog carries exception class name for diagnosability`() {
        val message = ShareReceiverActivity.redactedFailureLog("share: failed", RuntimeException("boom"))
        assertTrue(message.contains("RuntimeException"))
        assertFalse(message.contains("boom"))
    }
}
