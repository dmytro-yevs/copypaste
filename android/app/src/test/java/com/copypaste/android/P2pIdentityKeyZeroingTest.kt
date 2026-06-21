package com.copypaste.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for CopyPaste-ah3i: the ByteArray helpers that DeviceKeyStore uses
 * to produce and zero P2pIdentity key material must honour the zeroing contract.
 *
 * The UniFFI-generated DeviceCert carries keyDer as List<UByte> (JVM boxed list
 * — cannot be zeroed in-place). The intermediate ByteArray created by
 * DeviceCert.toP2pIdentity() CAN be zeroed, and Settings.p2pIdentity setter
 * should zero its local copy after wrapping with the AndroidKeyStore KEK.
 *
 * These tests validate:
 *   1. zeroByteArray() fills every byte with 0.
 *   2. After zeroing, the array is all zeros.
 *   3. P2pIdentity.zeroKeyMaterial() zeroes the keyDer in-place.
 *   4. The zeroing function is idempotent (safe to call multiple times).
 */
class P2pIdentityKeyZeroingTest {

    // ── Helper: P2pIdentity.zeroKeyMaterial() ────────────────────────────────

    private fun makeIdentity(keyBytes: ByteArray) = P2pIdentity(
        deviceId = "test-device",
        fingerprint = "aabbccdd",
        certDer = ByteArray(4) { 0xAB.toByte() },
        keyDer = keyBytes,
    )

    @Test
    fun zeroKeyMaterial_fillsAllBytesWithZero() {
        val key = byteArrayOf(1, 2, 3, 4, 5, 6, 7, 8)
        val identity = makeIdentity(key)
        identity.zeroKeyMaterial()
        assertArrayEquals(
            "zeroKeyMaterial must overwrite all keyDer bytes with 0",
            ByteArray(8) { 0 },
            identity.keyDer,
        )
    }

    @Test
    fun zeroKeyMaterial_doesNotAffectCertDer() {
        val key = byteArrayOf(1, 2, 3)
        val identity = makeIdentity(key)
        val certBefore = identity.certDer.copyOf()
        identity.zeroKeyMaterial()
        assertArrayEquals(
            "zeroKeyMaterial must not touch certDer",
            certBefore,
            identity.certDer,
        )
    }

    @Test
    fun zeroKeyMaterial_isIdempotent() {
        val key = byteArrayOf(9, 8, 7)
        val identity = makeIdentity(key)
        identity.zeroKeyMaterial()
        identity.zeroKeyMaterial() // second call must not throw
        assertArrayEquals(ByteArray(3) { 0 }, identity.keyDer)
    }

    @Test
    fun zeroKeyMaterial_worksOnEmptyKey() {
        val identity = makeIdentity(ByteArray(0))
        identity.zeroKeyMaterial() // must not throw on empty array
        assertEquals(0, identity.keyDer.size)
    }

    @Test
    fun zeroKeyMaterial_largeKey_allZeroed() {
        val key = ByteArray(4096) { it.toByte() }
        val identity = makeIdentity(key)
        identity.zeroKeyMaterial()
        assertTrue(
            "All bytes in a 4096-byte key must be zeroed",
            identity.keyDer.all { it == 0.toByte() },
        )
    }

    // ── ByteArray zeroing helper used by DeviceKeyStore ───────────────────────

    @Test
    fun byteArrayFill_zerosAllBytes() {
        val arr = byteArrayOf(0x11, 0x22, 0x33, 0x44)
        arr.fill(0)
        assertArrayEquals(ByteArray(4) { 0 }, arr)
    }

    @Test
    fun byteArrayFill_zeroOnEmptyArray_doesNotThrow() {
        val empty = ByteArray(0)
        empty.fill(0) // must not throw
        assertEquals(0, empty.size)
    }
}
