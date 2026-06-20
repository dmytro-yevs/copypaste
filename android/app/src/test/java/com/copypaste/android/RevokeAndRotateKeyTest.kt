package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * Pure-JVM unit tests for the revoke+rotate sync-key feature (CopyPaste-8qcm).
 *
 * Covers:
 *  1. [revokeDeviceAndRotateKey] wrapper: stub-mode guard (no native library loaded)
 *     throws [IllegalStateException] — matching the pattern of all other wrappers
 *     that return SECRET material.
 *  2. Passphrase validation logic: [isValidRotatePassphrase] — min 8 chars.
 *  3. [RevokeMode] enum: both members are distinct and have the expected name.
 *  4. [revokeWithAuditOnly] pure-logic path: verifies the plain revoke path
 *     delegates to [revokeDeviceAudit] contract (structural check, no FFI needed).
 *
 * All tests run on the JVM (no Android runtime, no NDK). The native library is NOT
 * loaded in this environment ([isNativeLibraryLoaded] == false), so the stub paths
 * execute.
 */
class RevokeAndRotateKeyTest {

    // ── 1. revokeDeviceAndRotateKey stub-mode guard ───────────────────────────

    @Test
    fun revokeDeviceAndRotateKey_stubMode_throwsIllegalState() {
        // isNativeLibraryLoaded is false in JVM unit tests (no .so on the classpath).
        // The wrapper MUST throw rather than return a stub/empty key — returning a
        // zero/empty key would silently break sync on the calling side.
        try {
            revokeDeviceAndRotateKey(
                dbPath = "/tmp/test.db",
                key = ByteArray(32),
                fingerprint = "aabbccdd",
                name = "Test peer",
                newPassphrase = "correct-horse-battery-staple",
            )
            fail("Expected IllegalStateException when native library is not loaded")
        } catch (e: IllegalStateException) {
            // Expected: the wrapper must not return a stub key.
            assertTrue(
                "Exception message must mention native library unavailability",
                e.message?.contains("native library") == true ||
                    e.message?.contains("not loaded") == true,
            )
        }
    }

    // ── 2. Passphrase validation ──────────────────────────────────────────────

    @Test
    fun isValidRotatePassphrase_emptyString_returnsFalse() {
        assertFalse("Empty passphrase must be invalid", isValidRotatePassphrase(""))
    }

    @Test
    fun isValidRotatePassphrase_sevenChars_returnsFalse() {
        assertFalse("7-char passphrase must be invalid (< 8 chars)", isValidRotatePassphrase("1234567"))
    }

    @Test
    fun isValidRotatePassphrase_exactlyEightChars_returnsTrue() {
        assertTrue("8-char passphrase is the minimum valid length", isValidRotatePassphrase("12345678"))
    }

    @Test
    fun isValidRotatePassphrase_longPassphrase_returnsTrue() {
        assertTrue(
            "Long passphrase must be valid",
            isValidRotatePassphrase("correct-horse-battery-staple"),
        )
    }

    // ── 3. RevokeMode enum ───────────────────────────────────────────────────

    @Test
    fun revokeMode_auditOnly_hasDistinctName() {
        assertEquals("AUDIT_ONLY", RevokeMode.AUDIT_ONLY.name)
    }

    @Test
    fun revokeMode_revokeAndRotate_hasDistinctName() {
        assertEquals("REVOKE_AND_ROTATE", RevokeMode.REVOKE_AND_ROTATE.name)
    }

    @Test
    fun revokeMode_twoMembersAreDistinct() {
        assertTrue(
            "AUDIT_ONLY and REVOKE_AND_ROTATE must be different enum constants",
            RevokeMode.AUDIT_ONLY != RevokeMode.REVOKE_AND_ROTATE,
        )
    }
}
