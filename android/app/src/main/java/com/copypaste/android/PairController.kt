package com.copypaste.android

import android.content.Context
import android.graphics.Bitmap
import android.os.Build
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.copypaste_android.BootstrapResult
import uniffi.copypaste_android.ScannedPairing

// CopyPaste-vp63.38: PairController is the state machine that used to live as
// local `remember { mutableStateOf(...) }` vars + closures inside the
// `PairScreen` composable (QR generation, ZXing/deep-link scan handling, PAKE
// bootstrap, post-bootstrap sync + roster commit). Extracted verbatim so
// PairScreen.kt is left with only Scaffold/dialog orchestration.
//
// The PairingApi seam + ScanTransition live in PairingApi.kt; runBootstrap/
// finalizeSync (the two heaviest async steps) live as extension functions in
// PairBootstrapSync.kt — split purely to keep every file under the size
// target, not a behavioural change. All three files together form ONE
// logical unit: PairController's state machine.

/** Pairing token lifetime in seconds — mirrors the Rust core's PAKE session TTL. */
internal const val PAIR_TOKEN_TTL_SECONDS = 120

/** Threshold below which the countdown switches to an urgency color. */
internal const val PAIR_TOKEN_URGENT_THRESHOLD_SECONDS = 20

/**
 * Pixel resolution of the QR bitmap passed to ZXing.
 * CopyPaste-s6cc: raised 512→800 so the bitmap is not downscaled at 3× density
 * (512px < 480 logical px) — downscaling blurs module edges and hurts scanner decoding.
 */
internal const val QR_BITMAP_PX = 800

/**
 * Non-UI state holder driving the pairing state machine: QR generation/TTL
 * refresh, ZXing/deep-link scan-result handling, PAKE bootstrap, and the
 * post-bootstrap sync + roster commit (the latter two in PairBootstrapSync.kt).
 *
 * State is exposed as Compose [androidx.compose.runtime.MutableState]-backed
 * properties so [PairScreen] recomposes on every transition, exactly as when
 * these were local `remember { mutableStateOf(...) }` vars.
 *
 * SECURITY: [qr]/[qrBitmap] encode the pairing secret (PAKE password + optional
 * cloud sync key via QR provisioning) — never log their contents.
 * [pendingProvisioningRaw] likewise carries the raw scanned/deep-linked payload;
 * it is applied (see `finalizeSync` in PairBootstrapSync.kt) only AFTER the
 * user confirms the PAKE-verified peer (CopyPaste-tqt0) — never at scan/parse
 * time.
 *
 * Constructor properties are `internal` (not `private`) so the
 * [runBootstrap]/[finalizeSync] extension functions in PairBootstrapSync.kt
 * can reach them; the class itself stays `internal` (module-private).
 */
