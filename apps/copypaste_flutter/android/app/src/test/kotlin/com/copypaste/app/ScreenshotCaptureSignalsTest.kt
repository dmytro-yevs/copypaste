package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Test

class ScreenshotCaptureSignalsTest {
    @Test fun listenersReceiveChangesOnlyWhileRegistered() {
        var calls = 0
        val listener: () -> Unit = { calls++ }
        ScreenshotCaptureSignals.add(listener)
        try {
            ScreenshotCaptureSignals.changed()
            assertEquals(1, calls)
        } finally {
            ScreenshotCaptureSignals.remove(listener)
        }
        ScreenshotCaptureSignals.changed()
        assertEquals(1, calls)
    }
}
