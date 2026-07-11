package com.copypaste.android.paparazzi

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material3.MaterialTheme
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
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// CopyPaste-ci3u / CopyPaste-f0f3a.4 — repository-free goldens for
// HistoryScreen's list-body `when` seam ([HistoryListBody], S2.9 Paparazzi
// seam), extracted precisely so it can be exercised in a JVM golden test
// without an Activity/FFI dependency. `ClipboardRepository` IS constructed
// here (via `paparazzi.context`) because its constructor only touches
// SharedPreferences/Settings — no FFI/network I/O — so it stays hermetic
// under Robolectric (fresh in-memory prefs per test, nothing persists
// across tests); it is only actually read by [HistoryRow]'s image-kind
// thumbnail lookup, which gracefully no-ops on a cache miss.
// ---------------------------------------------------------------------------
class HistoryListSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    private val padding = PaddingValues(16.dp)

    /**
     * Real [ClipboardItem] fixtures spanning text/URL/code/image/sensitive
     * content types plus a PINNED/TODAY/YESTERDAY/EARLIER time spread, so
     * the populated golden below exercises a real [HistoryDateHeaderRow]
     * the way [com.copypaste.android.HistoryList] actually renders rows,
     * not just flat rows.
     */
    private fun fixtureItems(nowMs: Long): List<ClipboardItem> = listOf(
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
            id = "today-code",
            contentType = "code",
            isSensitive = false,
            wallTimeMs = nowMs - 3_700_000L,
            snippet = "fun main() = println(\"hello\")",
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
        val repository = ClipboardRepository(paparazzi.context)
        val nowMs = System.currentTimeMillis()
        // Fixture items are already pinned-first / recency-descending within
        // each bucket, matching [buildHistoryListEntries]'s sorted-input
        // contract, so the fold below produces the same header/row sequence
        // the real HistoryList would for this data.
        val entries = buildHistoryListEntries(fixtureItems(nowMs), nowMs)
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
                    val colors = MaterialTheme.colorScheme
                    val cpColors = LocalCpColors.current
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
        }
    }
}
