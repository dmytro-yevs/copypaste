package com.copypaste.app

import org.junit.Assert.*
import org.junit.Test

class ClipboardFocusCaptureGuardTest {
    @Test fun unchangedClipboardIsSkippedOnlyOnThePickerReturn() {
        val guard = ClipboardFocusCaptureGuard()
        guard.beginFilePicker(123)
        assertTrue(guard.hasPendingPicker)
        assertFalse(guard.shouldCaptureAfterFocus(123))
        assertFalse(guard.hasPendingPicker)
        assertTrue(guard.shouldCaptureAfterFocus(123))
    }

    @Test fun aNewCopyDuringThePickerIsStillCaptured() {
        val guard = ClipboardFocusCaptureGuard()
        guard.beginFilePicker(123)
        assertTrue(guard.shouldCaptureAfterFocus(124))
    }

    @Test fun unavailableGenerationAndFailedPickerNeverLoseACopy() {
        val guard = ClipboardFocusCaptureGuard()
        guard.beginFilePicker(123)
        assertTrue(guard.shouldCaptureAfterFocus(null))
        guard.beginFilePicker(null)
        assertTrue(guard.shouldCaptureAfterFocus(123))
        guard.beginFilePicker(123)
        guard.abandonFilePicker()
        assertTrue(guard.shouldCaptureAfterFocus(123))
    }
}
