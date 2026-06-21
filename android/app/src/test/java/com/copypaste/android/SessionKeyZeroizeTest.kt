package com.copypaste.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Tests for CopyPaste-ah3i — PAKE session key zeroization contract enforcement.
 *
 * The UDL contract (copypaste_android.udl lines 332–333, 362–364) mandates:
 *   "key_der and each session_key are secret — the caller MUST zero the
 *    ByteArrays after the call and never log them."
 *
 * Previously, the CopypasteBindings.kt wrappers for [syncWithPeer],
 * [startP2pListener], and [updateP2pListenerPeers] did NOT zero the session key
 * ByteArrays after passing them to Rust, leaving plaintext PAKE session key bytes
 * on the JVM heap until GC. These tests verify the fix:
 *
 *  1. [syncWithPeer] zeros [sessionKey] in its finally block (verified in stub
 *     mode where the call throws [IllegalStateException] before touching Rust).
 *  2. [startP2pListener] zeros each [PeerSessionKeyInfo.sessionKey] in its finally
 *     block (stub path throws [IllegalStateException]).
 *  3. [updateP2pListenerPeers] zeros each session key in its finally block
 *     (stub path is a no-op return, but finally still runs).
 *  4. [PeerSessionKeyInfo] equality semantics remain correct after zeroing.
 *
 * All tests run on the JVM — no NDK, no Android runtime required.
 * In stub mode [isNativeLibraryLoaded] is false; the wrappers throw or return early,
 * but the finally blocks still execute, zeroing the key bytes.
 *
 * Run via: ./gradlew :app:testDebugUnitTest --tests "*.SessionKeyZeroizeTest"
 */
class SessionKeyZeroizeTest {

    private val zeroed32 = ByteArray(32) { 0 }

    // ── 1. syncWithPeer zeros sessionKey in its finally block ─────────────────────

    /**
     * CopyPaste-ah3i: after [syncWithPeer] throws (stub mode — no .so), the
     * [sessionKey] array passed by the caller must be all-zero bytes.
     *
     * This proves the finally block runs on every exit path including the
     * [IllegalStateException] thrown when the native library is absent.
     */
    @Test
    fun syncWithPeer_zerosSessionKey_onStubThrow() {
        val key = ByteArray(32) { (it + 1).toByte() }
        // Capture a copy of the original bytes to confirm they were non-zero first.
        val originalCopy = key.copyOf()
        assertTrue("Pre-condition: key must be non-zero", originalCopy.any { it != 0.toByte() })

        try {
            syncWithPeer(
                peerAddr = "127.0.0.1:9999",
                peerFingerprint = "deadbeefdeadbeef",
                sessionKey = key,
                certDer = emptyList(),
                keyDer = emptyList(),
                localItems = emptyList(),
                revokedFingerprints = emptyList(),
                deviceId = "test-device",
            )
        } catch (e: IllegalStateException) {
            // Expected in stub mode — native library not loaded.
        }

        // The finally block must have zeroed the key regardless of exception.
        assertArrayEquals(
            "CopyPaste-ah3i: syncWithPeer must zero sessionKey bytes in finally block",
            zeroed32,
            key,
        )
    }

    // ── 2. startP2pListener zeros session keys in its finally block ───────────────

    /**
     * CopyPaste-ah3i: after [startP2pListener] throws (stub mode), every
     * [PeerSessionKeyInfo.sessionKey] in the passed list must be all-zero bytes.
     */
    @Test
    fun startP2pListener_zerosesPeerSessionKeys_onStubThrow() {
        val key1 = ByteArray(32) { 0xAA.toByte() }
        val key2 = ByteArray(32) { 0xBB.toByte() }
        val peers = listOf(
            PeerSessionKeyInfo("fp-aaa", key1),
            PeerSessionKeyInfo("fp-bbb", key2),
        )

        try {
            startP2pListener(
                listenPort = 0,
                certDer = emptyList(),
                keyDer = emptyList(),
                allowedFingerprints = emptyList(),
                revokedFingerprints = emptyList(),
                sessionKeys = peers,
                localItems = emptyList(),
                deviceId = "test-device",
            )
        } catch (e: IllegalStateException) {
            // Expected in stub mode.
        }

        assertArrayEquals(
            "CopyPaste-ah3i: startP2pListener must zero peer key1 in finally block",
            zeroed32,
            key1,
        )
        assertArrayEquals(
            "CopyPaste-ah3i: startP2pListener must zero peer key2 in finally block",
            zeroed32,
            key2,
        )
    }

    // ── 3. updateP2pListenerPeers zeros session keys in its finally block ─────────

    /**
     * CopyPaste-ah3i: [updateP2pListenerPeers] returns early in stub mode
     * (isNativeLibraryLoaded == false) but must still execute the finally block
     * that zeros session keys.
     */
    @Test
    fun updateP2pListenerPeers_zerosSessionKeys_onStubReturn() {
        val key = ByteArray(32) { 0x55.toByte() }
        val peers = listOf(PeerSessionKeyInfo("fp-ccc", key))

        // In stub mode this returns without throwing (unlike syncWithPeer/startP2pListener).
        updateP2pListenerPeers(
            listenerId = 0L,
            allowed = emptyList(),
            revoked = emptyList(),
            sessionKeys = peers,
        )

        assertArrayEquals(
            "CopyPaste-ah3i: updateP2pListenerPeers must zero session keys even in stub return",
            zeroed32,
            key,
        )
    }

    // ── 4. PeerSessionKeyInfo equality is unaffected ──────────────────────────────

    /**
     * Sanity-check that [PeerSessionKeyInfo] equality uses [ByteArray.contentEquals]
     * so two objects with the same fingerprint and key bytes are equal, and that the
     * class itself compiles correctly with the security comment added in the fix.
     */
    @Test
    fun peerSessionKeyInfo_equalityByContent() {
        val a = PeerSessionKeyInfo("fp-test", ByteArray(32) { 1 })
        val b = PeerSessionKeyInfo("fp-test", ByteArray(32) { 1 })
        val c = PeerSessionKeyInfo("fp-test", ByteArray(32) { 2 })
        assertTrue("Same fingerprint + same key bytes must be equal", a == b)
        assertTrue("Same fingerprint + different key bytes must not be equal", a != c)
    }
}
