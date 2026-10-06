package com.copypaste.app

import android.app.Activity
import android.app.Application
import com.google.mlkit.common.MlKitException
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.util.ReflectionHelpers
import java.lang.reflect.Proxy

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36], manifest = Config.NONE, application = Application::class)
class PairingScannerChannelTest {
    @Test
    fun scannerCloseCompletesAsCancellationExactlyOnce() {
        val (channel, result) = pendingScan()
        fail(channel, MlKitException("Scanner closed", MlKitException.CODE_SCANNER_CANCELLED))
        channel.dispose()
        assertEquals(1, result.successes)
        assertNull(result.value)
        assertNull(result.errorCode)
    }

    @Test
    fun realScannerFailureRemainsAnError() {
        val (channel, result) = pendingScan()
        fail(channel, MlKitException("Scanner unavailable", MlKitException.CODE_SCANNER_UNAVAILABLE))
        channel.dispose()
        assertEquals(0, result.successes)
        assertEquals("scanner_unavailable", result.errorCode)
    }

    private fun pendingScan(): Pair<PairingScannerChannel, ScanResult> {
        val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        val messenger = Proxy.newProxyInstance(
            BinaryMessenger::class.java.classLoader,
            arrayOf(BinaryMessenger::class.java),
        ) { _, _, _ -> null } as BinaryMessenger
        val channel = PairingScannerChannel(activity, messenger)
        val result = ScanResult()
        ReflectionHelpers.setField(channel, "pending", result)
        return channel to result
    }

    private fun fail(channel: PairingScannerChannel, error: Exception) {
        ReflectionHelpers.callInstanceMethod<Void>(
            channel,
            "scannerFailed",
            ReflectionHelpers.ClassParameter.from(Exception::class.java, error),
        )
    }

    private class ScanResult : MethodChannel.Result {
        var successes = 0
        var value: Any? = null
        var errorCode: String? = null

        override fun success(result: Any?) {
            successes++
            value = result
        }

        override fun error(code: String, message: String?, details: Any?) {
            errorCode = code
        }

        override fun notImplemented() = fail("Unexpected method")
    }
}
