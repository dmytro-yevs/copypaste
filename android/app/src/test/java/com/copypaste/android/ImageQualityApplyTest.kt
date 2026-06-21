package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-ctmv: imageQuality slider writes a pref but capture hardcoded PNG@100.
 *
 * Root-cause: ClipboardService.captureImageClip re-encoded the decoded bitmap with
 * Bitmap.CompressFormat.PNG and quality=100, ignoring Settings.imageQuality entirely.
 * PNG ignores the quality parameter anyway (always lossless), so even passing a lower
 * value would not reduce file size.
 *
 * Fix: Read settings.imageQuality at encode time. When quality < 100, switch to JPEG
 * (which honours the 1–99 quality value); quality == 100 keeps PNG (lossless) as before.
 *
 * Structural (source-scan) test — verifies the production code references the
 * settings.imageQuality property and the JPEG branch rather than hardcoding PNG@100.
 * Runtime image-encode tests require the Android Bitmap codec which is not available
 * in the JVM unit-test environment.
 */
class ImageQualityApplyTest {

    private val serviceSource: String by lazy {
        val anchor = ImageQualityApplyTest::class.java
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
     * captureImageClip must read settings.imageQuality (not hardcode quality=100).
     */
    @Test
    fun `captureImageClip reads imageQuality from settings`() {
        assertTrue(
            "captureImageClip must read settings.imageQuality to honour the user's quality pref",
            serviceSource.contains("settings.imageQuality"),
        )
    }

    /**
     * captureImageClip must not hardcode Bitmap.CompressFormat.PNG with quality 100.
     * After the fix, quality 100 still uses PNG but the value comes from settings,
     * so the literal "PNG, 100" pattern must not appear.
     */
    @Test
    fun `captureImageClip does not hardcode PNG at quality 100`() {
        // The old implementation always called bitmap.compress(PNG, 100, baos).
        // After the fix the format and quality come from variables (encodeFormat, encodeQuality).
        assertFalse(
            "captureImageClip must not hardcode CompressFormat.PNG, 100 — quality must come from settings",
            serviceSource.contains("CompressFormat.PNG, 100"),
        )
    }

    /**
     * captureImageClip must have a JPEG branch so quality < 100 produces smaller files.
     */
    @Test
    fun `captureImageClip includes a JPEG encode branch`() {
        assertTrue(
            "captureImageClip must include a JPEG encode path for quality < 100",
            serviceSource.contains("CompressFormat.JPEG") ||
                serviceSource.contains("encodeFormat") && serviceSource.contains("useJpeg"),
        )
    }
}
