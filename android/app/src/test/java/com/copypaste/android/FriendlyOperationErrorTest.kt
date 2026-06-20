package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-7m6r: ErrorMessages.friendlyOperationError must return user-safe strings.
 *
 * Root cause: HistoryActivity toasts surfaced raw exception messages (e.g. SQLite
 * class names, file-system paths from e.message) directly to the user. The fix
 * routes them through friendlyOperationError which strips internals and returns a
 * safe, localised message.
 *
 * These are pure-JVM tests; no Android context is required.
 */
class FriendlyOperationErrorTest {

    // ── helper: create a fake Exception with a given raw message ──────────────

    private fun exc(msg: String): Exception = RuntimeException(msg)

    // ── friendlyOperationError must not expose internals ──────────────────────

    @Test
    fun `friendlyOperationError does not expose sql class names`() {
        val e = exc("android.database.sqlite.SQLiteDatabaseCorruptException: database disk image is malformed (code 11)")
        val msg = ErrorMessages.friendlyOperationError(e)
        assertFalse(
            "friendlyOperationError must not contain SQLite class name",
            msg.contains("android.database.sqlite"),
        )
        assertFalse(
            "friendlyOperationError must not contain raw exception text",
            msg.contains("disk image is malformed"),
        )
    }

    @Test
    fun `friendlyOperationError does not expose file paths`() {
        val e = exc("SQLITE_CANTOPEN: /data/data/com.copypaste.android/files/clipboard.db")
        val msg = ErrorMessages.friendlyOperationError(e)
        assertFalse(
            "friendlyOperationError must not leak file paths",
            msg.contains("/data/"),
        )
        assertFalse(
            "friendlyOperationError must not leak package-private paths",
            msg.contains("com.copypaste.android"),
        )
    }

    @Test
    fun `friendlyOperationError returns non-blank string`() {
        val e = exc("something went wrong")
        val msg = ErrorMessages.friendlyOperationError(e)
        assertTrue(
            "friendlyOperationError must return a non-blank message",
            msg.isNotBlank(),
        )
    }

    @Test
    fun `friendlyOperationError handles null message gracefully`() {
        // Exception with a null message (e.g. NullPointerException())
        val e = NullPointerException()
        val msg = ErrorMessages.friendlyOperationError(e)
        assertTrue(
            "friendlyOperationError must return a non-blank message even for null-message exceptions",
            msg.isNotBlank(),
        )
        assertFalse(
            "friendlyOperationError must not expose class name on null message",
            msg.contains("NullPointerException"),
        )
    }

    @Test
    fun `friendlyOperationError recognises network-class errors`() {
        val e = exc("Connection refused: localhost/127.0.0.1:8080")
        val msg = ErrorMessages.friendlyOperationError(e)
        // The message should be safe (no internals) and non-blank.
        assertTrue(msg.isNotBlank())
        assertFalse("No raw connection string in output", msg.contains("127.0.0.1"))
    }

    @Test
    fun `friendlyOperationError recognises storage-class errors`() {
        val e = exc("ENOSPC: no space left on device")
        val msg = ErrorMessages.friendlyOperationError(e)
        assertTrue(msg.isNotBlank())
        // Should hint at storage
        assertTrue(
            "Storage error should mention storage or disk in the friendly message",
            msg.contains("storage", ignoreCase = true) ||
                msg.contains("space", ignoreCase = true) ||
                msg.contains("disk", ignoreCase = true) ||
                msg.contains("device", ignoreCase = true),
        )
    }
}
