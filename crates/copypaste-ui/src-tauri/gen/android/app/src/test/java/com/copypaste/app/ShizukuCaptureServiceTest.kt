package com.copypaste.app

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ShizukuCaptureServiceTest {
    @Test
    fun forwardsOnlyClipboardServiceEventsForThisPackage() {
        assertTrue(
            isClipboardAccessEvent(
                "09-27 12:00:00.000 E ClipboardService: read com.copypaste.app",
                "com.copypaste.app",
            ),
        )
        assertFalse(isClipboardAccessEvent("ClipboardService: read com.other.app", "com.copypaste.app"))
        assertFalse(isClipboardAccessEvent("ActivityTaskManager: com.copypaste.app", "com.copypaste.app"))
    }
}
