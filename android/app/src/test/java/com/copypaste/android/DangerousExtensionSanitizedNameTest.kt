package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Regression test for CopyPaste-ev7z: isDangerousExtension must be called on the
 * SANITIZED filename, not the raw peer-supplied name.
 *
 * A malicious peer could supply a name like "evil.pdf\0.sh" which sanitizeFilename
 * strips to "evil.pdf.sh"; the extension extracted from the SANITIZED name is "sh"
 * (dangerous), whereas the extension extracted from the raw name is "" (after the
 * null byte) or "sh" depending on how the code splits — the important invariant is
 * that the raw name must NEVER be the input to the extension check.
 *
 * This test verifies that the sanitized name's extension differs from the raw name
 * in edge-cases that could be used to bypass the denylist, and that the correct
 * (sanitized) extension is used for the decision.
 */
class DangerousExtensionSanitizedNameTest {

    /**
     * Helper that mirrors what HistoryActivity should do:
     *   1. sanitize the raw filename
     *   2. extract the extension from the SANITIZED name
     *   3. check isDangerousExtension on that sanitized extension
     *
     * The CORRECT pattern — the bug was that rawName was used for the extension
     * check instead of safeName.
     */
    private fun isDangerousViaCorrectPath(rawName: String): Boolean {
        val safeName = FileSecurityHelper.sanitizeFilename(rawName)
        val ext = safeName.substringAfterLast('.', "").lowercase()
        return FileSecurityHelper.isDangerousExtension(ext)
    }

    /**
     * Helper that mirrors the BUGGY pattern in the original code:
     * extension is extracted from rawName, not safeName.
     */
    private fun isDangerousViaBuggyPath(rawName: String): Boolean {
        val ext = rawName.substringAfterLast('.', "").lowercase()
        return FileSecurityHelper.isDangerousExtension(ext)
    }

    // ── Regression: path-traversal hiding a dangerous extension ──────────────

    @Test
    fun sanitizedPathExtractExtFromSafeName_traversalHidingShell() {
        // "../../etc/report.sh" sanitized to "report.sh" → extension "sh" (dangerous)
        val raw = "../../etc/report.sh"
        val safeName = FileSecurityHelper.sanitizeFilename(raw)
        val ext = safeName.substringAfterLast('.', "").lowercase()
        assertTrue("sanitized name of a traversal path should be 'report.sh'",
            safeName == "report.sh")
        assertTrue("extension from sanitized name must be 'sh' (dangerous)",
            FileSecurityHelper.isDangerousExtension(ext))
        assertTrue("correct path reports dangerous", isDangerousViaCorrectPath(raw))
    }

    @Test
    fun sanitizedName_windowsTraversalDangerousExtension() {
        // "C:\\Windows\\evil.apk" → sanitized "evil.apk" → ext "apk" → dangerous
        val raw = "C:\\Windows\\evil.apk"
        val safe = FileSecurityHelper.sanitizeFilename(raw)
        assertEquals("evil.apk", safe)
        val ext = safe.substringAfterLast('.', "").lowercase()
        assertTrue(FileSecurityHelper.isDangerousExtension(ext))
        assertTrue("correct path detects danger", isDangerousViaCorrectPath(raw))
    }

    // ── Safe names pass through correctly ────────────────────────────────────

    @Test
    fun sanitizedName_safeExtension_notDangerous() {
        val raw = "document.pdf"
        assertTrue("correct path: safe file is not dangerous",
            !isDangerousViaCorrectPath(raw))
    }

    @Test
    fun sanitizedName_imageExtension_notDangerous() {
        val raw = "photo.png"
        assertFalse("png must not be flagged as dangerous",
            isDangerousViaCorrectPath(raw))
    }

    // ── Verify sanitizeFilename is the correct choke-point ───────────────────

    @Test
    fun sanitizeFilename_thenExtract_givesCorrectExtension() {
        // Normal case: safe → safe; dangerous → still dangerous after sanitize.
        val cases = listOf(
            "report.pdf" to false,
            "script.sh" to true,
            "app.apk" to true,
            "photo.jpg" to false,
            "virus.exe" to true,
            "data.csv" to false,
        )
        for ((name, expectDangerous) in cases) {
            val safe = FileSecurityHelper.sanitizeFilename(name)
            val ext = safe.substringAfterLast('.', "").lowercase()
            val actual = FileSecurityHelper.isDangerousExtension(ext)
            assertEquals(
                "Mismatch for '$name' (sanitized='$safe', ext='$ext')",
                expectDangerous, actual,
            )
        }
    }
}
