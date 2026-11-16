package com.copypaste.app

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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

    @Test fun reportedLengthSizesTheSourceBufferExactly() {
        val pending = AndroidClipboardReader.Pending(AndroidClipboardReader.Host(1))
        val source = ByteArray(20_000) { it.toByte() }
        val payload = AndroidClipboardReader.readBounded(source.inputStream(), 65_536, pending, source.size.toLong())!!
        assertEquals(source.size, payload.bytes.size)
        assertArrayEquals(source, payload.bytes)
        assertEquals(null, AndroidClipboardReader.readBounded(source.inputStream(), 19_999, pending, source.size.toLong()))
    }

    @Test fun pngPassThroughAcceptsOnlyOrdinaryStaticImages() {
        fun chunk(type: String, data: ByteArray = ByteArray(0)) =
            java.nio.ByteBuffer.allocate(4).putInt(data.size).array() + type.toByteArray() + data + ByteArray(4)
        fun png(depth: Int = 8, interlace: Int = 0, animated: Boolean = false): ByteArray {
            val header = ByteArray(13).also { it[3] = 1; it[7] = 1; it[8] = depth.toByte(); it[9] = 6; it[12] = interlace.toByte() }
            return byteArrayOf(-119, 80, 78, 71, 13, 10, 26, 10) + chunk("IHDR", header) +
                (if (animated) chunk("acTL", ByteArray(8)) else ByteArray(0)) + chunk("IDAT", ByteArray(2)) +
                byteArrayOf(0, 0, 0, 0, 73, 69, 78, 68, -82, 66, 96, -126)
        }
        fun ordinary(bytes: ByteArray, length: Int = bytes.size) =
            AndroidClipboardReader.ordinaryPng(AndroidClipboardReader.BinaryPayload(bytes + ByteArray(4), length))
        assertTrue(ordinary(png()))
        assertFalse(ordinary(png(), png().size - 1))
        assertFalse(ordinary(png(depth = 16)))
        assertFalse(ordinary(png(interlace = 1)))
        assertFalse(ordinary(png(animated = true)))
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
