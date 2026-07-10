package com.copypaste.android.paparazzi

import androidx.compose.runtime.Composable
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.AboutScreen
import com.copypaste.android.ui.theme.AccentColor
import com.copypaste.android.ui.theme.CopyPasteTheme
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// CopyPaste-myh8.11 S11 W4 — golden fixtures for AboutScreen after adding the
// build-number label, license line and gradient brand mark. Representative
// convention (design.md D13/R14): one dark/INDIGO fixture (the app default)
// plus one light/BLUE fixture to also exercise the light-theme text ramp and
// a non-default accent for the brand-mark gradient — not a full theme x
// accent cross-product.
// ---------------------------------------------------------------------------
class AboutScreenSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `dark theme, indigo accent`() {
        paparazzi.snapshot {
            AboutScreenFixture(isDark = true, accent = AccentColor.INDIGO)
        }
    }

    @Test
    fun `light theme, blue accent`() {
        paparazzi.snapshot {
            AboutScreenFixture(isDark = false, accent = AccentColor.BLUE)
        }
    }
}

@Composable
private fun AboutScreenFixture(isDark: Boolean, accent: AccentColor) {
    CopyPasteTheme(isDark = isDark, accent = accent) {
        AboutScreen()
    }
}
