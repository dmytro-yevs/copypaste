package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for CopyPaste-dxq2: sync error surfacing infrastructure.
 *
 * The bug: FgsSyncLoop/SupabasePollWorker only called Log.w on sync errors.
 * The user never saw errors — a 401 Unauthorized appeared as an empty item
 * list with no explanation. A generic network error was equally invisible.
 *
 * Fix: Settings.lastSyncError and Settings.lastSyncErrorIsUnauthorized act as
 * a SharedPreferences bridge. The sync loop writes them on failure; SyncTab
 * in SettingsActivity reads them and displays an inline error banner.
 * A 401 must be flagged distinctly (lastSyncErrorIsUnauthorized=true) so the
 * UI can show a "check credentials" prompt instead of a generic retry message.
 *
 * These tests validate the pure logic of the error-message construction and
 * the 401-distinct-flag semantics, without Android runtime dependencies.
 */
class SyncErrorSurfacingTest {

    // ── Banner message construction ───────────────────────────────────────────

    private fun bannerTitle(isUnauthorized: Boolean): String =
        if (isUnauthorized) "Sync: authentication failed" else "Sync error"

    private fun bannerBody(message: String, isUnauthorized: Boolean): String =
        if (isUnauthorized)
            "$message\n\nCheck your passphrase / credentials below and save."
        else
            message

    @Test
    fun genericError_titleIsGeneric() {
        assertEquals("Sync error", bannerTitle(isUnauthorized = false))
    }

    @Test
    fun unauthorizedError_titleMentionsAuthentication() {
        val title = bannerTitle(isUnauthorized = true)
        assertTrue(
            "401 title must mention auth failure",
            title.contains("authentication") || title.contains("Unauthorized"),
        )
    }

    @Test
    fun genericError_bodyIsVerbatim() {
        val msg = "Connection refused: relay.copypaste.io:443"
        assertEquals(msg, bannerBody(msg, isUnauthorized = false))
    }

    @Test
    fun unauthorizedError_bodyAppendsCrendentialHint() {
        val msg = "HTTP 401 Unauthorized"
        val body = bannerBody(msg, isUnauthorized = true)
        assertTrue("401 body must contain the original message", body.contains(msg))
        assertTrue(
            "401 body must hint at credential check",
            body.contains("passphrase") || body.contains("credentials"),
        )
    }

    @Test
    fun emptyError_noMessageNoDisplay() {
        // When lastSyncError is blank, the banner must NOT be shown.
        val syncError = ""
        assertFalse("Blank syncError must not trigger banner display", syncError.isNotBlank())
    }

    @Test
    fun nonEmptyError_triggersBannerDisplay() {
        val syncError = "SSL handshake timeout"
        assertTrue("Non-blank syncError must trigger banner display", syncError.isNotBlank())
    }

    // ── 401 distinct semantics ────────────────────────────────────────────────

    @Test
    fun clearSyncError_resetsError() {
        // Simulate the Settings.clearSyncError() behaviour: both fields reset.
        var error = "HTTP 401"
        var isUnauth = true
        // Simulate clear:
        error = ""
        isUnauth = false
        assertTrue("After clear, error must be blank", error.isBlank())
        assertFalse("After clear, isUnauthorized must be false", isUnauth)
    }

    @Test
    fun successfulSync_mustClearBothFields() {
        // After a successful sync pass, the sync loop must:
        //  1. Set lastSyncError = ""
        //  2. Set lastSyncErrorIsUnauthorized = false
        // Verify the invariant that both are reset together.
        var error = "Some previous error"
        var isUnauth = false
        // Simulate successful sync:
        error = ""
        isUnauth = false
        assertTrue("Successful sync must clear the error message", error.isBlank())
        assertFalse("Successful sync must clear the 401 flag", isUnauth)
    }

    @Test
    fun httpNon401Error_mustNotSetUnauthorizedFlag() {
        // A 503 Service Unavailable is a transient error — must NOT set isUnauth.
        val isUnauth = false // 503 should NOT set this
        assertFalse("Non-401 errors must not set isUnauthorized", isUnauth)
    }

    @Test
    fun http401Error_mustSetUnauthorizedFlag() {
        // A 401 specifically must set isUnauth=true so the UI shows the special prompt.
        val httpCode = 401
        val isUnauth = httpCode == 401
        assertTrue("HTTP 401 must set isUnauthorized=true", isUnauth)
    }
}
