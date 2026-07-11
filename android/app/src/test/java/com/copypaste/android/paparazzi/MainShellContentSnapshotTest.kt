package com.copypaste.android.paparazzi

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.ClipboardItem
import com.copypaste.android.ClipboardRepository
import com.copypaste.android.HistoryDateHeaderRow
import com.copypaste.android.HistoryListBody
import com.copypaste.android.HistoryListEntry
import com.copypaste.android.HistoryRow
import com.copypaste.android.buildHistoryListEntries
import com.copypaste.android.ui.shell.MainShellContent
import com.copypaste.android.ui.shell.NavTab
import com.copypaste.android.ui.theme.BlurMode
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-navigation-chrome / android-visual-regression (f0f3a.8) — a small,
// curated golden fixture set for [MainShellContent], deliberately NOT a full
// theme×tab×blurMode×fontScale cross-product (design.md D13/R14
// "representative goldens... not a full cross-product"). MainShellContent is
// hermetic (no ClipboardViewModel/repository/FFI in its params — see its
// kdoc): DEVICES/SETTINGS content slots below are trivial non-ViewModel
// stubs, not the real DevicesScreen/SettingsScreen, since those screens are
// out of scope for this seam. Curated set:
//   - dark, CLIPS, empty history state
//   - dark, CLIPS, populated (real HistoryRow fixtures, shared with
//     HistoryListSnapshotTest's populated golden)
//   - light, CLIPS, populated (theme axis, without crossing every tab)
//   - dark, DEVICES selected, nav visible, trivial stub content
//   - dark, SETTINGS selected, nav visible, trivial stub content
//   - fontScale=2.0 stress, CLIPS populated (separate class: Paparazzi's
//     DeviceConfig is a JUnit @Rule field, fixed per test class)
// ---------------------------------------------------------------------------

/**
 * Real [ClipboardItem] fixtures spanning text/URL/image/sensitive content
 * types plus a PINNED/TODAY/YESTERDAY/EARLIER time spread, shared with
 * [HistoryListSnapshotTest]'s populated golden.
 */
private fun mainShellFixtureItems(nowMs: Long): List<ClipboardItem> = listOf(
    ClipboardItem(
        id = "pinned-text",
        contentType = "text",
        isSensitive = false,
        wallTimeMs = nowMs - 60_000L,
        snippet = "Pinned reminder: ship the release notes",
        pinned = true,
        pinnedSortIndex = 0,
    ),
    ClipboardItem(
        id = "today-url",
        contentType = "url",
        isSensitive = false,
        wallTimeMs = nowMs - 3_600_000L,
        snippet = "https://example.com/docs/getting-started",
    ),
    ClipboardItem(
        id = "yesterday-image",
        contentType = "image",
        isSensitive = false,
        wallTimeMs = nowMs - 90_000_000L,
        snippet = "[image]",
    ),
    ClipboardItem(
        id = "earlier-sensitive",
        contentType = "text",
        isSensitive = true,
        wallTimeMs = nowMs - 300_000_000L,
        snippet = "4111 1111 1111 1111",
    ),
)

/** Real HistoryListBody + HistoryRow content, mirroring HistoryList.kt's fold. */
@Composable
private fun MainShellPopulatedClipsContent(repository: ClipboardRepository, bottomContentPadding: Dp) {
    val nowMs = System.currentTimeMillis()
    val entries = buildHistoryListEntries(mainShellFixtureItems(nowMs), nowMs)
    val colors = MaterialTheme.colorScheme
    val cpColors = LocalCpColors.current
    HistoryListBody(
        padding = PaddingValues(bottom = bottomContentPadding),
        loading = false,
        hasAnyItems = true,
        hasFilteredItems = true,
        isDegraded = false,
        isPrivateMode = false,
        searchQuery = "",
        onRetry = {},
    ) {
        Column {
            entries.forEach { entry ->
                when (entry) {
                    is HistoryListEntry.Header -> HistoryDateHeaderRow(group = entry.group)
                    is HistoryListEntry.Row -> HistoryRow(
                        item = entry.item,
                        colors = colors,
                        cpColors = cpColors,
                        repository = repository,
                        maskSensitive = false,
                        imageMaxHeightDp = 160,
                        previewDelayMs = 0L,
                        previewLines = 1,
                        selectionMode = false,
                        isSelected = false,
                        onDelete = {},
                        onSetPinned = { _, _ -> },
                        onLongPress = {},
                        onCheckboxTap = {},
                    )
                }
            }
        }
    }
}

