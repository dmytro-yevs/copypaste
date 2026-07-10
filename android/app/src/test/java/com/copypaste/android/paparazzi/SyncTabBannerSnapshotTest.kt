package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
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
import androidx.compose.material3.Text
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// CopyPaste-myh8.11 S11 Wave 2 — SyncTab's sync-error and cloud-mismatch
// banners, migrated off ad-hoc Cards onto the shared CpBanner (ui/theme/
// Banner.kt). [com.copypaste.android.SyncTab] itself requires ~20 constructor
// params (backend enums, every credential field, callbacks) that only exist
// wired to Settings/DevicesOnlineState at the Activity level — reproducing
// its full composable graph here would smother the banner signal in
// unrelated form fields. So this fixture calls CpBanner directly with the
// SAME variant + message/action wiring SyncTab now uses (see SyncTab.kt's
// syncError/syncErrorIsUnauthorized and detectCloudAccountMismatch blocks),
// mirroring StorageTabLoadingSnapshotTest's "representative golden"
// convention. Smell flag: if SyncTab's banner wiring changes, update this
// fixture in lockstep (see kdoc, not a hidden landmine).
// ---------------------------------------------------------------------------
class SyncTabBannerSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `dark theme, auth error banner`() {
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
    fun `dark theme, generic error banner with retry`() {
        paparazzi.snapshot {
            BannerFixture {
                CpBanner(
                    message = "Relay sync failed. Verify the relay URL and that the relay server is running.",
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

    @Test
    fun `dark theme, cloud mismatch banner`() {
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
