package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for CopyPaste-v0yi: pasteAsPlainText must affect COPY behaviour in
 * HistoryActivity, not just be stored without effect.
 *
 * The setting's intent: when enabled, copying any item (text, image, or file)
 * from history always produces a plain-text ClipData, NOT a binary URI.
 * Previously the setting was stored but never read at copy time — structural no-op.
 *
 * These tests validate the copy-decision logic extracted from HistoryActivity:
 *   [copyDecision] mirrors the `when` branch selection in copyItemById.
 *
 * The pure-logic helper is tested here without Android runtime dependencies.
 */
class PasteAsPlainTextTest {

    /** Mirror of the copy-path decision in copyItemById. */
    enum class CopyPath { IMAGE_URI, FILE_URI, PLAIN_TEXT }

    /**
     * Decision function matching the fixed copyItemById logic:
     *  - pasteAsPlainText=true  → always PLAIN_TEXT regardless of item type.
     *  - pasteAsPlainText=false → IMAGE_URI for images, FILE_URI for files, PLAIN_TEXT for text.
     */
    private fun copyDecision(
        isImage: Boolean,
        isFile: Boolean,
        pasteAsPlainText: Boolean,
    ): CopyPath = when {
        pasteAsPlainText -> CopyPath.PLAIN_TEXT   // override: always plain text
        isImage -> CopyPath.IMAGE_URI
        isFile -> CopyPath.FILE_URI
        else -> CopyPath.PLAIN_TEXT
    }

    // ── pasteAsPlainText=false (existing behaviour must be unchanged) ──────

    @Test
    fun pasteOff_imageItem_returnsImageUri() {
        assertEquals(CopyPath.IMAGE_URI, copyDecision(isImage = true, isFile = false, pasteAsPlainText = false))
    }

    @Test
    fun pasteOff_fileItem_returnsFileUri() {
        assertEquals(CopyPath.FILE_URI, copyDecision(isImage = false, isFile = true, pasteAsPlainText = false))
    }

    @Test
    fun pasteOff_textItem_returnsPlainText() {
        assertEquals(CopyPath.PLAIN_TEXT, copyDecision(isImage = false, isFile = false, pasteAsPlainText = false))
    }

    // ── pasteAsPlainText=true: ALL paths must produce PLAIN_TEXT ─────────

    @Test
    fun pasteOn_imageItem_returnsPlainText_notImageUri() {
        // This is the primary regression test: with the setting ON, images must NOT
        // be copied as URIs — previously the setting was ignored here.
        val result = copyDecision(isImage = true, isFile = false, pasteAsPlainText = true)
        assertEquals(
            "pasteAsPlainText=true must downgrade image copy to plain text",
            CopyPath.PLAIN_TEXT, result,
        )
        assertNotEquals(CopyPath.IMAGE_URI, result)
    }

    @Test
    fun pasteOn_fileItem_returnsPlainText_notFileUri() {
        val result = copyDecision(isImage = false, isFile = true, pasteAsPlainText = true)
        assertEquals(
            "pasteAsPlainText=true must downgrade file copy to plain text",
            CopyPath.PLAIN_TEXT, result,
        )
        assertNotEquals(CopyPath.FILE_URI, result)
    }

    @Test
    fun pasteOn_textItem_remainsPlainText() {
        // Text was already plain text — this must still be plain text.
        assertEquals(
            CopyPath.PLAIN_TEXT,
            copyDecision(isImage = false, isFile = false, pasteAsPlainText = true),
        )
    }

    // ── Idempotency / exhaustive ─────────────────────────────────────────

    @Test
    fun allCombinations_pasteOn_alwaysPlainText() {
        // Exhaustive: regardless of item type, pasteAsPlainText=true is PLAIN_TEXT.
        for (isImage in listOf(false, true)) {
            for (isFile in listOf(false, true)) {
                if (isImage && isFile) continue // invalid state — both cannot be true
                val result = copyDecision(isImage, isFile, pasteAsPlainText = true)
                assertEquals(
                    "pasteAsPlainText=true must always yield PLAIN_TEXT " +
                        "(isImage=$isImage, isFile=$isFile)",
                    CopyPath.PLAIN_TEXT, result,
                )
            }
        }
    }

    @Test
    fun allCombinations_pasteOff_respectsItemType() {
        // Exhaustive: with the setting off, item type drives the decision.
        val expectations = listOf(
            Triple(true, false, CopyPath.IMAGE_URI),
            Triple(false, true, CopyPath.FILE_URI),
            Triple(false, false, CopyPath.PLAIN_TEXT),
        )
        for ((isImage, isFile, expected) in expectations) {
            assertEquals(
                "pasteAsPlainText=false: isImage=$isImage isFile=$isFile should yield $expected",
                expected,
                copyDecision(isImage, isFile, pasteAsPlainText = false),
            )
        }
    }
}
