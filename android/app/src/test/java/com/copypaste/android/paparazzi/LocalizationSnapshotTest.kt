package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.R
import com.copypaste.android.ui.theme.BannerVariant
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CpBanner
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// CopyPaste-myh8.13 S13 Wave (e) — locale/scale regression coverage for the
// shared CpBanner (ui/theme/Banner.kt), reusing SyncTabBannerSnapshotTest's
// BannerFixture setup and wiring (same rationale: SyncTab.kt itself needs
// ~20 constructor params only wired at the Activity level). Each class here
// swaps ONE dimension off the DeviceConfig.PIXEL_5 baseline (locale="uk",
// long-string stress content, fontScale=2.0f) to catch text overflow/clipping
// and missing-translation regressions that a single-locale golden can't see.
// One Paparazzi rule per class, matching the sibling *SnapshotTest convention
// — Paparazzi's rule config is fixed at field-construction time, so
// multi-variant coverage needs multiple small classes, not one rule reused
// with different configs.
// ---------------------------------------------------------------------------

class LocalizationUkSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5.copy(locale = "uk"),
        maxPercentDifference = 0.0,
    )

    @Test
    fun `uk locale, auth error banner`() {
        paparazzi.snapshot {
            BannerFixture {
                CpBanner(
                    message = stringResource(
                        R.string.sync_error_unauthorized,
                        "401 Unauthorized",
                    ),
                    variant = BannerVariant.ERROR,
                )
            }
        }
    }

    @Test
    fun `uk locale, cloud mismatch banner`() {
        paparazzi.snapshot {
            BannerFixture {
                CpBanner(
                    message = stringResource(
                        R.string.setting_cloud_account_mismatch_title,
                    ) + "\n" + stringResource(R.string.setting_cloud_account_mismatch_body),
                    variant = BannerVariant.INFO,
                )
            }
        }
    }
}

class LocalizationLongStringSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `en locale, long string stress banner`() {
        paparazzi.snapshot {
            BannerFixture {
                CpBanner(
                    message = LONG_STRESS_MESSAGE,
                    variant = BannerVariant.WARN,
                    actions = {
                        CopyPasteButton(onClick = {}, variant = ButtonVariant.GHOST) {
                            Text(stringResource(R.string.btn_retry))
                        }
                    },
                )
            }
        }
    }
}

class LocalizationFontScaleSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5.copy(fontScale = 2.0f),
        maxPercentDifference = 0.0,
    )

    @Test
    fun `200 percent font scale, generic error banner`() {
        paparazzi.snapshot {
            BannerFixture {
                CpBanner(
                    message = stringResource(
                        R.string.sync_error_unauthorized,
                        "401 Unauthorized",
                    ),
                    variant = BannerVariant.ERROR,
                    actions = {
                        CopyPasteButton(onClick = {}, variant = ButtonVariant.GHOST) {
                            Text(stringResource(R.string.btn_retry))
                        }
                    },
                )
            }
        }
    }
}

private const val LONG_STRESS_MESSAGE =
    "Relay sync failed for device \"Dmytro's Extremely Long Corporate Workstation " +
        "Name (Finance Department, Building 7, Floor 12, Desk 4471-B)\". Verify the " +
        "relay URL and that the relay server is running, then retry the connection " +
        "from every paired device before contacting support."

@Composable
private fun BannerFixture(content: @Composable () -> Unit) {
    CopyPasteTheme(isDark = true) {
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
