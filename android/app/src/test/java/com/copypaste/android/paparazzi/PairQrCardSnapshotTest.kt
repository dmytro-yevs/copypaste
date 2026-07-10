package com.copypaste.android.paparazzi

import android.graphics.Bitmap
import android.graphics.Color as AndroidColor
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.PairQrCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-pairing golden fixtures for PairQrCard (S8 task 8.1). Deliberately
// NOT a full theme x state cross-product (design.md D13/R14) — one
// representative fixture per QR-area state, all hermetic (no repository/FFI/
// Activity; [fakeQrBitmap] is an obviously-synthetic checkerboard, never real
// pairing material — see PairQrCard's own SECURITY kdoc).
// ---------------------------------------------------------------------------
class PairQrCardSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `loading state`() {
        paparazzi.snapshot {
            PairQrCardFixture(
                loading = true,
                qrBitmap = null,
                hasQr = false,
                expired = false,
                qrRevealed = false,
                remainingSeconds = 0,
            )
        }
    }

    @Test
    fun `blurred at rest`() {
        paparazzi.snapshot {
            PairQrCardFixture(
                loading = false,
                qrBitmap = fakeQrBitmap(),
                hasQr = true,
                expired = false,
                qrRevealed = false,
                remainingSeconds = 90,
            )
        }
    }

    @Test
    fun `revealed near expiry warning`() {
        paparazzi.snapshot {
            PairQrCardFixture(
                loading = false,
                qrBitmap = fakeQrBitmap(),
                hasQr = true,
                expired = false,
                qrRevealed = true,
                remainingSeconds = 15,
            )
        }
    }

    @Test
    fun `expired offers regenerate`() {
        paparazzi.snapshot {
            PairQrCardFixture(
                loading = false,
                qrBitmap = null,
                hasQr = true,
                expired = true,
                qrRevealed = true,
                remainingSeconds = 0,
            )
        }
    }
}

/** Obviously-synthetic checkerboard — never real pairing material in a golden. */
private fun fakeQrBitmap(): Bitmap {
    val size = 21
    val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
    for (y in 0 until size) {
        for (x in 0 until size) {
            val on = (x + y) % 2 == 0
            bmp.setPixel(x, y, if (on) AndroidColor.BLACK else AndroidColor.WHITE)
        }
    }
    return bmp
}

@Composable
private fun PairQrCardFixture(
    loading: Boolean,
    qrBitmap: Bitmap?,
    hasQr: Boolean,
    expired: Boolean,
    qrRevealed: Boolean,
    remainingSeconds: Int,
) {
    CopyPasteTheme(isDark = true) {
        Box(Modifier.padding(16.dp)) {
            PairQrCard(
                loading = loading,
                qrBitmap = qrBitmap,
                hasQr = hasQr,
                expired = expired,
                qrRevealed = qrRevealed,
                remainingSeconds = remainingSeconds,
                onTap = {},
            )
        }
    }
}
