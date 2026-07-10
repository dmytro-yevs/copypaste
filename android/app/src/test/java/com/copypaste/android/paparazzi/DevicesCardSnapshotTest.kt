package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.NoPeerCard
import com.copypaste.android.OwnDeviceRow
import com.copypaste.android.P2pIdentity
import com.copypaste.android.PairedPeer
import com.copypaste.android.PeerRow
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.LocalCpColors
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// android-devices (S7 restyle) — a small, representative golden fixture set
// (design.md D13/R14 "representative goldens... not a full cross-product"),
// mirroring NavPillSnapshotTest's approach: [PeerRow]/[OwnDeviceRow]/
// [NoPeerCard] are hermetic (plain data + callback params, no repository/FFI/
// Activity — see their kdocs), so every fixture below is deterministic. The
// "one dialog" golden reproduces UnpairConfirmDialog's exact visual structure
// (GlassAlertDialog + local-only notice + danger/ghost buttons) WITHOUT going
// through DevicesController, which requires a real Context/AndroidKeyStore
// unavailable in this JVM host — see [DevicesDialogFixture]'s kdoc.
// ---------------------------------------------------------------------------
class DevicesCardSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    // Fixed clock so "Xm ago" relative-time text never drifts between runs.
    private val fixedNowMs = 1_700_000_000_000L

    @Test
    fun `dark theme, own device card`() {
        paparazzi.snapshot {
            CardFixture(isDark = true) {
                OwnDeviceRow(
                    identity = P2pIdentity(
                        deviceId = "own-device",
                        fingerprint = "0123456789abcdef".repeat(4),
                        certDer = ByteArray(0),
                        keyDer = ByteArray(0),
                    ),
                    nowMs = fixedNowMs,
                    ownPublicIp = "203.0.113.10",
                    // CopyPaste-6l1ky: lanIpv4Address() enumerates the HOST's real
                    // NetworkInterfaces, so on a JVM-hosted Paparazzi run it
                    // returns whatever LAN IP the runner happens to have —
                    // non-deterministic across otherwise-identical runs. Fix it
                    // so the golden is byte-exact and reproducible.
                    localIpOverrideForTest = "10.1.0.82",
                )
            }
        }
    }

    @Test
    fun `dark theme, peer card online`() {
        paparazzi.snapshot {
            CardFixture(isDark = true) {
                PeerRow(
                    peer = fixturePeer(latencyMs = 42),
                    online = true,
                    nowMs = fixedNowMs,
                    onUnpair = {},
                    onRevoke = {},
                    onCopyFingerprint = {},
                )
            }
        }
    }

    @Test
    fun `light theme, peer card offline`() {
        paparazzi.snapshot {
            CardFixture(isDark = false) {
                PeerRow(
                    // No live P2P link — RTT renders the "—" placeholder
                    // (android-devices spec "RTT shows a placeholder without a
                    // live P2P link") instead of hiding the row.
                    peer = fixturePeer(latencyMs = null),
                    online = false,
                    nowMs = fixedNowMs,
                    onUnpair = {},
                    onRevoke = {},
                    onCopyFingerprint = {},
                )
            }
        }
    }

    @Test
    fun `dark theme, no paired peers empty state`() {
        paparazzi.snapshot {
            CardFixture(isDark = true) {
                NoPeerCard(onPair = {})
            }
        }
    }

    @Test
    fun `dark theme, unpair confirm dialog`() {
        paparazzi.snapshot {
            CopyPasteTheme(isDark = true) {
                DevicesDialogFixture()
            }
        }
    }

    private fun fixturePeer(latencyMs: Int?) = PairedPeer(
        fingerprint = "fedcba9876543210".repeat(4),
        syncAddr = "10.0.0.7:4242",
        name = "Work MacBook",
        sessionKeyWrappedB64 = "",
        sessionKeyIvB64 = "",
        peerModel = "MacBook Pro (M2)",
        peerOs = "macOS 15.4",
        peerAppVersion = "0.5.4",
        peerLocalIp = "10.0.0.7",
        peerPublicIp = "203.0.113.20",
        pairedAtMs = fixedNowMs - 86_400_000L,
        lastSyncMs = fixedNowMs - 120_000L,
        latencyMs = latencyMs,
        sasVerified = true,
    )
}

@Composable
private fun CardFixture(isDark: Boolean, content: @Composable () -> Unit) {
    CopyPasteTheme(isDark = isDark) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(LocalCpColors.current.bg)
                .padding(16.dp),
        ) {
            CopyPasteCard {
                content()
            }
        }
    }
}

/**
 * Reproduces [com.copypaste.android.UnpairConfirmDialog]'s exact visual
 * structure (title/body/local-only-notice text + danger/ghost buttons) as a
 * standalone, hermetic fixture. The real function is `private` and reads its
 * target off a [com.copypaste.android.DevicesController], which needs a real
 * `Context` + `AndroidKeyStore` (not available in this Paparazzi JVM host) —
 * so this fixture calls the SAME shared building blocks
 * ([GlassAlertDialog]/[CopyPasteButton]) with literal fixture text instead,
 * matching the "representative golden" pattern used by NavPillSnapshotTest's
 * `StripedBackdrop`.
 */
@Composable
private fun DevicesDialogFixture() {
    GlassAlertDialog(
        onDismissRequest = {},
        title = { Text("Unpair device?") },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    "This device will no longer sync with Work MacBook over P2P. " +
                        "You can re-pair at any time by scanning a new QR code.",
                )
                Text("This is a local action only — the other device isn't notified.")
            }
        },
        confirmButton = {
            CopyPasteButton(onClick = {}, variant = ButtonVariant.DANGER) { Text("Unpair") }
        },
        dismissButton = {
            CopyPasteButton(onClick = {}, variant = ButtonVariant.GHOST) { Text("Cancel") }
        },
    )
}
