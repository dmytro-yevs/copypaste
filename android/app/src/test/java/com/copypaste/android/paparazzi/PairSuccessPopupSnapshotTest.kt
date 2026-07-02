package com.copypaste.android.paparazzi

import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.PairedPeer
import com.copypaste.android.PairedSuccessPopup
import com.copypaste.android.ui.theme.CopyPasteTheme
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-pairing golden fixtures for PairedSuccessPopup (S8 task 8.1). Two
// representative fixtures (design.md D13/R14 "representative, not a full
// cross-product"): full peer metadata, and the legacy/pre-ABI-14 peer with no
// metadata (all the optional peerModel/peerOs/peerAppVersion rows absent).
// The fingerprint below is an obviously-synthetic all-zero placeholder, never
// real pairing material.
// ---------------------------------------------------------------------------
class PairSuccessPopupSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `full peer metadata`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                PairedSuccessPopup(
                    peer = PairedPeer(
                        fingerprint = "0".repeat(64),
                        syncAddr = "192.0.2.10:7420",
                        name = "Fixture Device",
                        sessionKeyWrappedB64 = "",
                        sessionKeyIvB64 = "",
                        peerModel = "Pixel 8",
                        peerOs = "Android 15",
                        peerAppVersion = "0.4.0",
                    ),
                    onDismiss = {},
                )
            }
        }
    }

    @Test
    fun `legacy peer no metadata`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = false) {
                PairedSuccessPopup(
                    peer = PairedPeer(
                        fingerprint = "0".repeat(64),
                        syncAddr = "",
                        name = "",
                        sessionKeyWrappedB64 = "",
                        sessionKeyIvB64 = "",
                    ),
                    onDismiss = {},
                )
            }
        }
    }
}
