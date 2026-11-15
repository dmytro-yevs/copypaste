package com.copypaste.app

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidClipboardBufferTest {
    @Test fun encodedPayloadIncludesOnlyWrittenBytes() {
        val output = AndroidClipboardReader.BoundedOutputStream(64)
        val pngPrefix = byteArrayOf(-119, 80, 78, 71, 13, 10, 26, 10, 1)
        output.write(pngPrefix)
        val payload = output.bytes()
        assertEquals(pngPrefix.size, payload.length)
        assertTrue(payload.bytes.size > payload.length)
        assertArrayEquals(pngPrefix, payload.bytes.copyOf(payload.length))
    }

    @Test fun encodedPayloadRejectsBytesBeyondLimit() {
        val output = AndroidClipboardReader.BoundedOutputStream(3)
        output.write(byteArrayOf(1, 2, 3))
        try {
            output.write(4)
            throw AssertionError("Expected size limit failure")
        } catch (_: AndroidClipboardReader.SizeLimitExceeded) {
            assertEquals(3, output.bytes().length)
        }
    }
}
