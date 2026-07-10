package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * CopyPaste-myh8.11 S11 W3: pure-function coverage for [logLevelMarker], which
 * feeds LogLine's leading marker (icon/badge) so colour is never the sole
 * level signal.
 */
class LogLevelMarkerTest {

    @Test
    fun `detects error level`() {
        assertEquals(LogLevel.E, logLevelMarker("2026-01-15 12:34:56.789 E/MyTag: boom"))
    }

    @Test
    fun `detects warning level`() {
        assertEquals(LogLevel.W, logLevelMarker("2026-01-15 12:34:56.789 W/MyTag: careful"))
    }

    @Test
    fun `detects info level`() {
        assertEquals(LogLevel.I, logLevelMarker("2026-01-15 12:34:56.789 I/MyTag: started"))
    }

    @Test
    fun `detects debug level`() {
        assertEquals(LogLevel.D, logLevelMarker("2026-01-15 12:34:56.789 D/MyTag: verbose"))
    }

    @Test
    fun `crash header line has no level`() {
        assertNull(logLevelMarker("=== Crash report ==="))
    }

    @Test
    fun `plain stack trace line has no level`() {
        assertNull(logLevelMarker("    at com.copypaste.android.Foo.bar(Foo.kt:42)"))
    }

    @Test
    fun `plain text without a level code has no level`() {
        assertNull(logLevelMarker("just some free text"))
    }
}
