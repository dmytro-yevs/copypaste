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
//   - CopyPaste-9u7l fix: the backdrop refreshes via a BOUNDED, THROTTLED
//     polling loop (`CapturedBackdropBlur`'s `LaunchedEffect`, ~100ms
//     ceiling), not an immediate/synchronous invalidation on scroll — so
//     after `performScrollToIndex` + `waitForIdle` this test also sleeps past
//     that refresh-latency ceiling (`Thread.sleep(2_000)` + a second
//     `waitForIdle`) before sampling `afterScroll`, to avoid a flaky false
//     negative from asserting before the throttled tick has fired. 2s (20x
//     the 100ms ceiling) rather than a tighter margin: a bulk
//     `connectedDebugAndroidTest` run with 26 tests back-to-back is
//     measurably slower per-frame than running this class alone, and both
//     250ms and 500ms margins flaked under that load.
// ---------------------------------------------------------------------------
class BackdropScrollFreshnessConnectedTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val stripeColors = listOf(
        Color(0xFFE53935), Color(0xFFFFB300), Color(0xFF43A047), Color(0xFF1E88E5), Color(0xFF8E24AA),
    )

    @Test(timeout = 30_000)
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

        // 152, not a multiple of `stripeColors.size` (5): scrolling by a
        // multiple of the stripe period would leave every visible pixel color
        // byte-identical to the pre-scroll frame (index % 5 unchanged for
        // every visible row) regardless of whether the backdrop actually
        // refreshed, making the probe below vacuously pass/fail on layout
        // alone rather than on freshness.
        composeRule.onNodeWithTag("scrollContent").performScrollToIndex(152)
        composeRule.waitForIdle()
        // Bounded-latency polling refresh (CopyPaste-9u7l): the throttled tick
        // loop has a ~100ms ceiling, so wait past it before sampling. 2s
        // margin found necessary under bulk-suite load (see class kdoc).
        Thread.sleep(2_000)
        composeRule.waitForIdle()

        val afterScroll = composeRule.onNodeWithTag("navPill").captureToImage().asAndroidBitmap()

        assertFalse(
            "pill backdrop pixels were identical before/after scroll — the captured " +
                "backdrop appears stale instead of re-sampling the scrolled content",
            beforeScroll.sameAs(afterScroll),
        )
    }
}
