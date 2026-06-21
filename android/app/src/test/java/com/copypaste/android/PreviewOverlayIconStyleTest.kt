package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * CopyPaste-5917.23: PreviewOverlay must use Icons.Outlined.* not Icons.Filled.*.
 *
 * Root cause: PreviewOverlay.kt imported and rendered 8 Icons.Filled.* icons
 * (AttachFile, BookmarkAdded, BookmarkBorder, Close, ContentCopy, Delete, OpenInNew,
 * SaveAlt). The app-wide styleguide requires Icons.Outlined.* for all decorative/action
 * icons — HistoryActivity and CopyPasteTopBar use Outlined exclusively.
 *
 * Fix: replaced every Icons.Filled.* import and usage in PreviewOverlay.kt with
 * the Icons.Outlined.* equivalent.
 *
 * These tests verify the source-file invariant via static analysis on the Kotlin source.
 * They complement but do not replace visual review.
 */
class PreviewOverlayIconStyleTest {

    private val previewOverlaySrc: String by lazy {
        val candidates = listOf(
            "android/app/src/main/java/com/copypaste/android/PreviewOverlay.kt",
            "../android/app/src/main/java/com/copypaste/android/PreviewOverlay.kt",
            "../../android/app/src/main/java/com/copypaste/android/PreviewOverlay.kt",
        )
        candidates
            .map { File(it) }
            .firstOrNull { it.exists() }
            ?.readText()
            ?: error("Could not locate PreviewOverlay.kt from test working directory")
    }

    @Test
    fun noFilledIconImports() {
        assertFalse(
            "PreviewOverlay.kt must not import any Icons.Filled.* — use Icons.Outlined.* instead",
            previewOverlaySrc.contains("import androidx.compose.material.icons.filled."),
        )
    }

    @Test
    fun noFilledIconUsages() {
        assertFalse(
            "PreviewOverlay.kt must not reference Icons.Filled.* anywhere — use Icons.Outlined.*",
            previewOverlaySrc.contains("Icons.Filled."),
        )
    }

    @Test
    fun hasOutlinedIconImports() {
        assertTrue(
            "PreviewOverlay.kt must import at least one Icons.Outlined.* icon",
            previewOverlaySrc.contains("import androidx.compose.material.icons.outlined."),
        )
    }

    @Test
    fun hasOutlinedClose() {
        // Close (X) button in the overlay header.
        assertTrue(
            "PreviewOverlay.kt must use Icons.Outlined.Close",
            previewOverlaySrc.contains("Icons.Outlined.Close"),
        )
    }

    @Test
    fun hasOutlinedContentCopy() {
        // Copy action button.
        assertTrue(
            "PreviewOverlay.kt must use Icons.Outlined.ContentCopy",
            previewOverlaySrc.contains("Icons.Outlined.ContentCopy"),
        )
    }

    @Test
    fun hasOutlinedDelete() {
        // Delete action button.
        assertTrue(
            "PreviewOverlay.kt must use Icons.Outlined.Delete",
            previewOverlaySrc.contains("Icons.Outlined.Delete"),
        )
    }

    @Test
    fun hasOutlinedAttachFile() {
        // Attachment / file-type placeholder icon.
        assertTrue(
            "PreviewOverlay.kt must use Icons.Outlined.AttachFile",
            previewOverlaySrc.contains("Icons.Outlined.AttachFile"),
        )
    }
}
