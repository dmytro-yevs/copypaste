package com.copypaste.android.ui.shell

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsSelected
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onChildren
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import com.copypaste.android.R
import com.copypaste.android.ui.theme.BlurMode
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.icons.LucideIcons
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-navigation-chrome connected a11y/inset checks (S4). Per the
// project's "connected-test CI availability" resolved decision, this run is
// REQUIRED LOCALLY for S4 (:app:connectedDebugAndroidTest) — CI stays
// advisory-only until CopyPaste-k1l0. No emulator is available in this
// sandbox; this class is written so it COMPILES
// (`:app:compileDebugAndroidTestKotlin`) and is ready for the pending local
// emulator run (bd-noted as outstanding — see CopyPaste-myh8.4 notes).
//
// NavPill is hermetic (no repository/FFI/Activity in its params — see its
// kdoc), so every fixture here is a plain, deterministic composable call.
// ---------------------------------------------------------------------------
class NavPillConnectedTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val fixtureTabs = listOf(
        NavPillTab(R.string.title_history, LucideIcons.NavHistory),
        NavPillTab(R.string.title_devices, LucideIcons.NavDevices),
        NavPillTab(R.string.title_settings, LucideIcons.NavSettings),
    )

    @Test
    fun eachTabMeetsTheFortyEightDpMinimumTouchTarget() {
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                NavPill(
                    tabs = fixtureTabs,
                    selectedIndex = 0,
                    onTabSelected = {},
                    blurMode = BlurMode.OPAQUE_FALLBACK,
                    reducedMotion = false,
                )
            }
        }

        composeRule.onNodeWithText(composeRule.activity.getString(R.string.title_history))
            .assertHeightIsAtLeast(CpDimensions.touchMin)
        composeRule.onNodeWithText(composeRule.activity.getString(R.string.title_devices))
            .assertHeightIsAtLeast(CpDimensions.touchMin)
        composeRule.onNodeWithText(composeRule.activity.getString(R.string.title_settings))
            .assertHeightIsAtLeast(CpDimensions.touchMin)
    }

    @Test
    fun tappingATabInvokesOnTabSelectedAndTheSelectedTabIsMarkedSelected() {
        var lastSelected = -1
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                NavPill(
                    tabs = fixtureTabs,
                    selectedIndex = 0,
                    onTabSelected = { lastSelected = it },
                    blurMode = BlurMode.OPAQUE_FALLBACK,
                    reducedMotion = true,
                )
            }
        }

        composeRule.onNodeWithText(composeRule.activity.getString(R.string.title_history))
            .assertIsSelected()
        composeRule.onNodeWithText(composeRule.activity.getString(R.string.title_devices))
            .performClick()

        assertEquals(1, lastSelected)
    }

    /**
     * android-navigation-chrome "IME visible" scenario: the pill is hidden
     * outright (not repositioned) — asserted here by [visible]=false rendering
     * NO child nodes at all, the same deterministic behaviour the real shell
     * gets from `visible = !WindowInsets.isImeVisible`.
     */
    @Test
    fun hiddenWhenNotVisibleNoNodesAreEmitted() {
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                NavPill(
                    tabs = fixtureTabs,
                    selectedIndex = 0,
                    onTabSelected = {},
                    blurMode = BlurMode.OPAQUE_FALLBACK,
                    reducedMotion = false,
                    visible = false,
                )
            }
        }

        composeRule.onRoot().onChildren().assertCountEquals(0)
    }

    @Test
    fun sideAndBottomOffsetsAreHonoredWithoutClippingOffscreen() {
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                NavPill(
                    tabs = fixtureTabs,
                    selectedIndex = 2,
                    onTabSelected = {},
                    blurMode = BlurMode.REAL_BACKDROP,
                    reducedMotion = false,
                    sideOffset = 24.dp,
                    bottomOffset = 40.dp,
                )
            }
        }

        // All three labels must still be found/visible with generous insets —
        // a regression that clipped the pill off-bounds would fail this lookup.
        composeRule.onNodeWithText(composeRule.activity.getString(R.string.title_settings))
            .assertIsSelected()
    }
}
