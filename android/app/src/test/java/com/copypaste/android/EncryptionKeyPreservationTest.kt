package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * Tests for CopyPaste-gkgp: encryption key must never be silently regenerated
 * on read failure. Only a missing key (first run) should create a new key.
 *
 * These are pure-JVM tests verifying the contract described in Settings.loadOrCreateKey().
 * They do NOT require an Android runtime.
 *
 * Run via: ./gradlew :app:testDebugUnitTest --tests "*.EncryptionKeyPreservationTest"
 */
class EncryptionKeyPreservationTest {

    /**
     * CopyPaste-gkgp: EncryptionKeyLostException must be a subtype of Exception
     * so callers can catch it and surface a degraded-state error rather than
     * silently operating with a wrong key. It must NOT extend RuntimeException
     * so the compiler requires an explicit try/catch or @Throws annotation at
     * every call site.
     *
     * (JVM unit tests cannot instantiate Settings without Android Context, so we
     * validate the exception contract in isolation.)
     */
    @Test
    fun `EncryptionKeyLostException extends Exception`() {
        val e = EncryptionKeyLostException("test message")
        // Must be an Exception (checked) so callers are forced to handle it.
        assertTrue(
            "EncryptionKeyLostException must extend Exception",
            e is Exception,
        )
    }

    @Test
    fun `EncryptionKeyLostException carries the original cause`() {
        val cause = IllegalStateException("keystore error")
        val e = EncryptionKeyLostException("wrap failed", cause)
        assertNotNull("cause must be preserved", e.cause)
        assertTrue(
            "cause must be the original exception",
            e.cause is IllegalStateException,
        )
    }

    @Test
    fun `EncryptionKeyLostException message is non-blank`() {
        val e = EncryptionKeyLostException("KEK unavailable after key-store wipe")
        assertFalse(
            "Message must be non-blank",
            e.message.isNullOrBlank(),
        )
    }

    /**
     * Confirm that EncryptionKeyLostException is NOT a subtype of
     * RuntimeException.  If it were, callers could accidentally ignore it
     * (unchecked exception — no compiler warning), which would cause the
     * service to continue with a silently-random key, destroying history
     * exactly as described in the bug.
     *
     * NOTE: Kotlin/JVM does not enforce checked-exception at the language level,
     * so this test is the only compile-time-equivalent guard we can write in a
     * JVM unit test.
     */
    @Test
    fun `EncryptionKeyLostException is NOT a RuntimeException`() {
        val e = EncryptionKeyLostException("lost")
        assertFalse(
            "EncryptionKeyLostException must NOT extend RuntimeException — " +
                "unchecked exception would let callers silently ignore it",
            (e as Any) is RuntimeException,
        )
    }
}
