package com.copypaste.android

import androidx.activity.ComponentActivity
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.printToString
import com.copypaste.android.ui.theme.CopyPasteTheme
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-preview S6 connected masked-secrecy check (spec.md "Preview Masking
// Parity" + Gates: "connected semantics/a11y... masked-secrecy merged+
// unmerged... mandatory local run for security-relevant slices (S5, S6)").
//
// Per the project's "connected-test CI availability" resolved decision, this
// run is REQUIRED LOCALLY for S6 (:app:connectedDebugAndroidTest) — CI stays
// advisory-only until CopyPaste-k1l0. No emulator is available in this
// sandbox; this class is written so it COMPILES
// (`:app:compileDebugAndroidTestKotlin`) and is ready for the pending local
// emulator run (bd-noted as outstanding).
//
// Exercises the PUBLIC `PreviewOverlay` entry point (not the internal
// PreviewTextContent/PreviewImageContent renderers) with a real on-device
// ClipboardRepository/Settings — Kotlin `internal` visibility is not
// friend-linked from the `androidTest` variant to `main` by default in this
// module's Gradle config, so calling the internal renderers directly from
// here would not compile; PreviewOverlay is the only public S6 entry point.
// The repository/settings never have this item's ciphertext stored, so the
// `loadFullPlaintext`/`getImageBytes` lookups miss and PreviewTextContent
// falls back to `item.snippet` (see PreviewContent.kt's `displayText` when-
// chain) — the snippet already carries the synthetic secret used below, so
// the masked-secrecy assertion is exercised the same way it would be for a
// real stored item.
// ---------------------------------------------------------------------------
class PreviewMaskedSecrecyConnectedTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val secretPlaintext = "sk_live_super_secret_998877_do_not_leak"

    private fun sensitiveTextItem() = ClipboardItem(
        id = "preview-secrecy-text-1",
        contentType = "text",
        isSensitive = true,
        wallTimeMs = System.currentTimeMillis(),
        snippet = secretPlaintext,
    )

    // S6 fix round: mirrors [sensitiveTextItem] for the IMAGE kind — no stored
    // image bytes means PreviewImageContent never reaches Success (falls back
    // to the masked lock placeholder / failure state), but [snippet] still
    // carries a synthetic secret so this guards against any future code path
    // that surfaces it (e.g. a filename/caption) as a semantics leak.
    private fun sensitiveImageItem() = ClipboardItem(
        id = "preview-secrecy-image-1",
        contentType = "image/png",
        isSensitive = true,
        wallTimeMs = System.currentTimeMillis(),
        snippet = secretPlaintext,
    )

    private fun setPreviewContent(item: ClipboardItem, onReveal: (() -> Unit)? = null) {
        composeRule.setContent {
            val ctx = LocalContext.current
            // CopyPaste-myh8 gate: lint's RememberReturnType check misresolves this
            // identical, already-widespread `remember { MainSourceSetClass(ctx) }`
            // pattern (see HistoryList.kt/PairScreen.kt/etc.) as Unit-returning ONLY
            // when the call site lives in the androidTest source set — a lint
            // cross-sourceSet UAST resolution false positive, not a real Unit remember.
            @Suppress("RememberReturnType")
            val repository: ClipboardRepository = remember { ClipboardRepository(ctx) }
            @Suppress("RememberReturnType")
            val settings: Settings = remember { Settings(ctx) }
            CopyPasteTheme(isDark = true) {
                PreviewOverlay(
                    phase = PreviewPhase.Pinned,
                    item = item,
                    repository = repository,
                    settings = settings,
                    maskSensitive = true,
                    onDismiss = {},
                    onCopy = {},
                    onSetPinned = {},
                    onDelete = {},
                    onSaveFile = {},
                )
            }
        }
    }

    @Test
    fun maskedPreviewNeverExposesPlaintextInAnySemanticsNode() {
        setPreviewContent(sensitiveTextItem())
        composeRule.waitForIdle()

        // spec.md "Masked text preview hides plaintext from semantics":
        // check the COMPLETE semantics dump (merged AND unmerged), not just
        // the merged contentDescription a single node reports.
        val mergedDump = composeRule.onRoot(useUnmergedTree = false).printToString()
        val unmergedDump = composeRule.onRoot(useUnmergedTree = true).printToString()

        assertFalse("merged semantics tree leaked plaintext", mergedDump.contains(secretPlaintext))
        assertFalse("unmerged semantics tree leaked plaintext", unmergedDump.contains(secretPlaintext))
    }

    @Test
    fun maskedImagePreviewNeverExposesPlaintextInAnySemanticsNode() {
        setPreviewContent(sensitiveImageItem())
        composeRule.waitForIdle()

        // spec.md "Masked image preview: no plaintext description while masked" —
        // same merged+unmerged dump discipline as the text case above.
        val mergedDump = composeRule.onRoot(useUnmergedTree = false).printToString()
        val unmergedDump = composeRule.onRoot(useUnmergedTree = true).printToString()

        assertFalse("merged semantics tree leaked plaintext for a masked image", mergedDump.contains(secretPlaintext))
        assertFalse("unmerged semantics tree leaked plaintext for a masked image", unmergedDump.contains(secretPlaintext))
    }

    @Test
    fun revealActionUnmasksAndPlaintextThenAppearsInSemantics() {
        setPreviewContent(sensitiveTextItem())
        composeRule.waitForIdle()

        // Sanity check the test methodology itself: before Reveal, the
        // toolbar shows the Reveal action (not Copy) for a sensitive item.
        composeRule.onNodeWithContentDescription(
            composeRule.activity.getString(R.string.action_reveal),
        ).performClick()
        composeRule.waitForIdle()

        // spec.md "Reveal action unmasks a sensitive item in Preview": once
        // revealed, the real plaintext DOES appear — proving the masked-state
        // assertion above is a real redaction, not an unrelated render failure.
        val mergedDump = composeRule.onRoot(useUnmergedTree = false).printToString()
        assertTrue("revealed preview should expose plaintext", mergedDump.contains(secretPlaintext))
    }
}
