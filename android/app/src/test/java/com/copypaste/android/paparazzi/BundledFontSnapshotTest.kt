package com.copypaste.android.paparazzi

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// S0.4/S2.5 zero-production-code proof snapshot (android-visual-regression
// "Version compatibility is proven before adoption" scenario): renders one
// bundled-font fixture (CpTypography.title -> the bundled Inter 700 face,
// S1.3) through app.cash.paparazzi 1.3.4 on the pinned toolchain (AGP 8.3.0 /
// Kotlin 1.9.23 / Compose compiler 1.5.11), Pixel-class device, API 34,
// portrait, locale en (design.md "Resolved decisions" / S0.6 golden device
// config). This establishes the baseline dir/naming and record/verify
// mechanism every later slice's golden fixtures build on:
//   - baseline dir: android/app/src/test/snapshots/images/ (Paparazzi's
//     default location — matches design.md's "direct PNG in git, no LFS").
//   - record: (cd android && ./gradlew :app:recordPaparazziDebug -x buildCargoNdk)
//   - verify: (cd android && ./gradlew :app:verifyPaparazziDebug -x buildCargoNdk)
//   - diff threshold: maxPercentDifference = 0.0 (android-visual-regression
//     "the default target is pixel-level (0% differing pixels) and any
//     nonzero tolerance requires named-owner approval" — no owner has
//     approved a nonzero tolerance, so this stays exact).
//   - never-auto-accept: CI runs verifyPaparazziDebug only (never
//     recordPaparazziDebug); a changed/added baseline PNG is reviewed like
//     any other diff in the PR (task 2.7 CI wiring, cross-platform-parity.md
//     "Baseline Update Approval Gate").
// ---------------------------------------------------------------------------
class BundledFontSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `bundled Inter 700 title renders deterministically`() {
        paparazzi.snapshot {
            BundledFontFixture()
        }
    }
}

@Composable
private fun BundledFontFixture() {
    CopyPasteTheme(isDark = true) {
        Text(
            text = "CopyPaste",
            style = CpTypography.title,
            color = LocalCpColors.current.text,
            modifier = Modifier.padding(24.dp),
        )
    }
}
