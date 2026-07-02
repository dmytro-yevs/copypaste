package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.R
import com.copypaste.android.ui.shell.NavPill
import com.copypaste.android.ui.shell.NavPillTab
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
//   - dark / Settings selected / REAL_BACKDROP (proves the captured-layer
//     blur path renders under Paparazzi's software layoutlib renderer)
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

@Composable
private fun NavPillFixture(isDark: Boolean, selectedIndex: Int, blurMode: BlurMode) {
    CopyPasteTheme(isDark = isDark) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(LocalCpColors.current.bg),
        ) {
            NavPill(
                tabs = fixtureTabs,
                selectedIndex = selectedIndex,
                onTabSelected = {},
                blurMode = blurMode,
                reducedMotion = true,
                modifier = Modifier,
            )
        }
    }
}
