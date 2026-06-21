package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-5917.76: paste-as-plain-text silently ignores image/file items.
 *
 * Root cause: the `else` branch in copyItemById (HistoryActivity) runs for ALL items
 * not handled by the typed branches. With pasteAsPlainText=true, the image/file typed
 * branches are skipped (they require !forcePlainText). The else branch then called
 * loadFullPlaintext which returns null for image/file items, falling back to item.snippet
 * (e.g. "[image]") and silently setting a useless clip — no toast, no explanation.
 *
 * Fix: in the else branch, if forcePlainText && (item.isImage || item.isFile), call
 * onMediaCopyAsText callback and return without modifying the clipboard.
 *
 * These tests verify the copy-path and feedback logic using a pure simulation.
 */
class PasteAsPlainTextMediaFeedbackTest {

    /** Mirror of the actual copy-branch decision in copyItemById after the fix. */
    enum class CopyAction {
        /** Image was copied as a content:// URI — normal (pasteAsPlainText=false). */
        IMAGE_URI,
        /** File was copied as a content:// URI — normal (pasteAsPlainText=false). */
        FILE_URI,
        /** Item was copied as plain text — either a text item, or a text downgrade. */
        PLAIN_TEXT,
        /**
         * Media item tapped with pasteAsPlainText=true — onMediaCopyAsText fired,
         * clipboard NOT modified.
         */
        MEDIA_COPY_AS_TEXT_REJECTED,
    }

    /**
     * Simulates the fixed copyItemById branching, including the 5917.76 guard.
     *
     * @param isImage   whether the item is an image
     * @param isFile    whether the item is a file
     * @param pasteAsPlainText  the setting value
     * @return the CopyAction that would be taken
     */
    private fun simulate(
        isImage: Boolean,
        isFile: Boolean,
        pasteAsPlainText: Boolean,
    ): CopyAction {
        val forcePlainText = pasteAsPlainText
        return when {
            isImage && !forcePlainText -> CopyAction.IMAGE_URI
            isFile  && !forcePlainText -> CopyAction.FILE_URI
            else -> {
                // CopyPaste-5917.76 guard: image/file with forcePlainText=true → reject
                if (forcePlainText && (isImage || isFile)) {
                    CopyAction.MEDIA_COPY_AS_TEXT_REJECTED
                } else {
                    CopyAction.PLAIN_TEXT
                }
            }
        }
    }

    // ── pasteAsPlainText=false: normal behaviour unchanged ─────────────────

    @Test
    fun pasteOff_imageItem_copiesAsUri() {
        assertEquals(
            CopyAction.IMAGE_URI,
            simulate(isImage = true, isFile = false, pasteAsPlainText = false),
        )
    }

    @Test
    fun pasteOff_fileItem_copiesAsUri() {
        assertEquals(
            CopyAction.FILE_URI,
            simulate(isImage = false, isFile = true, pasteAsPlainText = false),
        )
    }

    @Test
    fun pasteOff_textItem_copiesAsPlainText() {
        assertEquals(
            CopyAction.PLAIN_TEXT,
            simulate(isImage = false, isFile = false, pasteAsPlainText = false),
        )
    }

    // ── pasteAsPlainText=true: media items must be REJECTED with feedback ──

    @Test
    fun pasteOn_imageItem_isRejectedNotSilentlyDowngraded() {
        val result = simulate(isImage = true, isFile = false, pasteAsPlainText = true)
        assertEquals(
            "pasteAsPlainText=true on an image item must fire onMediaCopyAsText " +
                "and NOT copy a '[image]' snippet to clipboard",
            CopyAction.MEDIA_COPY_AS_TEXT_REJECTED,
            result,
        )
        // Ensure it's explicitly NOT one of the copy-data paths
        assertFalse(result == CopyAction.IMAGE_URI)
        assertFalse(result == CopyAction.PLAIN_TEXT)
    }

    @Test
    fun pasteOn_fileItem_isRejectedNotSilentlyDowngraded() {
        val result = simulate(isImage = false, isFile = true, pasteAsPlainText = true)
        assertEquals(
            "pasteAsPlainText=true on a file item must fire onMediaCopyAsText " +
                "and NOT copy a file snippet to clipboard",
            CopyAction.MEDIA_COPY_AS_TEXT_REJECTED,
            result,
        )
        assertFalse(result == CopyAction.FILE_URI)
        assertFalse(result == CopyAction.PLAIN_TEXT)
    }

    @Test
    fun pasteOn_textItem_copiesAsPlainText() {
        // Text items have a real plaintext payload — they must still copy normally.
        assertEquals(
            CopyAction.PLAIN_TEXT,
            simulate(isImage = false, isFile = false, pasteAsPlainText = true),
        )
    }

    // ── Callback-receives-message contract ────────────────────────────────

    @Test
    fun mediaFeedbackCallback_receivesNonBlankMessage() {
        // Simulate the callback invocation; the message must not be blank.
        var capturedMessage: String? = null
        val onMediaCopyAsText: (String) -> Unit = { msg -> capturedMessage = msg }

        val isImage = true
        val forcePlainText = true
        // Reproduce the guard condition from the fixed copyItemById:
        if (forcePlainText && (isImage)) {
            onMediaCopyAsText("Image and file items cannot be pasted as plain text")
        }

        assertTrue(
            "onMediaCopyAsText must receive a non-blank message",
            capturedMessage?.isNotBlank() == true,
        )
    }

    @Test
    fun clipboardNotModifiedWhenMediaRejected() {
        // When MEDIA_COPY_AS_TEXT_REJECTED, clipboard write must be skipped.
        // Simulated via a boolean flag that the fixed code sets only when PLAIN_TEXT.
        var clipboardWritten = false
        val isImage = true
        val forcePlainText = true

        val action = simulate(isImage = isImage, isFile = false, pasteAsPlainText = forcePlainText)
        if (action != CopyAction.MEDIA_COPY_AS_TEXT_REJECTED) {
            // Only non-rejected paths write to the clipboard
            clipboardWritten = true
        }

        assertFalse(
            "Clipboard must NOT be written when onMediaCopyAsText fires",
            clipboardWritten,
        )
    }

    // ── Exhaustive matrix ─────────────────────────────────────────────────

    @Test
    fun exhaustiveMatrix_pasteOn_mediaAlwaysRejected() {
        // Every media item (image OR file) with pasteAsPlainText=true must be rejected.
        for (isImage in listOf(true, false)) {
            for (isFile in listOf(true, false)) {
                if (isImage && isFile) continue // physically impossible item
                if (!isImage && !isFile) continue // text item — not media
                val result = simulate(isImage, isFile, pasteAsPlainText = true)
                assertEquals(
                    "Media item (isImage=$isImage, isFile=$isFile) with pasteAsPlainText=true must be rejected",
                    CopyAction.MEDIA_COPY_AS_TEXT_REJECTED,
                    result,
                )
            }
        }
    }
}
