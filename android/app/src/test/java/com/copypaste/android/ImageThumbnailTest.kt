package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for [ImageThumbnailUtils.scaledDimensions]:
 * verifies the never-upscale, aspect-preserving thumbnail scaling logic.
 *
 * No Android SDK required — [ImageThumbnailUtils.scaledDimensions] is a pure
 * Kotlin function that can run on the JVM test runner.
 */
class ImageThumbnailTest {

    @Test
    fun landscapeWider_scalesDownToMaxDim() {
        // 1360x680 → fits in 680 max dim → 680x340
        val (w, h) = ImageThumbnailUtils.scaledDimensions(1360, 680, maxDim = 680)
        assertEquals(680, w)
        assertEquals(340, h)
    }

    @Test
    fun portraitTaller_scalesDownToMaxDim() {
        // 340x1360 → fits in 680 max dim → 170x680
        val (w, h) = ImageThumbnailUtils.scaledDimensions(340, 1360, maxDim = 680)
        assertEquals(170, w)
        assertEquals(680, h)
    }

    @Test
    fun square_scalesDown() {
        // 1000x1000 → 680x680
        val (w, h) = ImageThumbnailUtils.scaledDimensions(1000, 1000, maxDim = 680)
        assertEquals(680, w)
        assertEquals(680, h)
    }

    @Test
    fun smallImage_isNeverUpscaled() {
        // 100x50 → already within 680, must stay 100x50
        val (w, h) = ImageThumbnailUtils.scaledDimensions(100, 50, maxDim = 680)
        assertEquals(100, w)
        assertEquals(50, h)
    }

    @Test
    fun exactlyAtMaxDim_isUnchanged() {
        // 680x480 → max dim is 680, width is already exactly 680 → unchanged
        val (w, h) = ImageThumbnailUtils.scaledDimensions(680, 480, maxDim = 680)
        assertEquals(680, w)
        assertEquals(480, h)
    }

    @Test
    fun aspectRatioIsPreserved_landscape() {
        // 2000x1000 → 680x340, ratio = 2.0
        val (w, h) = ImageThumbnailUtils.scaledDimensions(2000, 1000, maxDim = 680)
        val ratio = w.toDouble() / h.toDouble()
        assertTrue("aspect ratio must be preserved (expected ~2.0, got $ratio)", ratio in 1.99..2.01)
    }

    @Test
    fun aspectRatioIsPreserved_portrait() {
        // 1000x2000 → 340x680, ratio = 0.5
        val (w, h) = ImageThumbnailUtils.scaledDimensions(1000, 2000, maxDim = 680)
        val ratio = w.toDouble() / h.toDouble()
        assertTrue("aspect ratio must be preserved (expected ~0.5, got $ratio)", ratio in 0.49..0.51)
    }

    @Test
    fun zeroWidth_returnsOneByOne() {
        // Degenerate input should not crash and should produce at least 1x1
        val (w, h) = ImageThumbnailUtils.scaledDimensions(0, 100, maxDim = 680)
        assertTrue("width must be >= 1", w >= 1)
        assertTrue("height must be >= 1", h >= 1)
    }

    @Test
    fun zeroHeight_returnsOneByOne() {
        val (w, h) = ImageThumbnailUtils.scaledDimensions(100, 0, maxDim = 680)
        assertTrue("width must be >= 1", w >= 1)
        assertTrue("height must be >= 1", h >= 1)
    }
}
