package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.ClipboardItem
import com.copypaste.android.PreviewFileContent
import com.copypaste.android.PreviewImageContent
import com.copypaste.android.PreviewImageLoadState
import com.copypaste.android.PreviewTextContent
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-preview S6 golden fixtures (task 6.3 "GOLDENS"): a small,
// representative fixture set (design.md D13/R14 "representative goldens...
// not a full cross-product" — mirrors NavPillSnapshotTest's own restraint) —
// masked vs revealed text, image success, and file meta. Every fixture is
// hermetic/stateless (no repository/FFI/Activity — S2.9 Paparazzi seam rule):
// PreviewTextContent/PreviewImageContent/PreviewFileContent take only plain
// params, so these snapshots call them directly rather than routing through
// PreviewOverlay's repository-backed produceState loading.
//
// SECURITY: the "masked" fixture's synthetic secret must never appear in the
// recorded PNG's pixels in an OCR-legible way for this golden to be safe to
// commit — the fixture relies on the same blur/placeholder path production
// code uses (PreviewTextContent's `masked` branch), so if that path ever
// regresses to rendering plaintext unblurred, this golden's recorded PNG
// would visibly change and the verify gate would catch it.
// ---------------------------------------------------------------------------
class PreviewContentSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `masked sensitive text preview`() {
        paparazzi.snapshot {
            PreviewCardFixture {
                PreviewTextContent(
                    item = sensitiveTextFixture(),
                    fullText = FIXTURE_SECRET,
                    maskSensitive = true,
                    revealed = false,
                    pinned = true,
                )
            }
        }
    }

    @Test
    fun `revealed sensitive text preview`() {
        paparazzi.snapshot {
            PreviewCardFixture {
                PreviewTextContent(
                    item = sensitiveTextFixture(),
                    fullText = FIXTURE_SECRET,
                    maskSensitive = true,
                    revealed = true,
                    pinned = true,
                )
            }
        }
    }

    @Test
    fun `image preview success state`() {
        paparazzi.snapshot {
            PreviewCardFixture {
                PreviewImageContent(
                    state = PreviewImageLoadState.Success(fixtureBitmap()),
                    isSensitive = false,
                    maskSensitive = true,
                    revealed = false,
                    pinned = true,
                    imageScale = 1f,
                    imagePanX = 0f,
                    imagePanY = 0f,
                    onTransform = { _: Float, _: Offset -> },
                )
            }
        }
    }

    @Test
    fun `file preview meta state`() {
        paparazzi.snapshot {
            PreviewCardFixture {
                PreviewFileContent(item = fileFixture())
            }
        }
    }
}

private const val FIXTURE_SECRET = "sk_live_fixture_secret_never_real"

private fun sensitiveTextFixture() = ClipboardItem(
    id = "fixture-secret-1",
    contentType = "text",
    isSensitive = true,
    wallTimeMs = 0L,
    snippet = FIXTURE_SECRET,
)

private fun fileFixture() = ClipboardItem(
    id = "fixture-file-1",
    contentType = "file",
    isSensitive = false,
    wallTimeMs = 0L,
    snippet = "[file: quarterly-report.pdf]",
)

private fun fixtureBitmap() =
    android.graphics.Bitmap.createBitmap(8, 8, android.graphics.Bitmap.Config.ARGB_8888)
        .apply { eraseColor(android.graphics.Color.BLUE) }
        .asImageBitmap()

@Composable
private fun PreviewCardFixture(content: @Composable () -> Unit) {
    CopyPasteTheme(isDark = true) {
        Box(
            modifier = Modifier
                .width(360.dp)
                .height(240.dp)
                .background(LocalCpColors.current.card),
        ) {
            content()
        }
    }
}
