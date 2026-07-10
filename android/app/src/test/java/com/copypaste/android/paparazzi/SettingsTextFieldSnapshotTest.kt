package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.SettingsTextField
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// S9 Wave 5 — SettingsTextField normal vs error state (SettingsComponents.kt,
// STYLEGUIDE §9.3 input). [SettingsTextField] is hermetic (plain value/callback
// params, no repository/FFI/Activity), so it can be snapshotted directly
// without a fixture wrapper, mirroring DevicesCardSnapshotTest's CardFixture
// pattern for the theme + background wrapper. A separate focused golden is
// intentionally skipped — the focus ring is a static, default M3 outline
// (de-style convention: don't snapshot framework-owned chrome).
// ---------------------------------------------------------------------------
class SettingsTextFieldSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `dark theme, normal field`() {
        paparazzi.snapshot {
            FieldFixture(isDark = true) {
                SettingsTextField(
                    label = "Sync port",
                    hint = "4242",
                    value = "4242",
                    onValueChange = {},
                )
            }
        }
    }

    @Test
    fun `dark theme, error field`() {
        paparazzi.snapshot {
            FieldFixture(isDark = true) {
                SettingsTextField(
                    label = "Sync port",
                    hint = "4242",
                    value = "99999999",
                    onValueChange = {},
                    isError = true,
                    errorText = "Port must be between 1 and 65535",
                )
            }
        }
    }

    @Test
    fun `light theme, normal field`() {
        paparazzi.snapshot {
            FieldFixture(isDark = false) {
                SettingsTextField(
                    label = "Sync port",
                    hint = "4242",
                    value = "4242",
                    onValueChange = {},
                )
            }
        }
    }

    @Test
    fun `light theme, error field`() {
        paparazzi.snapshot {
            FieldFixture(isDark = false) {
                SettingsTextField(
                    label = "Sync port",
                    hint = "4242",
                    value = "99999999",
                    onValueChange = {},
                    isError = true,
                    errorText = "Port must be between 1 and 65535",
                )
            }
        }
    }
}

@Composable
private fun FieldFixture(isDark: Boolean, content: @Composable () -> Unit) {
    CopyPasteTheme(isDark = isDark) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(LocalCpColors.current.bg)
                .padding(16.dp),
        ) {
            content()
        }
    }
}
