package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
class ShizukuClipboardTest {
    @Test
    @Config(sdk = [33])
    fun api30AndAboveAdvertiseShizukuSupport() {
        assertTrue(ShizukuClipboard.isSupported())
    }

    @Test
    @Config(sdk = [29])
    fun api29DoesNotAdvertiseShizukuSupport() {
        assertFalse(ShizukuClipboard.isSupported())
    }

    @Test
    @Config(sdk = [33])
    fun armFailsClosedWhenShizukuIsAbsent() {
        val armed = ShizukuClipboard.arm(RuntimeEnvironment.getApplication()) {}
        assertFalse(armed)
        assertFalse(ShizukuClipboard.isListening())
        assertEquals("shizuku is not running", ShizukuClipboard.lastFailure)
    }
}
