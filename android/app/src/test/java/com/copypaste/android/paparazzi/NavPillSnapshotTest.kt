package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.R
import com.copypaste.android.ui.shell.BackdropCaptureState
import com.copypaste.android.ui.shell.NavPill
import com.copypaste.android.ui.shell.NavPillTab
import com.copypaste.android.ui.shell.captureBackdrop
import com.copypaste.android.ui.theme.BlurMode
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.icons.LucideIcons
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-navigation-chrome / android-visual-regression: a small, focused
// golden fixture set for [NavPill] (S4 task 4.2) — deliberately NOT a full
// theme×tab×blurMode cross-product (design.md D13/R14 "representative
// goldens... not a full cross-product"). NavPill is hermetic (no repository/
// FFI/Activity in its params — see its kdoc), so [BlurMode] and the selected
// tab are pinned per-fixture for deterministic, crash-free rendering:
//   - dark / Clips selected / OPAQUE_FALLBACK
//   - light / Devices selected / OPAQUE_FALLBACK
//   - dark / Settings selected / REAL_BACKDROP, rendered over [StripedBackdrop]
//     (S4 review fix — a flat single-color backdrop makes "blur" a
//     mathematical no-op regardless of whether the blur code runs at all;
//     stripes make the captured-layer blur's actual smearing visible in the
//     golden, so this fixture would fail to reproduce if the blur path
//     silently degenerated back into a no-op)
// ---------------------------------------------------------------------------
class NavPillSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `dark theme, Clips selected, opaque fallback`() {
        paparazzi.snapshot {
            NavPillFixture(isDark = true, selectedIndex = 0, blurMode = BlurMode.OPAQUE_FALLBACK)
        }
    }

    @Test
    fun `light theme, Devices selected, opaque fallback`() {
        paparazzi.snapshot {
            NavPillFixture(isDark = false, selectedIndex = 1, blurMode = BlurMode.OPAQUE_FALLBACK)
        }
    }

    @Test
    fun `dark theme, Settings selected, real backdrop blur`() {
        paparazzi.snapshot {
            NavPillFixture(isDark = true, selectedIndex = 2, blurMode = BlurMode.REAL_BACKDROP)
        }
    }
}

private val fixtureTabs = listOf(
    NavPillTab(R.string.title_history, LucideIcons.NavHistory),
    NavPillTab(R.string.title_devices, LucideIcons.NavDevices),
    NavPillTab(R.string.title_settings, LucideIcons.NavSettings),
)

/**
 * High-contrast striped backdrop (pure, stateless — same shape as the S0.5
 * spike's `BlurSpikeActivity.ColorfulBackdrop`) so a blurred vs. non-blurred
 * render of this golden visibly differ: a flat backdrop would pass this
 * fixture whether or not the blur path actually runs.
 */
@Composable
private fun StripedBackdrop(modifier: Modifier = Modifier) {
    val stripeColors = remember {
        listOf(Color(0xFFE53935), Color(0xFFFFB300), Color(0xFF43A047), Color(0xFF1E88E5), Color(0xFF8E24AA))
    }
    Column(modifier = modifier) {
        repeat(24) { index ->
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(16.dp)
                    .background(stripeColors[index % stripeColors.size]),
            )
        }
    }
}

@Composable
private fun NavPillFixture(isDark: Boolean, selectedIndex: Int, blurMode: BlurMode) {
    CopyPasteTheme(isDark = isDark) {
        // Only the REAL_BACKDROP fixture wires a capture source — the
        // OPAQUE_FALLBACK fixtures deliberately leave it null to prove NavPill
        // never fakes a blur when no source is available (see its kdoc).
        val backdropState = remember(blurMode) {
            if (blurMode == BlurMode.REAL_BACKDROP) BackdropCaptureState() else null
        }
        Box(modifier = Modifier.fillMaxSize()) {
            // The capture SOURCE (striped backdrop) must NOT contain NavPill
            // itself — NavPill (via [CapturedBackdropBlur]) is the CONSUMER
            // of the capture, drawn as a sibling outside this subtree,
            // exactly like MainShell's real wiring (content+gradient+badge
            // captured, NavPill drawn after/outside). Nesting NavPill inside
            // the captured subtree would recurse: the source's recording
            // would try to draw the not-yet-finished Picture into itself.
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(LocalCpColors.current.bg)
                    .captureBackdrop(backdropState),
            ) {
                StripedBackdrop(modifier = Modifier.fillMaxSize())
            }
            NavPill(
                tabs = fixtureTabs,
                selectedIndex = selectedIndex,
                onTabSelected = {},
                blurMode = blurMode,
                reducedMotion = true,
                backdropState = backdropState,
                modifier = Modifier,
            )
        }
    }
}
