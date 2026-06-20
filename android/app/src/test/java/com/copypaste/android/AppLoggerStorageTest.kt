package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-k8cm: log/crash storage must use internal (MODE_PRIVATE) paths, not external.
 * CopyPaste-rurw: AppLogger must redact sensitive content before writing.
 *
 * These are pure-JVM tests — no Android Context required — exercising the
 * redaction helper and storage-path contract via inspectable constants/functions.
 */
class AppLoggerStorageTest {

    // ── CopyPaste-k8cm: internal storage contract ─────────────────────────────

    /**
     * Verify the documented storage path note does NOT mention external storage.
     *
     * The old implementation used getExternalFilesDir which puts logs under
     * /sdcard/Android/data/…/files/logs/ — world-readable via USB and MTP even
     * when the app is not running, defeating private storage guarantees.
     *
     * Fix: AppLogger.STORAGE_IS_INTERNAL must be true (set by the fixed implementation).
     */
    @Test
    fun `log storage is marked as internal not external`() {
        assertTrue(
            "AppLogger.STORAGE_IS_INTERNAL must be true — logs must use context.filesDir, not getExternalFilesDir",
            AppLogger.STORAGE_IS_INTERNAL,
        )
    }

    /**
     * Verify the legacy external-path string "getExternalFilesDir" is NOT referenced
     * in the documented adb-pull hint (which was the smoking-gun comment pointing to /sdcard/).
     *
     * This test acts as a compile-time guard: if the constant is updated back to point
     * at external storage, this test fails.
     */
    @Test
    fun `log dir description does not mention external sdcard path`() {
        val desc = AppLogger.LOG_DIR_DESCRIPTION
        assertFalse(
            "Log dir description must not mention /sdcard/ or external storage, got: $desc",
            desc.contains("/sdcard/", ignoreCase = true) ||
                desc.contains("external", ignoreCase = true),
        )
    }

    // ── CopyPaste-rurw: redaction contract ────────────────────────────────────

    /**
     * Tokens / API keys: strings of 20+ hex or base64 chars must be redacted.
     * Copied from a bearer token or API key in the clipboard.
     */
    @Test
    fun `redact scrubs long hex token`() {
        val raw = "Bearer a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4"
        val redacted = AppLogger.redact(raw)
        assertFalse("hex token must be scrubbed from log output", redacted.contains("a1b2c3d4e5f6"))
    }

    /**
     * Base64-looking strings (API keys, JWTs, secrets ≥ 20 chars).
     */
    @Test
    fun `redact scrubs long base64 segment`() {
        val raw = "key=ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh"
        val redacted = AppLogger.redact(raw)
        assertFalse("long base64 segment must be redacted", redacted.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh"))
    }

    /**
     * JWT format (header.payload.signature) must be redacted.
     */
    @Test
    fun `redact scrubs JWT-shaped token`() {
        val jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyMTIzIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV"
        val raw = "Authorization: Bearer $jwt"
        val redacted = AppLogger.redact(raw)
        assertFalse("JWT payload must be redacted", redacted.contains("eyJhbGciOiJIUzI1NiJ9"))
    }

    /**
     * Short human-readable messages must pass through unchanged so logs remain useful.
     */
    @Test
    fun `redact preserves short safe messages`() {
        val msg = "Logcat process started (api29Path=true, level=E)"
        val redacted = AppLogger.redact(msg)
        // The meaningful words must survive redaction.
        assertTrue("safe diagnostic messages must not be destroyed by redaction", redacted.contains("Logcat"))
        assertTrue("safe diagnostic messages must not be destroyed by redaction", redacted.contains("started"))
    }

    /**
     * Empty input must not throw and must return empty (or at least be non-null).
     */
    @Test
    fun `redact empty string returns empty string`() {
        val redacted = AppLogger.redact("")
        assertTrue("redact of empty string must return empty", redacted.isEmpty())
    }

    /**
     * UUID-shaped strings (item IDs) must be redacted — they are internal identifiers
     * that must not leak to log files (see CopyPaste-g4ik for the original guard).
     */
    @Test
    fun `redact scrubs UUID-shaped identifier`() {
        val raw = "item stored: 550e8400-e29b-41d4-a716-446655440000"
        val redacted = AppLogger.redact(raw)
        assertFalse("UUID identifier must be redacted from log output", redacted.contains("550e8400-e29b-41d4"))
    }

    /**
     * Log level / tag prefix must survive so log lines remain parse-able.
     */
    @Test
    fun `redact preserves log level and tag`() {
        val msg = "LogcatCaptureService started OK"
        val redacted = AppLogger.redact(msg)
        assertTrue("service tag must survive redaction", redacted.contains("LogcatCapture"))
    }
}
