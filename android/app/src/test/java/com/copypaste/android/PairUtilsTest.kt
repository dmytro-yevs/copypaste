package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * JVM unit tests for the pure (non-Android, non-FFI) helpers in PairActivity.
 *
 * The camera scanner and preview orientation are inherently device-side and are
 * NOT covered here — they require a real device/emulator with a camera.
 */
class PairUtilsTest {
    @Test
    fun formatScannedInfo_includesNameAndFingerprint() {
        assertEquals("Pixel 8 (abc123)", formatScannedInfo("Pixel 8", "abc123"))
    }

    @Test
    fun formatScannedInfo_blankNameFallsBackToDevice() {
        assertEquals("device (abc123)", formatScannedInfo("", "abc123"))
    }

    @Test
    fun formatScannedInfo_whitespaceNameFallsBackToDevice() {
        assertEquals("device (fp)", formatScannedInfo("   ", "fp"))
    }
}