@Composable
private fun MainShellEmptyClipsContent(bottomContentPadding: Dp) {
    HistoryListBody(
        padding = PaddingValues(bottom = bottomContentPadding),
        loading = false,
        hasAnyItems = false,
        hasFilteredItems = false,
        isDegraded = false,
        isPrivateMode = false,
        searchQuery = "",
        onRetry = {},
    ) {}
}

/** Trivial non-ViewModel stand-in — DEVICES/SETTINGS content is out of this seam's scope. */
@Composable
private fun MainShellStubTabContent() {
    Box(modifier = Modifier.fillMaxSize())
}

class MainShellContentSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    private fun shellFixture(
        isDark: Boolean,
        selectedTab: NavTab,
        clipsContent: @Composable (Dp) -> Unit = { MainShellEmptyClipsContent(it) },
    ) {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = isDark) {
                MainShellContent(
                    selectedTab = selectedTab,
                    onTabSelected = {},
                    blurMode = BlurMode.OPAQUE_FALLBACK,
                    reducedMotion = false,
                    imeVisible = false,
                    sideOffset = 12.dp,
                    bottomOffset = 24.dp,
                    pillHeightDp = 74.dp,
                    onPillHeightChanged = {},
                    backdropState = null,
                    clipsContent = clipsContent,
                    devicesContent = { MainShellStubTabContent() },
                    settingsContent = { MainShellStubTabContent() },
                )
            }
        }
    }

    @Test
    fun `dark shell, clips tab, empty history, nav visible`() {
        shellFixture(isDark = true, selectedTab = NavTab.CLIPS)
    }

    @Test
    fun `dark shell, clips tab, populated history, nav visible`() {
        val repository = ClipboardRepository(paparazzi.context)
        shellFixture(isDark = true, selectedTab = NavTab.CLIPS) {
            MainShellPopulatedClipsContent(repository, it)
        }
    }

    @Test
    fun `light shell, clips tab, populated history, nav visible`() {
        val repository = ClipboardRepository(paparazzi.context)
        shellFixture(isDark = false, selectedTab = NavTab.CLIPS) {
            MainShellPopulatedClipsContent(repository, it)
        }
    }

    @Test
    fun `dark shell, devices tab selected, nav visible`() {
        shellFixture(isDark = true, selectedTab = NavTab.DEVICES)
    }

    @Test
    fun `dark shell, settings tab selected, nav visible`() {
        shellFixture(isDark = true, selectedTab = NavTab.SETTINGS)
    }
}

class MainShellContentFontScaleSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5.copy(fontScale = 2.0f),
        maxPercentDifference = 0.0,
    )

    @Test
    fun `200 percent font scale, clips tab populated`() {
        val repository = ClipboardRepository(paparazzi.context)
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                MainShellContent(
                    selectedTab = NavTab.CLIPS,
                    onTabSelected = {},
                    blurMode = BlurMode.OPAQUE_FALLBACK,
                    reducedMotion = false,
                    imeVisible = false,
                    sideOffset = 12.dp,
                    bottomOffset = 24.dp,
                    pillHeightDp = 74.dp,
                    onPillHeightChanged = {},
                    backdropState = null,
                    clipsContent = { bottomContentPadding ->
                        MainShellPopulatedClipsContent(repository, bottomContentPadding)
                    },
                    devicesContent = {},
                    settingsContent = {},
                )
            }
        }
    }
}
