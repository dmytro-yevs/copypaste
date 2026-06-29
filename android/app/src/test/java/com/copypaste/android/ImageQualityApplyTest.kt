package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * crh3.101: image_quality was removed as a documented NO-OP.
 *
 * The old ctmv tests verified that settings.imageQuality drove a JPEG vs PNG branch
 * in captureImageClip. That branch is now gone: the capture path always uses
 * Bitmap.CompressFormat.PNG with quality=100 (lossless, as it always was in practice).
 *
 * This structural test verifies the REMOVAL: no settings.imageQuality reference
 * and no JPEG branch remain in ClipboardService.
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
     * captureImageClip must NOT reference settings.imageQuality after crh3.101 removal.
     */
    @Test
    fun `captureImageClip does not reference removed imageQuality setting`() {
        assertFalse(
            "captureImageClip must not reference settings.imageQuality after crh3.101 removal",
            serviceSource.contains("settings.imageQuality"),
        )
    }

    /**
     * captureImageClip must use PNG lossless capture (always quality=100, no JPEG branch).
     */
    @Test
    fun `captureImageClip uses PNG lossless encode without a JPEG branch`() {
        assertFalse(
            "captureImageClip must not have a JPEG encode branch after crh3.101 removal",
            serviceSource.contains("CompressFormat.JPEG"),
        )
    }
}
