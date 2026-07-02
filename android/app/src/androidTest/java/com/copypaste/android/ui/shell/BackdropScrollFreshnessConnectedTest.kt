package com.copypaste.android.ui.shell

import android.os.Build
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.unit.dp
import com.copypaste.android.R
import com.copypaste.android.ui.theme.BlurMode
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.icons.LucideIcons
import org.junit.Assert.assertFalse
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-history S5 carried task (b) — "scroll-freshness": the floating
// pill's captured backdrop must show non-stale pixels after the content
// behind it scrolls (MainShell D7 edge-to-edge fix — task (a) in this same
// slice — is what makes real content, not just the gradient fade, actually
// reach the captured region in the first place).
//
// METHODOLOGY / DOCUMENTED LIMITS (best-effort design, per this task's own
// instruction to document limits, not a claim of full coverage):
//   - Uses the PUBLIC `NavPill`/`BackdropCaptureState`/`captureBackdrop`
//     building blocks directly (same public surface `NavPillConnectedTest`/
//     `NavPillSnapshotTest` already exercise) with a synthetic tall
//     multi-colored `LazyColumn` standing in for MainShell's real screen
//     content — this test does NOT render the real `HistoryScreen`/
//     `MainShell` (that would need a live repository/ViewModel and is
//     exercised separately); it isolates the ONE thing this task is actually
//     about: does the captured-layer blur consumer re-sample fresh pixels
//     after a scroll, or does it visibly freeze/stale-cache.
//   - `Bitmap.sameAs` is a whole-bitmap comparison — this is a coarse,
//     semantics-independent PROBE (pixels differ or they don't), not a
//     structural/streaming diff; a false negative (bitmaps identical despite
//     genuinely fresh capture) is possible only if the scroll happened to
//     land on visually-identical content, which the alternating stripe
//     palette below is specifically chosen to avoid.
//   - REAL_BACKDROP blur is API 31+ only (`RenderEffect`) — this test is
//     skipped (not failed) below that API via `assumeTrue`.
//   - No emulator is available in this sandbox; this class is written so it
//     COMPILES (`:app:compileDebugAndroidTestKotlin`) and is ready for the
//     pending local emulator run (bd-noted as outstanding, mirrors every
//     other connected test in this slice/wave).
// ---------------------------------------------------------------------------
class BackdropScrollFreshnessConnectedTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val stripeColors = listOf(
        Color(0xFFE53935), Color(0xFFFFB300), Color(0xFF43A047), Color(0xFF1E88E5), Color(0xFF8E24AA),
    )

    @Test
    fun pillBackdropShowsFreshPixelsAfterAProgrammaticScroll() {
        assumeTrue(
            "REAL_BACKDROP blur (RenderEffect) requires API 31+",
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.S,
        )

        val fixtureTabs = listOf(
            NavPillTab(R.string.title_history, LucideIcons.NavHistory),
            NavPillTab(R.string.title_devices, LucideIcons.NavDevices),
            NavPillTab(R.string.title_settings, LucideIcons.NavSettings),
        )

        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                // CopyPaste-myh8 gate: lint's RememberReturnType check misresolves this
                // identical, already-widespread `remember { BackdropCaptureState() }`
                // pattern (see MainShell.kt) as Unit-returning ONLY when the call site
                // lives in the androidTest source set — a lint cross-sourceSet UAST
                // resolution false positive, not a real Unit remember.
                @Suppress("RememberReturnType")
                val backdropState: BackdropCaptureState = remember { BackdropCaptureState() }
                Box(modifier = Modifier.fillMaxSize()) {
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .captureBackdrop(backdropState),
                    ) {
                        LazyColumn(modifier = Modifier.fillMaxSize().testTag("scrollContent")) {
                            items(200) { index ->
                                Box(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(48.dp)
                                        .background(stripeColors[index % stripeColors.size]),
                                )
                            }
                        }
                    }
                    NavPill(
                        tabs = fixtureTabs,
                        selectedIndex = 0,
                        onTabSelected = {},
                        blurMode = BlurMode.REAL_BACKDROP,
                        reducedMotion = true,
                        backdropState = backdropState,
                        modifier = Modifier.testTag("navPill"),
                    )
                }
            }
        }
        composeRule.waitForIdle()

        val beforeScroll = composeRule.onNodeWithTag("navPill").captureToImage().asAndroidBitmap()

        composeRule.onNodeWithTag("scrollContent").performScrollToIndex(150)
        composeRule.waitForIdle()

        val afterScroll = composeRule.onNodeWithTag("navPill").captureToImage().asAndroidBitmap()

        assertFalse(
            "pill backdrop pixels were identical before/after scroll — the captured " +
                "backdrop appears stale instead of re-sampling the scrolled content",
            beforeScroll.sameAs(afterScroll),
        )
    }
}
