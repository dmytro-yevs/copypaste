package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Verifies that [ByteArray.asUByteArray().asList()] is semantically equivalent
 * to [ByteArray.map { it.toUByte() }] for all byte values, at zero per-element
 * allocation cost (CopyPaste-a0er).
 *
 * Pure JVM test — no Android runtime required.
 */
@OptIn(ExperimentalUnsignedTypes::class)
class UByteBoxingTest {

    @Test
    fun asUByteArrayAsList_matchesMapToUByte_forSampleValues() {
        val bytes = byteArrayOf(0, 127, -1)

        val boxing = bytes.map { it.toUByte() }
        val noBoxing = bytes.asUByteArray().asList()

        assertEquals("size should match", boxing.size, noBoxing.size)
        for (i in boxing.indices) {
            assertEquals("element $i should match", boxing[i], noBoxing[i])
        }
    }

    @Test
    fun asUByteArrayAsList_matchesMapToUByte_forAllByteValues() {
        // Exhaustively test every possible signed byte value (-128..127)
        val bytes = ByteArray(256) { i -> (i - 128).toByte() }

        val boxing = bytes.map { it.toUByte() }
        val noBoxing = bytes.asUByteArray().asList()

        assertEquals(boxing.size, noBoxing.size)
        for (i in boxing.indices) {
            assertEquals("element $i: boxing=${boxing[i]} noBoxing=${noBoxing[i]}", boxing[i], noBoxing[i])
        }
    }

    @Test
    fun asUByteArrayAsList_emptyArray_returnsEmptyList() {
        val bytes = byteArrayOf()
        assertEquals(emptyList<UByte>(), bytes.asUByteArray().asList())
    }

    @Test
    fun asUByteArrayAsList_highBitValues_areUnsigned() {
        // 0xFF as signed byte is -1; as UByte it should be 255u
        val bytes = byteArrayOf(-1, -128, -2)
        val result = bytes.asUByteArray().asList()
        assertEquals(255u.toUByte(), result[0])
        assertEquals(128u.toUByte(), result[1])
        assertEquals(254u.toUByte(), result[2])
    }
}
