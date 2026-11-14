package com.copypaste.app.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PanelCaptureTest {
    private val clipboard = FakeClipboard()
    private val capture = PanelCapture(clipboard, whenRuntimeReady = { it() }, onSaved = {})

    @Test fun catchUpCapturesEachClipOnce() {
        clipboard.stamp = 100L
        assertTrue(capture.catchUp())
        assertFalse(capture.catchUp())

        clipboard.stamp = 200L
        assertTrue(capture.catchUp())
        assertEquals(2, clipboard.captures)
    }

    @Test fun ownWritesAndEmptyClipboardsAreNotCaptured() {
        clipboard.stamp = null
        assertFalse(capture.catchUp())
        assertEquals(0, clipboard.captures)
    }

    @Test fun aClipFirstSeenInAPasswordFieldIsNeverCaughtUpLater() {
        clipboard.stamp = 300L
        capture.onFieldChanged(FieldPolicy(password = true, textEditor = true))
        assertFalse(capture.catchUp())

        capture.onFieldChanged(FieldPolicy(textEditor = true))
        assertFalse(capture.catchUp())
        assertEquals(0, clipboard.captures)
    }

    @Test fun collectModeListensOnlyWhileAllowedAndCapturesOutsidePrivateFields() {
        clipboard.background = true
        capture.refreshListening()
        assertFalse(clipboard.listening)

        clipboard.background = false
        capture.refreshListening()
        assertTrue(clipboard.listening)

        capture.onFieldChanged(FieldPolicy(incognito = true, textEditor = true))
        clipboard.stamp = 1L
        capture.onClipChanged()
        assertEquals(0, clipboard.captures)

        capture.onFieldChanged(FieldPolicy.NONE)
        clipboard.stamp = 2L
        capture.onClipChanged()
        assertEquals(1, clipboard.captures)

        clipboard.allowed = false
        capture.refreshListening()
        assertFalse(clipboard.listening)
        capture.release()
        assertTrue(clipboard.released)
    }
}
