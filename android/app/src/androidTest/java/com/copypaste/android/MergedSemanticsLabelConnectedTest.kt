package com.copypaste.android

import androidx.activity.ComponentActivity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.SharedSettingsRow
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

/**
 * CopyPaste-myh8.15 — S15 merged-semantics label check for
 * [SharedSettingsRow] (Components.kt ~378, `.semantics(mergeDescendants =
 * true)`): the MERGED node must expose the row's title exactly ONCE, not
 * duplicated across the row's own `Text(title)` child (merges into
 * `SemanticsProperties.Text`) and `IdeSwitch`'s `contentDescription = title`
 * child (merges into `SemanticsProperties.ContentDescription`) — a screen
 * reader concatenating both properties would otherwise announce the title
 * twice for a single row.
 */
class MergedSemanticsLabelConnectedTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val title = "Mask sensitive content"
    private val subtitle = "Hide card numbers and passwords in history"

    @Test
    fun mergedRowExposesTheTitleExactlyOnceAcrossTextAndContentDescription() {
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                SharedSettingsRow(
                    title = title,
                    subtitle = subtitle,
                    checked = false,
                    onCheckedChange = {},
                )
            }
        }

        val mergedNode = composeRule.onNodeWithText(title, substring = true)
            .fetchSemanticsNode()
        val config = mergedNode.config

        val textOccurrences = config.getOrNull(SemanticsProperties.Text)
            .orEmpty()
            .count { it.text == title }
        val contentDescriptionOccurrences = config.getOrNull(SemanticsProperties.ContentDescription)
            .orEmpty()
            .count { it == title }

        assertEquals(
            "expected the merged node to carry the title exactly ONCE across " +
                "SemanticsProperties.Text + SemanticsProperties.ContentDescription " +
                "(found $textOccurrences Text + $contentDescriptionOccurrences " +
                "ContentDescription occurrence(s))",
            1,
            textOccurrences + contentDescriptionOccurrences,
        )
    }
}
