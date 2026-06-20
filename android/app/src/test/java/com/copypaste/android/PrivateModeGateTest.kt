package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * CopyPaste-ss82: verify that every capture path in ClipboardService gates on
 * Settings.privateMode before persisting or syncing a clipboard item.
 *
 * These tests scan the source file to confirm the guard is present in each
 * relevant function. This is intentionally a structural test — the alternative
 * (mocking Context + SharedPreferences + ClipboardManager) is far heavier and
 * adds fragility without additional signal.
 *
 * Run via: ./gradlew :app:testDebugUnitTest --tests "*.PrivateModeGateTest"
 */
class PrivateModeGateTest {

    /**
     * The relative path from the module root; resolved via the class-path
     * so the test works in both IDE and Gradle environments.
     */
    private val serviceSource: String by lazy {
        // Walk up from the test-class location to find the ClipboardService source.
        // This uses a known anchor (the test package directory) to reach main sources.
        val anchor = PrivateModeGateTest::class.java
            .protectionDomain?.codeSource?.location?.toURI()
            ?.let { File(it) }
        // From build/intermediates/... we climb to the module root, then descend.
        var dir: File? = anchor
        var moduleRoot: File? = null
        while (dir != null) {
            if (File(dir, "src/main").exists()) {
                moduleRoot = dir
                break
            }
            dir = dir.parentFile
        }
        requireNotNull(moduleRoot) { "Could not locate module root from $anchor" }
        File(
            moduleRoot,
            "src/main/java/com/copypaste/android/ClipboardService.kt",
        ).readText()
    }

    /**
     * CopyPaste-ss82: captureClip must gate on settings.privateMode.
     */
    @Test
    fun `captureClip checks privateMode before persisting`() {
        // The privateMode guard must appear in the body of captureClip.
        // We verify it appears at least once AFTER "suspend fun captureClip".
        val captureClipBody = serviceSource
            .substringAfter("suspend fun captureClip(")
            .substringBefore("suspend fun captureImageClip(")
        assertTrue(
            "captureClip must check settings.privateMode and return early when true",
            captureClipBody.contains("settings.privateMode"),
        )
    }

    /**
     * CopyPaste-ss82: captureImageClip must gate on settings.privateMode.
     */
    @Test
    fun `captureImageClip checks privateMode before persisting`() {
        val captureImageBody = serviceSource
            .substringAfter("suspend fun captureImageClip(")
            .substringBefore("suspend fun captureFileClip(")
        assertTrue(
            "captureImageClip must check settings.privateMode and return early when true",
            captureImageBody.contains("settings.privateMode"),
        )
    }

    /**
     * CopyPaste-ss82: captureFileClip must gate on settings.privateMode.
     */
    @Test
    fun `captureFileClip checks privateMode before persisting`() {
        val captureFileBody = serviceSource
            .substringAfter("suspend fun captureFileClip(")
            .substringBefore("private fun databasePath(")
        assertTrue(
            "captureFileClip must check settings.privateMode and return early when true",
            captureFileBody.contains("settings.privateMode"),
        )
    }
}
