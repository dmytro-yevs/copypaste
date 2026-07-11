package com.copypaste.android.paparazzi

import androidx.activity.compose.LocalActivityResultRegistryOwner
import androidx.activity.result.ActivityResultRegistry
import androidx.activity.result.ActivityResultRegistryOwner
import androidx.activity.result.contract.ActivityResultContract
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.SettingsScreen
import com.copypaste.android.ui.theme.CopyPasteTheme
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// bd CopyPaste-f0f3a.11 — full-screen goldens after removing the outer
// full-screen CopyPasteCard wrapper (top bar + tab row + grouped section
// cards, native mobile structure) so a regression back to the single
// bordered-panel layout is caught. Representative convention: one golden
// per acceptance-listed tab (General/Display/Sync/Storage) plus Storage
// doubling as the long-content/scroll case (it has the most rows of any
// tab), plus one embedded (no back button, no canvas backdrop) fixture for
// the bottom-nav-clearance scenario, matching how MainShell hosts this
// screen. `initialTab` is a test-only seam added to SettingsScreen since
// `selectedTab` is otherwise internal rememberSaveable state.
// ---------------------------------------------------------------------------
class SettingsScreenSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `dark theme, general tab`() {
        paparazzi.snapshot {
            SettingsScreenFixture(initialTab = TAB_GENERAL)
        }
    }

    @Test
    fun `dark theme, display tab`() {
        paparazzi.snapshot {
            SettingsScreenFixture(initialTab = TAB_DISPLAY)
        }
    }

    @Test
    fun `dark theme, sync tab`() {
        paparazzi.snapshot {
            SettingsScreenFixture(initialTab = TAB_SYNC)
        }
    }

    @Test
    fun `dark theme, storage tab, long content`() {
        paparazzi.snapshot {
            SettingsScreenFixture(initialTab = TAB_STORAGE)
        }
    }

    @Test
    fun `dark theme, embedded, bottom nav clearance`() {
        paparazzi.snapshot {
            SettingsScreenFixture(
                initialTab = TAB_GENERAL,
                showBackButton = false,
                paintCanvasBackdrop = false,
            )
        }
    }
}

// Mirrors the private TAB_* constants in SettingsActivity.kt (file-private,
// not visible from this test file).
private const val TAB_GENERAL = 0
private const val TAB_DISPLAY = 1
private const val TAB_SYNC = 2
private const val TAB_STORAGE = 3

// StorageTab's export/import file pickers call rememberLauncherForActivityResult,
// which requires a LocalActivityResultRegistryOwner in the composition — absent
// by default under Paparazzi/Robolectric. This fake never launches (goldens
// don't click the buttons), it only satisfies the composition-local lookup.
private val fakeActivityResultRegistry = object : ActivityResultRegistry() {
    override fun <I, O> onLaunch(
        requestCode: Int,
        contract: ActivityResultContract<I, O>,
        input: I,
        options: androidx.core.app.ActivityOptionsCompat?,
    ) = Unit
}

private val fakeActivityResultRegistryOwner = object : ActivityResultRegistryOwner {
    override val activityResultRegistry: ActivityResultRegistry = fakeActivityResultRegistry
}

@Composable
private fun SettingsScreenFixture(
    initialTab: Int,
    showBackButton: Boolean = true,
    paintCanvasBackdrop: Boolean = true,
) {
    CompositionLocalProvider(LocalActivityResultRegistryOwner.provides(fakeActivityResultRegistryOwner)) {
        CopyPasteTheme(isDark = true) {
            SettingsScreen(
                showBackButton = showBackButton,
                paintCanvasBackdrop = paintCanvasBackdrop,
                initialTab = initialTab,
            )
        }
    }
}
