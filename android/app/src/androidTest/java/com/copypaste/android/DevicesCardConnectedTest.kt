package com.copypaste.android

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.copypaste.android.ui.theme.CopyPasteTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

/**
 * CopyPaste-myh8.7 — S7 (Devices) connected checks for [PeerRow]/[OwnDeviceRow]:
 * fingerprint tap-to-copy (android-devices spec "Fingerprint tap-to-copy
 * parity" — NEW on both surfaces) and the RTT placeholder (android-devices
 * spec "RTT shows a placeholder without a live P2P link"). Both composables
 * are hermetic (plain data + callback params, no repository/FFI/Activity), so
 * every fixture here is deterministic. Per the S4 "connected-test CI
 * availability" decision this run is required locally
 * (:app:connectedDebugAndroidTest); no emulator is available in this sandbox,
 * so this class is written so it COMPILES and is ready for a pending local run.
 */
class DevicesCardConnectedTest {

    @get:Rule
    val composeRule = createAndroidComposeRule<ComponentActivity>()

    private val fingerprint = "0123456789abcdef".repeat(4)
    private val truncatedFingerprint = "0123456789abcdef…89abcdef"

    private fun fixturePeer(latencyMs: Int? = null) = PairedPeer(
        fingerprint = fingerprint,
        syncAddr = "",
        name = "Test Mac",
        sessionKeyWrappedB64 = "",
        sessionKeyIvB64 = "",
        peerModel = "MacBook Air (M3)",
        peerOs = "macOS 15.3",
        peerAppVersion = "0.5.3",
        peerLocalIp = "10.0.0.5",
        latencyMs = latencyMs,
        sasVerified = true,
    )

    @Test
    fun tappingThePeerCardFingerprintCopiesTheFullSixtyFourHexValue() {
        var copied: String? = null
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                PeerRow(
                    peer = fixturePeer(),
                    online = true,
                    nowMs = System.currentTimeMillis(),
                    onUnpair = {},
                    onRevoke = {},
                    onCopyFingerprint = { copied = it },
                )
            }
        }

        composeRule.onNodeWithText(truncatedFingerprint).performClick()

        assertEquals(fingerprint, copied)
    }

    @Test
    fun tappingTheOwnDeviceCardFingerprintCopiesTheFullSixtyFourHexValue() {
        var copied: String? = null
        val identity = P2pIdentity(
            deviceId = "own-device",
            fingerprint = fingerprint,
            certDer = ByteArray(0),
            keyDer = ByteArray(0),
        )
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                OwnDeviceRow(
                    identity = identity,
                    nowMs = System.currentTimeMillis(),
                    onCopyFingerprint = { copied = it },
                )
            }
        }

        composeRule.onNodeWithText(truncatedFingerprint).performClick()

        assertEquals(fingerprint, copied)
    }

    @Test
    fun rttRendersAnEmDashPlaceholderWithoutALiveP2pLink() {
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                PeerRow(
                    peer = fixturePeer(latencyMs = null),
                    online = false,
                    nowMs = System.currentTimeMillis(),
                    onUnpair = {},
                    onRevoke = {},
                    onCopyFingerprint = {},
                )
            }
        }

        // The RTT row is always rendered (android-devices spec) — asserting the
        // placeholder text exists proves the row was not silently hidden.
        composeRule.onNodeWithText(EM_DASH).assertExists()
    }

    @Test
    fun rttRendersTheMeasuredRoundTripTimeWhenPresent() {
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                PeerRow(
                    peer = fixturePeer(latencyMs = 42),
                    online = true,
                    nowMs = System.currentTimeMillis(),
                    onUnpair = {},
                    onRevoke = {},
                    onCopyFingerprint = {},
                )
            }
        }

        composeRule.onNodeWithText("42 ms").assertExists()
    }

    @Test
    fun unpairAndRevokeFooterButtonsInvokeTheirCallbacks() {
        var unpaired = false
        var revoked = false
        composeRule.setContent {
            CopyPasteTheme(isDark = true) {
                PeerRow(
                    peer = fixturePeer(),
                    online = true,
                    nowMs = System.currentTimeMillis(),
                    onUnpair = { unpaired = true },
                    onRevoke = { revoked = true },
                    onCopyFingerprint = {},
                )
            }
        }

        composeRule.onNodeWithText(composeRule.activity.getString(R.string.btn_unpair)).performClick()
        assertEquals(true, unpaired)
        composeRule.onNodeWithText(composeRule.activity.getString(R.string.btn_revoke)).performClick()
        assertEquals(true, revoked)
    }
}
