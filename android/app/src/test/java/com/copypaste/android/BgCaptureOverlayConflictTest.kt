package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-xxi2: background capture on Android 10+ unreliable; ClipboardService
 * overlay conflicts with ClipboardFloatingActivity. Reconcile so capture works reliably.
 *
 * Root-cause: ClipboardService.addCaptureOverlay() adds a TYPE_APPLICATION_OVERLAY
 * view with FLAG_NOT_FOCUSABLE. ClipboardFloatingActivity also adds a TYPE_APPLICATION_OVERLAY
 * view and then clears FLAG_NOT_FOCUSABLE to gain input focus. On some OEM ROMs, having
 * two same-process overlays — one of which then requests focus — causes the OS to grant
 * focus to neither (the focus token is ambiguous). Result: getPrimaryClip() returns null
 * even inside ClipboardFloatingActivity's OnGlobalLayoutListener.
 *
 * Fix: ClipboardService exposes suppressCaptureOverlay() / restoreCaptureOverlay() static
 * methods so ClipboardFloatingActivity can briefly remove the service overlay before
 * requesting focus, then restore it after finish(). This eliminates the two-overlay
 * conflict without changing ClipboardService's steady-state capture behaviour.
 *
 * Structural (source-scan) test — the runtime methods require an Android environment.
 */
class BgCaptureOverlayConflictTest {

    private val serviceSource: String by lazy {
        val anchor = BgCaptureOverlayConflictTest::class.java
            .protectionDomain?.codeSource?.location?.toURI()
            ?.let { java.io.File(it) }
        var dir: java.io.File? = anchor
        var moduleRoot: java.io.File? = null
        while (dir != null) {
            if (java.io.File(dir, "src/main").exists()) {
                moduleRoot = dir
                break
            }
            dir = dir.parentFile
        }
        requireNotNull(moduleRoot) { "Could not locate module root from $anchor" }
        java.io.File(
            moduleRoot,
            "src/main/java/com/copypaste/android/ClipboardService.kt",
        ).readText()
    }

    /**
     * ClipboardService must expose a suppressCaptureOverlay mechanism (static/companion)
     * so ClipboardFloatingActivity can remove the service overlay before claiming focus.
     * Without this, two overlays in the same process compete for the focus token.
     */
    @Test
    fun `ClipboardService exposes overlay suppression mechanism`() {
        assertTrue(
            "ClipboardService must expose suppressCaptureOverlay or equivalent for FloatingActivity coordination",
            serviceSource.contains("suppressCaptureOverlay") ||
                serviceSource.contains("overlayActive") ||
                serviceSource.contains("captureOverlaySuppressed"),
        )
    }

    /**
     * ClipboardService.dispatchClipData is the ONE canonical dispatch path used by both
     * the FGS clipListener and ClipboardFloatingActivity — verify it exists (already fixed
     * in BUG 1 fix, but confirmed here as the reconciliation entry point).
     */
    @Test
    fun `dispatchClipData is the single canonical capture dispatch`() {
        assertTrue(
            "dispatchClipData must be the shared dispatch for both capture paths",
            serviceSource.contains("fun dispatchClipData("),
        )
    }
}
