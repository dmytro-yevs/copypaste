package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for [ShareReceiverActivity.isStreamSizeAcceptable] — the pure size-cap
 * predicate guarding the exported share target against OOM/DoS (CopyPaste-lk5m).
 */
class ShareReceiverSizeCapTest {

    private val fileCap = ShareReceiverActivity.MAX_FILE_BYTES
    private val imageCap = ShareReceiverActivity.MAX_IMAGE_BYTES

    @Test fun unknownSizeIsAllowed_file() =
        assertTrue(ShareReceiverActivity.isStreamSizeAcceptable(-1L, isImage = false))

    @Test fun unknownSizeIsAllowed_image() =
        assertTrue(ShareReceiverActivity.isStreamSizeAcceptable(-1L, isImage = true))

    @Test fun zeroBytesAllowed() =
        assertTrue(ShareReceiverActivity.isStreamSizeAcceptable(0L, isImage = false))

    @Test fun smallFileAllowed() =
        assertTrue(ShareReceiverActivity.isStreamSizeAcceptable(1_024L, isImage = false))

    @Test fun fileAtCapAllowed() =
        assertTrue(ShareReceiverActivity.isStreamSizeAcceptable(fileCap, isImage = false))

    @Test fun fileOneOverCapRejected() =
        assertFalse(ShareReceiverActivity.isStreamSizeAcceptable(fileCap + 1, isImage = false))

    @Test fun hugeFileRejected() =
        assertFalse(ShareReceiverActivity.isStreamSizeAcceptable(2L * 1024 * 1024 * 1024, isImage = false))

    @Test fun imageAtCapAllowed() =
        assertTrue(ShareReceiverActivity.isStreamSizeAcceptable(imageCap, isImage = true))

    @Test fun imageOneOverCapRejected() =
        assertFalse(ShareReceiverActivity.isStreamSizeAcceptable(imageCap + 1, isImage = true))

    /** A file at the image cap (which is higher) must still be rejected on the file path. */
    @Test fun fileCapIsStricterThanImageCap() {
        val between = fileCap + 1
        assertFalse(ShareReceiverActivity.isStreamSizeAcceptable(between, isImage = false))
        assertTrue(ShareReceiverActivity.isStreamSizeAcceptable(between, isImage = true))
    }
}
