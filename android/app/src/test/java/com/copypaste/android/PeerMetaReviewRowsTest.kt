package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-1jms.33: pure-JVM unit tests for [peerMetaReviewRows].
 *
 * Verifies that the peer-review card helper:
 *  - includes only non-null, non-blank values
 *  - preserves label order: model → OS → version
 *  - returns an empty list when all fields are absent (no crash, no empty section)
 *
 * No Android SDK or Compose runtime required — pure Kotlin.
 */
class PeerMetaReviewRowsTest {

    // ── All three fields present ──────────────────────────────────────────────

    @Test
    fun `all three fields present — returns model, OS, version in order`() {
        val rows = peerMetaReviewRows(
            peerModel = "MacBook Air (M3)",
            peerOs = "macOS 15.3",
            peerAppVersion = "0.5.3",
        )
        assertEquals(3, rows.size)
        assertEquals("meta_label_model" to "MacBook Air (M3)", rows[0])
        assertEquals("meta_label_os" to "macOS 15.3", rows[1])
        assertEquals("meta_label_version" to "0.5.3", rows[2])
    }

    // ── Null fields are excluded ──────────────────────────────────────────────

    @Test
    fun `null peerModel is excluded`() {
        val rows = peerMetaReviewRows(
            peerModel = null,
            peerOs = "macOS 15.3",
            peerAppVersion = "0.5.3",
        )
        assertEquals(2, rows.size)
        assertEquals("meta_label_os" to "macOS 15.3", rows[0])
        assertEquals("meta_label_version" to "0.5.3", rows[1])
    }

    @Test
    fun `null peerOs is excluded`() {
        val rows = peerMetaReviewRows(
            peerModel = "MacBook Air (M3)",
            peerOs = null,
            peerAppVersion = "0.5.3",
        )
        assertEquals(2, rows.size)
        assertEquals("meta_label_model" to "MacBook Air (M3)", rows[0])
        assertEquals("meta_label_version" to "0.5.3", rows[1])
    }

    @Test
    fun `null peerAppVersion is excluded`() {
        val rows = peerMetaReviewRows(
            peerModel = "MacBook Air (M3)",
            peerOs = "macOS 15.3",
            peerAppVersion = null,
        )
        assertEquals(2, rows.size)
        assertEquals("meta_label_model" to "MacBook Air (M3)", rows[0])
        assertEquals("meta_label_os" to "macOS 15.3", rows[1])
    }

    // ── Blank fields are excluded (pre-ABI-14 / empty string from FFI) ───────

    @Test
    fun `blank peerModel is excluded`() {
        val rows = peerMetaReviewRows(
            peerModel = "  ",
            peerOs = "macOS 15.3",
            peerAppVersion = "0.5.3",
        )
        assertEquals(2, rows.size)
        assertTrue("model row must not be present", rows.none { it.first == "meta_label_model" })
    }

    @Test
    fun `blank peerOs is excluded`() {
        val rows = peerMetaReviewRows(
            peerModel = "MacBook Air (M3)",
            peerOs = "",
            peerAppVersion = "0.5.3",
        )
        assertEquals(2, rows.size)
        assertTrue("OS row must not be present", rows.none { it.first == "meta_label_os" })
    }

    @Test
    fun `blank peerAppVersion is excluded`() {
        val rows = peerMetaReviewRows(
            peerModel = "MacBook Air (M3)",
            peerOs = "macOS 15.3",
            peerAppVersion = "",
        )
        assertEquals(2, rows.size)
        assertTrue("version row must not be present", rows.none { it.first == "meta_label_version" })
    }

    // ── All fields absent ─────────────────────────────────────────────────────

    @Test
    fun `all fields null returns empty list — no crash`() {
        val rows = peerMetaReviewRows(
            peerModel = null,
            peerOs = null,
            peerAppVersion = null,
        )
        assertTrue("empty list expected when no metadata available", rows.isEmpty())
    }

    @Test
    fun `all fields blank returns empty list — no crash`() {
        val rows = peerMetaReviewRows(
            peerModel = "",
            peerOs = "  ",
            peerAppVersion = "\t",
        )
        assertTrue("empty list expected when all fields are blank", rows.isEmpty())
    }

    // ── Single field ──────────────────────────────────────────────────────────

    @Test
    fun `only model present — single row with correct label key`() {
        val rows = peerMetaReviewRows(
            peerModel = "Pixel 9 Pro",
            peerOs = null,
            peerAppVersion = null,
        )
        assertEquals(1, rows.size)
        assertEquals("meta_label_model" to "Pixel 9 Pro", rows[0])
    }
}
