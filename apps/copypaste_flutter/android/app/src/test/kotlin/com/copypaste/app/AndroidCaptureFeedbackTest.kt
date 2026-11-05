package com.copypaste.app

import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidCaptureFeedbackTest {
    @Test
    fun previewNormalizesNewlinesAndTabs() {
        assertEquals("a⏎b⇥c", AndroidCaptureFeedback.textPreview("a\r\nb\tc"))
    }

    @Test
    fun thumbnailSubsamplingKeepsTheThumbnailEdge() {
        assertEquals(1, AndroidCaptureFeedback.thumbnailSampleSize(511, 300))
        assertEquals(2, AndroidCaptureFeedback.thumbnailSampleSize(300, 512))
        assertEquals(8, AndroidCaptureFeedback.thumbnailSampleSize(4032, 3024))
    }

    @Test
    fun previewDoesNotSplitUnicodeCharacters() {
        assertEquals("😀".repeat(1000) + "…", AndroidCaptureFeedback.textPreview("😀".repeat(1001)))
    }
}