internal class PairController(
    internal val context: Context,
    internal val settings: Settings,
    internal val deviceKeyStore: DeviceKeyStore,
    internal val repository: ClipboardRepository,
    internal val scope: CoroutineScope,
    internal val api: PairingApi = PairingApi.Real,
) {
    var qr by mutableStateOf<PairingQrResult?>(null)
        private set
    var qrBitmap by mutableStateOf<Bitmap?>(null)
        private set
    var loading by mutableStateOf(false)
        private set

    /** Sanitized, user-friendly error string. Set by any failed step; the
     * caller shows it as a toast and then clears it back to null. */
    var errorMessage: String? by mutableStateOf(null)

    var scannedInfo: String? by mutableStateOf(null)
    var scannedPeer: ScannedPairing? by mutableStateOf(null)

    // CopyPaste-tqt0: the raw scanned/deep-linked QR payload, retained so its 6th-field
    // relay/Supabase provisioning can be applied AFTER the user confirms pairing (inside
    // finalizeSync, only once the PAKE bootstrap succeeds) — NOT at scan/parse time. A
    // hostile cppair:// could otherwise silently seed attacker-controlled URLs on a fresh
    // install before any consent.
    internal var pendingProvisioningRaw: String? by mutableStateOf(null)

    var syncing: Boolean by mutableStateOf(false)
    var syncResult: String? by mutableStateOf(null)

    /** Holds the just-paired peer to display in the compact success popup.
     * Set at the end of `finalizeSync`; cleared by the caller when the popup
     * is dismissed. */
    var pairedPeerForPopup: PairedPeer? by mutableStateOf(null)

    // CopyPaste-1jms.33: holds the BootstrapResult from the PAKE exchange so the
    // peer-review card can display model/OS/appVersion BEFORE the user clicks
    // "Confirm & sync".  Non-null means PAKE succeeded; null means not yet run or
    // the user discarded the result.  Cleared on finalizeSync completion or cancel.
    var pendingBootstrap: BootstrapResult? by mutableStateOf(null)

    /** Countdown ticker value — driven by the caller's lifecycle-aware loop
     * (see PairScreen's `LaunchedEffect(qr)`), read here for [expired]. */
    var remainingSeconds: Int by mutableStateOf(0)

    // CopyPaste-5917.36: QR blur+reveal — starts blurred (initial generateQr call);
    // first tap reveals. TTL auto-refresh intentionally does NOT reset this to false —
    // reveal state is user-owned and must persist across token refreshes.
    var qrRevealed: Boolean by mutableStateOf(false)

    val expired: Boolean get() = qr != null && remainingSeconds <= 0

    // ── QR generation ────────────────────────────────────────────────────────

    /**
     * Generate a new QR and render its bitmap.
     *
     * [resetReveal] is true only for the initial load (AND2 auto-start) so the
     * QR starts blurred; the 120s TTL auto-refresh calls this with the default
     * `false` (l7n0/PG-8: an auto-refresh must NOT re-blur an already-revealed
     * QR — reveal is user-owned and sticky across payload refreshes).
     */
    fun generateQr(resetReveal: Boolean = false) {
        scope.launch {
            loading = true
            try {
                val result = withContext(Dispatchers.IO) {
                    api.startPairing(settings.deviceId, Build.MODEL ?: "Android")
                }
                val bmp = withContext(Dispatchers.Default) {
                    api.encodeQrBitmap(result.qr, QR_BITMAP_PX)
                }
                qr = result
                qrBitmap = bmp
                if (resetReveal) qrRevealed = false
            } catch (e: Exception) {
                // CopyPaste-jwga: never surface raw exception text; sanitize centrally.
                errorMessage = ErrorMessages.friendlyQrError(e)
            } finally {
                loading = false
            }
        }
    }

    // ── Scan / deep-link handling ────────────────────────────────────────────

    /** ZXing in-app scan result — synchronous parsePairing (matches prior behaviour). */
    fun handleScanResult(contents: String) {
        applyScanTransition(scanTransition(api, contents))
    }

    /**
     * External `cppair://` deep-link payload (e.g. Google Lens) — parsePairing
     * runs off the main thread, matching the prior `LaunchedEffect` behaviour.
     * `suspend` (not `scope.launch`-ed internally) so the caller's own
     * `LaunchedEffect(incomingDeepLinkPayload)` can await it and only THEN
     * consume the payload (`onDeepLinkConsumed()`) — matching the original
     * `try { ... } finally { onDeepLinkConsumed() }` ordering exactly.
     */
    suspend fun handleDeepLinkPayload(payload: String) {
        val transition = withContext(Dispatchers.IO) { scanTransition(api, payload) }
        applyScanTransition(transition)
    }

    private fun applyScanTransition(transition: ScanTransition) {
        when (transition) {
            is ScanTransition.Scanned -> {
                scannedPeer = transition.scannedPeer
                // CopyPaste-1jms.33: new scan resets the PAKE result so the review card
                // starts fresh (no stale metadata from a previous bootstrap attempt).
                pendingBootstrap = null
                syncResult = null
                scannedInfo = transition.scannedInfo
                pendingProvisioningRaw = transition.pendingProvisioningRaw
            }
            is ScanTransition.Failed -> errorMessage = transition.errorMessage
        }
    }

    /** Discard a PAKE-verified result so the user can re-scan (the review
     * card's "Cancel" button). */
    fun cancelReview() {
        pendingBootstrap = null
        scannedPeer = null
        pendingProvisioningRaw = null
    }

    companion object {
        /**
         * Pure reducer for "a pairing payload was scanned/deep-linked": parses via
         * [api] and returns either the resulting peer-review state or a sanitized
         * error message. No Context/Settings dependency — the state-machine step
         * both [handleScanResult] and [handleDeepLinkPayload] apply, fully
         * exercisable in a JUnit test with a fake [PairingApi].
         */
        internal fun scanTransition(api: PairingApi, rawPayload: String): ScanTransition =
            try {
                val info = api.parsePairing(rawPayload)
                ScanTransition.Scanned(
                    scannedPeer = info,
                    scannedInfo = formatScannedInfo(info.deviceName, info.fingerprint),
                    pendingProvisioningRaw = rawPayload,
                )
            } catch (e: Exception) {
                // CopyPaste-jwga: never surface raw exception text to users.
                ScanTransition.Failed(ErrorMessages.friendlyPairingError(e))
            }
    }
}
