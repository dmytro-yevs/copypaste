package com.copypaste.android

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Pure-JVM structural guards for two UI repositioning fixes:
 *  1. The "About" entry is the LAST row of the General section in Settings.
 *  2. The "Expires in N s" countdown sits INSIDE the grey QR [CopyPasteCard].
 *
 * These assert source structure (not rendered layout) so they run under
 * `./gradlew testDebugUnitTest` without an emulator. They fail before the
 * repositioning edits and pass after.
 */
class UiRepositionTest {

    private fun source(relative: String): String {
        val f = File("src/main/java/com/copypaste/android/$relative")
        assertTrue("missing source file: ${f.absolutePath}", f.exists())
        return f.readText()
    }

    @Test
    fun aboutIsLastRowOfGeneralSection() {
        val src = source("SettingsActivity.kt")
        // An About nav row must exist inside the General tab.
        val aboutIdx = src.indexOf("AboutActivity::class.java")
        assertTrue("About nav row not found in SettingsActivity", aboutIdx >= 0)

        // It must come AFTER the diagnostics section (the previously-last block),
        // i.e. About is the last General-section entry.
        val diagnosticsIdx = src.indexOf("section_diagnostics")
        assertTrue("diagnostics section not found", diagnosticsIdx >= 0)
        assertTrue(
            "About row must appear after the diagnostics section (be last in General)",
            aboutIdx > diagnosticsIdx
        )
    }

    @Test
    fun expiresCountdownIsInsideQrCard() {
        val src = source("PairActivity.kt")
        // CopyPasteCard may be called with or without modifier arguments — match either.
        // E.g.: `CopyPasteCard {` or `CopyPasteCard(modifier = …) {`
        val cardStart = run {
            val idx1 = src.indexOf("CopyPasteCard {")
            val idx2 = src.indexOf("CopyPasteCard(")
            when {
                idx1 >= 0 && idx2 >= 0 -> minOf(idx1, idx2)
                idx1 >= 0 -> idx1
                idx2 >= 0 -> idx2
                else -> -1
            }
        }
        assertTrue("CopyPasteCard (grey QR block) not found", cardStart >= 0)

        // The CopyPasteCard lambda is the grey block. The closing brace that ends
        // the OutlinedButton-preceding card is the first top-level "}\n            }"
        // — instead of brace-matching, assert the expires label appears before the
        // scan button that immediately follows the card.
        val expiresIdx = src.indexOf("pair_token_expires_in_seconds")
        // The scan button call (not the fun definition) immediately follows the card.
        val scanButtonIdx = src.indexOf("onClick = { startScanFlow() }")
        assertTrue("expires-in label not found", expiresIdx >= 0)
        assertTrue("scan button not found", scanButtonIdx >= 0)
        assertTrue(
            "Expires-in countdown must be rendered before the scan button " +
                "(i.e. inside the QR card, not below it)",
            expiresIdx in (cardStart + 1) until scanButtonIdx
        )
    }
}
