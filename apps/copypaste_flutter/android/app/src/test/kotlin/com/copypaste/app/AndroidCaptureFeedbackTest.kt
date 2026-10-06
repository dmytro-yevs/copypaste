package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidCaptureFeedbackTest {
    @Test
    fun previewNormalizesNewlinesAndTabs() {
        assertEquals("a⏎b⇥c", AndroidCaptureFeedback.textPreview("a\r\nb\tc"))
    }

    @Test
    fun previewDoesNotSplitUnicodeCharacters() {
        assertEquals("😀".repeat(1000) + "…", AndroidCaptureFeedback.textPreview("😀".repeat(1001)))
    }
}
