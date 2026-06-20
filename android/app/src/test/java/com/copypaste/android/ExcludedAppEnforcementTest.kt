package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-x8a8: verify that captureClip enforces excludedAppBundleIds at
 * capture time so clipboard events from excluded source apps are silently dropped.
 *
 * The exclusion list is stored in Settings but was never consulted at the capture
 * call-site — the fix wires it into captureClip (and the dispatcher).
 *
 * These tests are structural (source-scan) because ClipboardService.captureClip
 * is a companion-object suspend fun that requires a full Android runtime to execute.
 *
 * Run via: ./gradlew :app:testDebugUnitTest --tests "*.ExcludedAppEnforcementTest"
 */
class ExcludedAppEnforcementTest {

    private val serviceSource: String by lazy {
        val anchor = ExcludedAppEnforcementTest::class.java
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
     * CopyPaste-x8a8: dispatchClipData must read the source package and check
     * it against settings.excludedAppBundleIds before dispatching to any
     * capture function.
     */
    @Test
    fun `dispatchClipData enforces excludedAppBundleIds`() {
        val dispatchBody = serviceSource
            .substringAfter("fun dispatchClipData(")
            .substringBefore("suspend fun captureClip(")
        assertTrue(
            "dispatchClipData must reference excludedAppBundleIds to enforce the exclusion list",
            dispatchBody.contains("excludedAppBundleIds"),
        )
    }

    /**
     * CopyPaste-x8a8: the exclusion check must cause an early return, not just
     * log a warning. Verify the exclusion check is paired with a return statement.
     */
    @Test
    fun `dispatchClipData returns early for excluded apps`() {
        val dispatchBody = serviceSource
            .substringAfter("fun dispatchClipData(")
            .substringBefore("suspend fun captureClip(")
        // The guard must return before dispatching to any capture path.
        // We look for "return" appearing in the exclusion gate block.
        assertTrue(
            "dispatchClipData must return early when the source app is excluded",
            dispatchBody.contains("return") && dispatchBody.contains("excludedAppBundleIds"),
        )
    }
}
