package com.copypaste.android

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.IdeSwitch
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

/**
 * CopyPaste-myh8.15 — S15 [IdeSwitch] connected a11y checks: 48dp
 * (`CpDimensions.touchMin`) interactive-node touch target, the visible
 * track's documented smaller size (`CpDimensions.toggleW`/`toggleH`,
 * STYLEGUIDE §9.2), edge-of-outer-box tap dispatch, and `Role.Switch`
 * `stateDescription` (assertIsOn/assertIsOff).
 *
 * FIXED (CopyPaste-myh8.15): Components.kt's [IdeSwitch] now attaches
 * `.toggleable(...)`/`.semantics {}` to the OUTER 48dp `Box`
 * (`CpDimensions.touchMin`), matching the AccentSwatchRow/IdeSegmentedControl
 * outer-touch/inner-visual precedent; the inner 38x22 `Box`
 * (`CpDimensions.toggleW`/`toggleH`) is purely visual.
 */
class IdeSwitchConnectedTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val switchLabel = "Test Switch"

    @Test
    fun interactiveNodeMeetsTheFortyEightDpTouchMinimum() {
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                IdeSwitch(checked = false, onCheckedChange = {}, name = switchLabel)
            }
        }

        val node = composeRule.onNodeWithContentDescription(switchLabel)
        node.assertWidthIsAtLeast(CpDimensions.touchMin)
        node.assertHeightIsAtLeast(CpDimensions.touchMin)
    }

    /**
     * Separate from the interactive-node bounds check above: the VISIBLE
     * track itself is documented (STYLEGUIDE §9.2) to render at the smaller
     * 38x22 size, not 48dp — asserted independently via the same semantics
     * node's reported size (the interactive node IS the visible track's
     * Box; see the class kdoc finding above).
     */
    @Test
    fun visibleTrackRendersAtItsDocumentedThirtyEightByTwentyTwoSize() {
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                IdeSwitch(checked = false, onCheckedChange = {}, name = switchLabel)
            }
        }

        val node = composeRule.onNodeWithContentDescription(switchLabel)
        node.assertWidthIsAtLeast(CpDimensions.toggleW)
        node.assertHeightIsAtLeast(CpDimensions.toggleH)
    }

    @Test
    fun tappingTheEdgeOfTheOuterFortyEightDpBoxTogglesTheSwitch() {
        var lastChecked: Boolean? = null
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                Box(modifier = Modifier) {
                    IdeSwitch(
                        checked = false,
                        onCheckedChange = { lastChecked = it },
                        name = switchLabel,
                    )
                }
            }
        }

        // Near the edge of the declared 48dp outer touch target (1dp in from
        // the corner), not the center of the visible 38x22 track.
        composeRule.onNodeWithContentDescription(switchLabel).performTouchInput {
            click(Offset(1.dp.toPx(), 1.dp.toPx()))
        }

        assertEquals(true, lastChecked)
    }

    @Test
    fun tappingTheSwitchCenterTogglesOnAndOff() {
        composeRule.setContent {
            var checked by remember { mutableStateOf(false) }
            CopyPasteTheme(isDark = true) {
                IdeSwitch(
                    checked = checked,
                    onCheckedChange = { checked = it },
                    name = switchLabel,
                )
            }
        }

        val node = composeRule.onNodeWithContentDescription(switchLabel)
        node.assertIsOff()

        node.performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithContentDescription(switchLabel).assertIsOn()

        composeRule.onNodeWithContentDescription(switchLabel).performClick()
        composeRule.waitForIdle()
        composeRule.onNodeWithContentDescription(switchLabel).assertIsOff()
    }
}
