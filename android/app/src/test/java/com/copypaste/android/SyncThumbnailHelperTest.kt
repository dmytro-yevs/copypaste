package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for [SyncThumbnailHelper].
 *
 * Verifies that thumbnail generation for synced images:
 *   1. Returns false (non-fatal) when the byte array is empty (no bytes to decode).
 *   2. Returns false (non-fatal) when the byte array cannot be decoded as a Bitmap
 *      (simulated by passing invalid bytes on the JVM where BitmapFactory returns null).
 *   3. The storeThumbnail callback is never invoked when decode fails.
 *
 * [SyncThumbnailHelper.generateAndStore] is designed to be non-fatal — it must
 * never throw; failures just return false so the image is still stored full-res.
 */
class SyncThumbnailHelperTest {

    @Test
    fun emptyBytes_returnsFalse_withoutThrowing() {
        // Empty byte array → BitmapFactory.decodeByteArray returns null → non-fatal false
        val result = SyncThumbnailHelper.generateAndStore(
            imageBytes = ByteArray(0),
            storeThumbnail = { _ -> },
        )
        assertFalse("empty bytes must return false (non-fatal)", result)
    }

    @Test
    fun invalidImageBytes_returnsFalse_withoutThrowing() {
        // Corrupt/non-PNG bytes → BitmapFactory.decodeByteArray returns null → non-fatal false
        val garbage = ByteArray(64) { it.toByte() }
        val result = SyncThumbnailHelper.generateAndStore(
            imageBytes = garbage,
            storeThumbnail = { _ -> },
        )
        assertFalse("invalid image bytes must return false (non-fatal)", result)
    }

    @Test
    fun storeThumbnail_isNeverCalledWhenDecodeFails() {
        // Ensure the storeThumbnail callback is NOT invoked when decode returns null.
        var callCount = 0
        SyncThumbnailHelper.generateAndStore(
            imageBytes = ByteArray(0),
            storeThumbnail = { _ -> callCount++ },
        )
        assertTrue("storeThumbnail must NOT be called when decode returns null", callCount == 0)
    }
}
