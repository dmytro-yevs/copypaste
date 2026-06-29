package com.copypaste.android

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.copypaste.android.ui.theme.DarkColors
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Regression test for CopyPaste-crh3.23 / CopyPaste-crh3.55.
 *
 * Sensitive [HistoryRow] content MUST be redacted from the Compose accessibility
 * semantic tree so that TalkBack never announces the plaintext of a sensitive
 * clipboard item (passwords, card numbers, tokens).
 *
 * The production fix:
 *   - API 31+: [Modifier.blur] hides the content visually; [Modifier.clearAndSetSemantics]
 *     replaces the semantic node with a non-sensitive description
 *     ("Sensitive content hidden — tap to reveal"), so the actual snippet text is
 *     never exposed to the a11y framework.
 *   - API < 31: [Modifier.blur] is a no-op there, so `display` is set to the bullet
 *     mask string ("••••••") instead of the real snippet, keeping plaintext out of the
 *     tree by a different route.
 *
 * Both paths are captured by a single assertion:
 *   onNodeWithText(actualSnippet).assertDoesNotExist()
 * (assertDoesNotExist is a method on SemanticsNodeInteraction, not an extension import)
 *
 * Counterpart (non-sensitive item) ensures normal content is NOT inadvertently
 * redacted: onNodeWithText(normalSnippet).assertExists()
 *
 * Type: instrumented (androidTest) — requires a device or emulator.
 * Compile-only verification: ./gradlew :app:compileDebugAndroidTestKotlin
 */
@RunWith(AndroidJUnit4::class)
class SensitiveHistoryRowA11yTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private val context
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    // ── sensitive item: plaintext must NOT appear in the a11y tree ───────────

    @Test
    fun sensitiveItem_maskedEnabled_snippetAbsentFromA11yTree() {
        val secretSnippet = "SuperSecret_Pa55w0rd"
        val sensitiveItem = ClipboardItem(
            id = "test-sensitive",
            contentType = "text/plain",
            isSensitive = true,
            wallTimeMs = 1_700_000_000_000L,
            snippet = secretSnippet,
        )
        val repo = ClipboardRepository(context)

        composeTestRule.setContent {
            // MaterialTheme provides typography/color scheme required by Text() and
            // Material3 composables inside HistoryRow.
            MaterialTheme {
                HistoryRow(
                    item = sensitiveItem,
                    colors = DarkColors,
                    repository = repo,
                    maskSensitive = true,
                    imageMaxHeightDp = 120,
                    previewDelayMs = 30_000L,
                    selectionMode = false,
                    isSelected = false,
                    onDelete = {},
                    onSetPinned = { _, _ -> },
                    onCopy = {},
                    onLongPress = {},
                    onCheckboxTap = {},
                )
            }
        }

        // The plaintext secret must NOT appear in the semantic tree on any API level:
        //   API 31+: clearAndSetSemantics replaces the Text node's semantics with the
        //            hidden label, so text = secretSnippet is no longer in the tree.
        //   API <31: `display` holds the bullet mask "••••••" instead of secretSnippet,
        //            so the semantic text node never contains the real secret.
        // Either way, a TalkBack traversal cannot announce the plaintext password.
        composeTestRule.onNodeWithText(secretSnippet).assertDoesNotExist()
    }

    // ── non-sensitive item: normal content MUST remain accessible ────────────

    @Test
    fun nonSensitiveItem_maskedEnabled_snippetPresentInA11yTree() {
        val normalSnippet = "Hello from clipboard — non-sensitive"
        val normalItem = ClipboardItem(
            id = "test-normal",
            contentType = "text/plain",
            isSensitive = false,
            wallTimeMs = 1_700_000_000_000L,
            snippet = normalSnippet,
        )
        val repo = ClipboardRepository(context)

        composeTestRule.setContent {
            MaterialTheme {
                HistoryRow(
                    item = normalItem,
                    colors = DarkColors,
                    repository = repo,
                    maskSensitive = true,
                    imageMaxHeightDp = 120,
                    previewDelayMs = 30_000L,
                    selectionMode = false,
                    isSelected = false,
                    onDelete = {},
                    onSetPinned = { _, _ -> },
                    onCopy = {},
                    onLongPress = {},
                    onCheckboxTap = {},
                )
            }
        }

        // Non-sensitive items must remain readable so TalkBack users can navigate
        // clipboard history and identify which item they want to copy.
        composeTestRule.onNodeWithText(normalSnippet).assertExists()
    }
}
