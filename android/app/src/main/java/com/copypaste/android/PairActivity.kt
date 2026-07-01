package com.copypaste.android

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.mutableStateOf
import com.copypaste.android.ui.theme.SecureWindowChrome

// CopyPaste-vp63.38: PairActivity is now a thin Activity shell — deep-link
// intent parsing + FLAG_SECURE window setup. The pairing UI/state machine
// lives in PairScreen.kt / PairController.kt (+ PairBootstrapSync.kt,
// PairingApi.kt) and the extracted card composables (PairQrCard.kt,
// PairedPeerList.kt).

/**
 * Pair Device screen.
 *
 * Two flows:
 *  - **Display**: [startPairing] (UniFFI `buildPairingQr`) yields a `CPPAIR1.…`
 *    payload, rendered as a QR code another device scans.
 *  - **Scan**: the ZXing camera scanner reads another device's QR; the payload
 *    is parsed via [parsePairing] (UniFFI `parsePairingQr`) to recover the peer
 *    fingerprint + PAKE password.
 *
 * The QR is a transport for the existing PAKE pairing material — not new crypto.
 */
class PairActivity : ComponentActivity() {

    // Holds an incoming cppair:// deep-link payload (the raw CPPAIR1.… string)
    // so the Compose screen can observe and process it.  Written from both
    // onCreate (cold start via deep-link) and onNewIntent (singleTop re-launch).
    private val deepLinkPayload = mutableStateOf<String?>(null)

    // Set to a non-null message when a cppair:// URI is received but the `p`
    // param fails the CPPAIR1. sanity check — gives the user visible feedback
    // instead of silently ignoring the malformed link.
    private val deepLinkError = mutableStateOf<String?>(null)

    // HB-6: when launched from the Devices screen's "Scan a device's QR" button
    // (Intent extra mode=scan), auto-open the camera scan flow on first compose.
    // Without the extra (e.g. opened to show this device's own QR) it stays false.
    private val autoScan = mutableStateOf(false)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // FLAG_SECURE: this screen renders the pairing QR, which encodes the PAKE
        // password + sync provisioning material (Supabase/relay creds, derived
        // sync key). Block screenshots and screen-recording so that secret cannot
        // be captured off-screen. Set before setContent so the window carries the
        // flag for its entire lifetime.
        window.setFlags(
            WindowManager.LayoutParams.FLAG_SECURE,
            WindowManager.LayoutParams.FLAG_SECURE,
        )
        enableEdgeToEdge()
        // Extract payload from a cold-start deep-link intent, if present.
        handleDeepLinkIntent(intent)
        // HB-6: honor mode=scan from the Devices screen to auto-open the scanner.
        if (intent?.getStringExtra("mode") == "scan") autoScan.value = true
        setContent {
            SecureWindowChrome {
                PairScreen(
                    onBack = { finish() },
                    incomingDeepLinkPayload = deepLinkPayload.value,
                    onDeepLinkConsumed = { deepLinkPayload.value = null },
                    incomingDeepLinkError = deepLinkError.value,
                    onDeepLinkErrorConsumed = { deepLinkError.value = null },
                    autoScan = autoScan.value,
                    onAutoScanConsumed = { autoScan.value = false },
                )
            }
        }
    }

    // Called by the system when launchMode="singleTop" delivers a new intent
    // to the already-running activity (e.g. user scans another QR via Lens).
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleDeepLinkIntent(intent)
    }

    /**
     * Route an incoming intent: valid CPPAIR1/CPPAIR2 payload → deepLinkPayload;
     * cppair:// URI with unrecognised payload → deepLinkError for user feedback.
     */
    private fun handleDeepLinkIntent(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        val uri: Uri = intent.data ?: return
        if (uri.scheme != "cppair" || uri.host != "pair") return
        val p = uri.getQueryParameter("p") ?: return
        // Accept both CPPAIR1 (legacy) and CPPAIR2 (current compact encoding).
        if (p.startsWith("CPPAIR1.") || p.startsWith("CPPAIR2.")) {
            deepLinkPayload.value = p
        } else {
            // Payload present but not a recognised pairing token — surface to user.
            deepLinkError.value = "Invalid pairing link"
        }
    }
}
