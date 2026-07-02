package com.copypaste.android.paparazzi

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.ContentIconTile
import com.copypaste.android.ContentVisualKind
import com.copypaste.android.HistoryDateGroup
import com.copypaste.android.HistoryDateHeaderRow
import com.copypaste.android.HistoryErrorState
import com.copypaste.android.MaskedRowSanitizedOverlay
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-history / android-visual-regression (S5 5.5) — component-level
// goldens for the pieces this slice introduced or rewrote. Deliberately
// hermetic (no ClipboardItem/ClipboardRepository/FFI/Activity — S2.9 Paparazzi
// seam rule): a full "populated HistoryRow" golden would need
// ClipboardRepository as a non-optional constructor parameter, which this
// slice intentionally does NOT instantiate in a JVM/Robolectric golden test
// (see bd notes — tracked as a follow-up needing a repository-free row-body
// extraction). Covers instead:
//   - content-type tiles (§9.4/§3.7), one per major ContentVisualKind incl. SECRET
//   - the pre-API-31 masked-row sanitized-overlay treatment (§"List Masking
//     Contract") in isolation from HistoryRow's SDK-branching
//   - date-group headers (§9.6), all four groups
//   - the NEW error/degraded list state (5.3)
// ---------------------------------------------------------------------------
class HistoryComponentsSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `content-type tiles for every major kind, dark theme`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                val cp = LocalCpColors.current
                Row(modifier = Modifier.padding(12.dp)) {
                    ContentIconTile(kind = ContentVisualKind.TEXT, chipLabel = "TEXT", colors = cp)
                    ContentIconTile(kind = ContentVisualKind.URL, chipLabel = "URL", colors = cp)
                    ContentIconTile(kind = ContentVisualKind.CODE, chipLabel = "CODE", colors = cp)
                    ContentIconTile(kind = ContentVisualKind.JSON, chipLabel = "JSON", colors = cp)
                    ContentIconTile(kind = ContentVisualKind.FILE, chipLabel = "FILE", colors = cp)
                    ContentIconTile(kind = ContentVisualKind.SECRET, chipLabel = "SECRET", colors = cp)
                }
            }
        }
    }

    @Test
    fun `content-type tiles, light theme`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = false) {
                val cp = LocalCpColors.current
                Row(modifier = Modifier.padding(12.dp)) {
                    ContentIconTile(kind = ContentVisualKind.EMAIL, chipLabel = "EMAIL", colors = cp)
                    ContentIconTile(kind = ContentVisualKind.PHONE, chipLabel = "PHONE", colors = cp)
                    ContentIconTile(kind = ContentVisualKind.NUMBER, chipLabel = "NUMBER", colors = cp)
                    ContentIconTile(kind = ContentVisualKind.PATH, chipLabel = "PATH", colors = cp)
                    ContentIconTile(kind = ContentVisualKind.IMAGE, chipLabel = "IMAGE", colors = cp)
                }
            }
        }
    }

    @Test
    fun `masked row sanitized overlay never renders the real secret glyphs`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                MaskedRowSanitizedOverlay(
                    snippet = "4111 1111 1111 1111",
                    isMonoKind = true,
                    previewLines = 1,
                    hiddenDesc = "Sensitive content hidden",
                    colors = MaterialTheme.colorScheme,
                )
            }
        }
    }

    @Test
    fun `date group header, pinned`() {
        paparazzi.snapshot { HeaderFixture(HistoryDateGroup.PINNED) }
    }

    @Test
    fun `date group header, today`() {
        paparazzi.snapshot { HeaderFixture(HistoryDateGroup.TODAY) }
    }

    @Test
    fun `date group header, yesterday`() {
        paparazzi.snapshot { HeaderFixture(HistoryDateGroup.YESTERDAY) }
    }

    @Test
    fun `date group header, earlier`() {
        paparazzi.snapshot { HeaderFixture(HistoryDateGroup.EARLIER) }
    }

    @Test
    fun `error degraded state, dark theme`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                HistoryErrorState(padding = PaddingValues(16.dp), onRetry = {})
            }
        }
    }
}

@Composable
private fun HeaderFixture(group: HistoryDateGroup) {
    CopyPasteTheme(isDark = true) {
        HistoryDateHeaderRow(group = group)
    }
}
