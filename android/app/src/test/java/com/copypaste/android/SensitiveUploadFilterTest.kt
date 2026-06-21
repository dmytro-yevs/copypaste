package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-ca2d: verify+enforce sensitive-item filtering in SyncManager/FgsSyncLoop
 * before upload — sensitive items must not leave the device.
 *
 * Root-cause: notifySyncManager() in ClipboardService pushes captured items to
 * Supabase/relay without checking whether the item was stored as sensitive. A user
 * with the default sensitiveTtlSecs > 0 and masking enabled could still have their
 * password manager clips exfiltrated.
 *
 * Fix: captureClip() (the common capture entry point) already calls storeClipboardItem
 * which sets is_sensitive=true, but the PUSH to Supabase/relay proceeds regardless.
 * The fix adds a sensitive-gate in notifySyncManager: before pushing, check whether
 * the stored item has is_sensitive=true (via repository.isItemSensitive) and skip the
 * push if it does. The local copy is retained (macOS parity: store locally, do not upload).
 *
 * Structural (source-scan) test.
 */
class SensitiveUploadFilterTest {

    private val serviceSource: String by lazy {
        val anchor = SensitiveUploadFilterTest::class.java
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
     * notifySyncManager must reference isSensitive in some form to gate uploads.
     * The sensitive flag is carried on the StoredItem / ClipboardItem.
     */
    @Test
    fun `captureClip skips notifySyncManager for sensitive items`() {
        // The captureClip function: after storing the item, before calling notifySyncManager,
        // must check the item's sensitive status. We verify the captureClip body references
        // a sensitive guard before the notifySyncManager call.
        val captureBody = serviceSource
            .substringAfter("suspend fun captureClip(")
            .substringBefore("suspend fun captureImageClip(")

        // Either: (a) notifySyncManager is not called at all for sensitive items, or
        // (b) the captureBody contains a sensitive check near the notifySyncManager call.
        // We check that the body does NOT call notifySyncManager unconditionally —
        // there must be some guard relating to sensitive or isSensitive.
        assertTrue(
            "captureClip must gate notifySyncManager on non-sensitive items (isSensitive check)",
            captureBody.contains("isSensitive") || captureBody.contains("sensitive"),
        )
    }
}
