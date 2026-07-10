package com.copypaste.android.paparazzi

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material3.Text
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.HistoryListBody
import com.copypaste.android.ui.theme.CopyPasteTheme
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// CopyPaste-ci3u — repository-free goldens for HistoryScreen's list-body
// `when` seam ([HistoryListBody], S2.9 Paparazzi seam), extracted precisely
// so it can be exercised in a JVM golden test without a
// ClipboardRepository/Activity/FFI dependency (see HistoryComponentsSnapshotTest's
// kdoc for why a full populated-row golden is a separate follow-up: this
// class covers the populated slot with a plain fixture, not a real HistoryRow).
// ---------------------------------------------------------------------------
class HistoryListSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    private val padding = PaddingValues(16.dp)

    @Test
    fun `loading state, dark theme`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                HistoryListBody(
                    padding = padding,
                    loading = true,
                    hasAnyItems = false,
                    hasFilteredItems = false,
                    isDegraded = false,
                    isPrivateMode = false,
                    searchQuery = "",
                    onRetry = {},
                ) {}
            }
        }
    }

    @Test
    fun `empty history, non-private, dark theme`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                HistoryListBody(
                    padding = padding,
                    loading = false,
                    hasAnyItems = false,
                    hasFilteredItems = false,
                    isDegraded = false,
                    isPrivateMode = false,
                    searchQuery = "",
                    onRetry = {},
                ) {}
            }
        }
    }

    @Test
    fun `empty history, private mode, dark theme`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                HistoryListBody(
                    padding = padding,
                    loading = false,
                    hasAnyItems = false,
                    hasFilteredItems = false,
                    isDegraded = false,
                    isPrivateMode = true,
                    searchQuery = "",
                    onRetry = {},
                ) {}
            }
        }
    }

    @Test
    fun `history error, degraded state, dark theme`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                HistoryListBody(
                    padding = padding,
                    loading = false,
                    hasAnyItems = false,
                    hasFilteredItems = false,
                    isDegraded = true,
                    isPrivateMode = false,
                    searchQuery = "",
                    onRetry = {},
                ) {}
            }
        }
    }

    @Test
    fun `no search results, dark theme`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                HistoryListBody(
                    padding = padding,
                    loading = false,
                    hasAnyItems = true,
                    hasFilteredItems = false,
                    isDegraded = false,
                    isPrivateMode = false,
                    searchQuery = "xyz",
                    onRetry = {},
                ) {}
            }
        }
    }

    @Test
    fun `populated content slot, dark theme`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                HistoryListBody(
                    padding = padding,
                    loading = false,
                    hasAnyItems = true,
                    hasFilteredItems = true,
                    isDegraded = false,
                    isPrivateMode = false,
                    searchQuery = "",
                    onRetry = {},
                ) {
                    Text("fixture row")
                }
            }
        }
    }
}
