package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Test

/**
 * CopyPaste-iqhr: captureImageClip and captureFileClip must NOT drop sensitive
 * items. They must let items through to the repository, which (via storeItem /
 * native storeClipboardItem) applies the same sensitive_capture_decision as the
 * text path: store with is_sensitive=true + TTL-based expiry.
 *
 * The previous "BUG 3 fix" in captureImageClip and captureFileClip used
 * isSensitive(uriStr) on the URI path itself — a heuristic that fires for
 * URIs like "content://...passwords.csv" and silently drops the item instead
 * of letting the user's TTL setting govern it. The correct fix is to remove
 * that early-drop gate entirely: URI-based sensitivity is unreliable (the URI
 * contains no plaintext secret; the actual content is what matters), and the
 * text path proves the right pattern is to store-not-drop.
 *
 * These are structural (source-scan) tests because the functions are companion-
 * object suspend funs that require a full Android runtime to execute.
 *
 * Run via: ./gradlew :app:testDebugUnitTest --tests "*.SensitiveCaptureDecisionTest"
 */
class SensitiveCaptureDecisionTest {

    private val serviceSource: String by lazy {
        val anchor = SensitiveCaptureDecisionTest::class.java
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
     * CopyPaste-iqhr: captureImageClip must NOT call isSensitive() on the URI
     * and drop the item — URI-path sensitivity checks are unreliable and wrong.
     * The sensitive_capture_decision is delegated to the repository layer (storeItem).
     */
    @Test
    fun `captureImageClip does not drop items based on isSensitive URI check`() {
        val captureImageBody = serviceSource
            .substringAfter("suspend fun captureImageClip(")
            .substringBefore("suspend fun captureFileClip(")

        // The early-drop guard that calls isSensitive(uriStr) and returns must NOT exist.
        // We allow isSensitive to be called elsewhere (e.g. logging), but the specific
        // drop-on-sensitive-URI pattern must be gone.
        val hasSensitiveUriDrop = captureImageBody.contains("isSensitive(uriStr)") &&
            captureImageBody.contains("skipping capture") &&
            captureImageBody
                .substringAfter("isSensitive(uriStr)")
                .substringBefore("BitmapFactory")
                .contains("return")
        assertFalse(
            "captureImageClip must NOT drop items when isSensitive(uriStr) is true — " +
                "route through storeItem (the same sensitive_capture_decision as text)",
            hasSensitiveUriDrop,
        )
    }

    /**
     * CopyPaste-iqhr: captureFileClip must NOT call isSensitive() on the URI
     * and drop the item. The repository's storeItem handles sensitive classification.
     */
    @Test
    fun `captureFileClip does not drop items based on isSensitive URI check`() {
        val captureFileBody = serviceSource
            .substringAfter("suspend fun captureFileClip(")
            .substringBefore("private fun databasePath(")

        // Same pattern — must NOT have the drop-on-sensitive-URI guard.
        val hasSensitiveUriDrop = captureFileBody.contains("isSensitive(uriStr)") &&
            captureFileBody.contains("skipping capture") &&
            captureFileBody
                .substringAfter("isSensitive(uriStr)")
                .substringBefore("openInputStream")
                .contains("return")
        assertFalse(
            "captureFileClip must NOT drop items when isSensitive(uriStr) is true — " +
                "route through storeItem (the same sensitive_capture_decision as text)",
            hasSensitiveUriDrop,
        )
    }
}
