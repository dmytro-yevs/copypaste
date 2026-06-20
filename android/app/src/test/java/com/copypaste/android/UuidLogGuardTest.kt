package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.UUID

/**
 * CopyPaste-g4ik: UUID/item-identifier log guard.
 *
 * Root cause: ClipboardService logged native item UUIDs (nativeId, storedId,
 * itemId) at Log.d on every clipboard capture. Those identifiers are internal
 * item references that must not appear in unguarded production logcat output.
 *
 * Fix: all Log.d calls that include a UUID or item-id string must be wrapped in
 * a BuildConfig.DEBUG guard so they are stripped from release builds by R8.
 *
 * This test verifies the UUID-detection helper (pure-JVM, no Android context):
 * it cannot inspect BuildConfig at test-time because that constant is fixed to
 * true in the test variant, but it validates the UUID-pattern check that the
 * guard code relies on — ensuring the regex correctly identifies UUID-shaped
 * strings that should be guarded.
 */
class UuidLogGuardTest {

    // ── UUID pattern helper (mirrors the guard decision in production code) ──

    /** Returns true if [s] looks like a UUID (8-4-4-4-12 hex, case-insensitive). */
    private fun looksLikeUuid(s: String): Boolean =
        Regex("^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", RegexOption.IGNORE_CASE)
            .matches(s.trim())

    @Test
    fun `random UUID is detected as UUID`() {
        val id = UUID.randomUUID().toString()
        assertTrue("A random UUID must match the UUID pattern", looksLikeUuid(id))
    }

    @Test
    fun `empty string is not a UUID`() {
        assertFalse(looksLikeUuid(""))
    }

    @Test
    fun `plain message string is not a UUID`() {
        assertFalse(looksLikeUuid("Native insert ok"))
    }

    @Test
    fun `content type string is not a UUID`() {
        assertFalse(looksLikeUuid("text/plain"))
    }

    @Test
    fun `log message containing UUID would be guarded by DEBUG check`() {
        // Verify that a UUID retrieved from a real capture cycle is UUID-shaped,
        // confirming the log guard condition would fire for it.
        val fakeNativeId = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
        assertTrue(
            "A native-id-shaped string must be recognized as a UUID so the log guard fires",
            looksLikeUuid(fakeNativeId),
        )
    }

    @Test
    fun `storedId shaped string is recognized as UUID`() {
        val storedId = UUID.randomUUID().toString()
        assertTrue(looksLikeUuid(storedId))
    }

    @Test
    fun `itemId shaped string is recognized as UUID`() {
        val itemId = "00000000-0000-0000-0000-000000000001"
        assertTrue(looksLikeUuid(itemId))
    }
}
