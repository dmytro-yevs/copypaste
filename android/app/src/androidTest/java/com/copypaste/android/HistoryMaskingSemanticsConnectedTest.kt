package com.copypaste.android

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.printToString
import com.copypaste.android.ui.theme.CopyPasteTheme
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertFalse
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-history S5 5.4 connected masked-secrecy check (spec.md "List
// Masking Contract" + Gates: "connected semantics/a11y... masked-secrecy
// merged+unmerged... mandatory local run for security-relevant slices (S5,
// S6)"). Mirrors `PreviewMaskedSecrecyConnectedTest` (S6)'s methodology.
//
// Per the project's "connected-test CI availability" resolved decision, this
// run is REQUIRED LOCALLY for S5 (`:app:connectedDebugAndroidTest`) — CI stays
// advisory-only until CopyPaste-k1l0. No emulator is available in this
// sandbox; this class is written so it COMPILES
// (`:app:compileDebugAndroidTestKotlin`) and is ready for the pending local
// emulator run (bd-noted as outstanding).
//
// Exercises the PUBLIC `HistoryScreen` entry point (not the internal
// HistoryRow/MaskedRowSanitizedOverlay renderers) with a REAL on-device
// `ClipboardRepository`/native encryption — Kotlin `internal` visibility is
// not friend-linked from the `androidTest` variant to `main` in this module's
// Gradle config (see `PreviewMaskedSecrecyConnectedTest`'s kdoc), so calling
// the internal renderers directly from here would not compile. A real
// sensitive item is stored through `ClipboardRepository.storeItem` BEFORE
// `HistoryScreen` mounts, then picked up by `ClipboardViewModel.loadItems()`
// through the SAME on-device `SharedPreferences` store (both the seeding
// repository and the ViewModel's own repository resolve to the same
// `context.applicationContext`), so this exercises the full real pipeline
// (not a snippet-fallback shortcut) — HIGHER fidelity than the Preview
// equivalent, at the cost of depending on the on-device sensitive-detection
// classifier actually flagging the seeded string (a well-known Luhn-valid
// test card number, chosen specifically because it is a canonical
// sensitive-detection positive across every classifier this project uses).
// ---------------------------------------------------------------------------
class HistoryMaskingSemanticsConnectedTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val secretPlaintext = "4111 1111 1111 1111"

    @Test
    fun maskedHistoryRowNeverExposesPlaintextInAnySemanticsNode() {
        val activity = composeRule.activity
        val repository = ClipboardRepository(activity)
        val settings = Settings(activity)

        runBlocking {
            repository.storeItem(
                plaintext = secretPlaintext,
                key = settings.encryptionKey,
                contentType = "text/plain",
            )
        }

        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                // Default `viewModel = viewModel()` resolves through
                // `LocalViewModelStoreOwner` (the ComponentActivity) — its
                // internal ClipboardRepository resolves the SAME
                // `context.applicationContext` SharedPreferences store the
                // seeding repository above just wrote to.
                HistoryScreen(showBackButton = false)
            }
        }
        composeRule.waitForIdle()

        // spec.md "List Masking Contract": check the COMPLETE semantics dump
        // (merged AND unmerged), not just a single node's merged
        // contentDescription — a plaintext leak in an unmerged child node
        // would otherwise go undetected.
        val mergedDump = composeRule.onRoot(useUnmergedTree = false).printToString()
        val unmergedDump = composeRule.onRoot(useUnmergedTree = true).printToString()

        assertFalse("merged semantics tree leaked plaintext", mergedDump.contains(secretPlaintext))
        assertFalse("unmerged semantics tree leaked plaintext", unmergedDump.contains(secretPlaintext))
    }
}
