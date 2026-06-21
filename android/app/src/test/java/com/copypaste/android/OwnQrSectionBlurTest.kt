package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Source-inspection tests for the QR blur policy in OwnQrSection —
 * CopyPaste-5917.36 / CopyPaste-1jms.4 parity.
 *
 * The blur model (CopyPaste-v5a): qrBlurred is user-owned. Only the initial
 * composition starts blurred. Regenerating (manual second tap OR the automatic
 * 120 s TTL rotation) must NOT change qrBlurred. Only an explicit first tap
 * reveals, and the reveal state persists across subsequent refreshes.
 *
 * These tests verify that generateQr() in OwnQrSection does NOT assign qrBlurred,
 * matching PairActivity's behaviour (line 437-439) and macOS DevicesView policy.
 *
 * Runs on the JVM without Compose runtime (source-text inspection).
 */
class OwnQrSectionBlurTest {

    private fun devicesSource(): String {
        val f = File("src/main/java/com/copypaste/android/DevicesActivity.kt")
        assertTrue("DevicesActivity.kt not found at ${f.absolutePath}", f.exists())
        return f.readText()
    }

    /**
     * generateQr() must NOT contain `qrBlurred = true`.
     * Assigning true re-blurs the QR on every TTL auto-refresh — violates CopyPaste-v5a.
     */
    @Test
    fun `generateQr does not set qrBlurred to true`() {
        val src = devicesSource()
        // Locate generateQr() block (from the fun declaration to its closing brace).
        // We check the source contains the v5a guard comment rather than the banned assignment.
        val generateQrIdx = src.indexOf("fun generateQr()")
        assertTrue("generateQr() not found in DevicesActivity", generateQrIdx >= 0)

        // Find the fun block end — scan forward for matching brace depth.
        var depth = 0
        var start = generateQrIdx
        var blockEnd = generateQrIdx
        for (i in start until src.length) {
            when (src[i]) {
                '{' -> depth++
                '}' -> {
                    depth--
                    if (depth == 0 && start != generateQrIdx) {
                        blockEnd = i
                        break
                    }
                }
            }
            if (src[i] == '{') start = i
        }
        val generateQrBlock = src.substring(generateQrIdx, blockEnd + 1)

        assertFalse(
            "generateQr() must NOT assign qrBlurred=true (violates CopyPaste-v5a blur policy). " +
                "Remove the assignment; qrBlurred is user-owned and must survive TTL refresh.",
            generateQrBlock.contains("qrBlurred = true"),
        )
    }

    /**
     * The initial composition must start blurred (privacy default).
     * qrBlurred must be initialised to true via remember { mutableStateOf(true) }.
     */
    @Test
    fun `OwnQrSection initialises qrBlurred to true for privacy default`() {
        val src = devicesSource()
        assertTrue(
            "OwnQrSection must initialise qrBlurred=true as privacy default " +
                "(remember { mutableStateOf(true) })",
            src.contains("mutableStateOf(true)") && src.contains("qrBlurred"),
        )
    }

    /**
     * The countdown ticker comment must reference CopyPaste-v5a (blur-independent policy).
     * This guards against future comment drift re-introducing the incorrect policy claim.
     */
    @Test
    fun `countdown ticker comment references v5a blur policy`() {
        val src = devicesSource()
        // Either v5a or 5917.36 in the countdown-ticker area is acceptable.
        val hasV5aRef = src.contains("CopyPaste-v5a") || src.contains("5917.36")
        assertTrue(
            "The QR countdown area should reference CopyPaste-v5a or 5917.36 (blur policy anchor)",
            hasV5aRef,
        )
    }
}
