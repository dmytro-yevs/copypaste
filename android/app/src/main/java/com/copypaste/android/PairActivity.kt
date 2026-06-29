package com.copypaste.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.net.Uri
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCode
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
// TextButton removed — replaced by CopyPasteButton (CopyPaste-bdac.8)
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.repeatOnLifecycle
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.foundation.border
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.accentFill
import com.copypaste.android.ui.theme.accentTint
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.RadiusChip
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.screenCanvas
import com.copypaste.android.ui.theme.rememberTranslucency
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.copypaste_android.BootstrapResult
import uniffi.copypaste_android.ScannedPairing
import uniffi.copypaste_android.bootstrapPairInitiator
// syncWithPeer is the package-local wrapper in CopypasteBindings.kt (ABI-9
// signature: revokedFingerprints + deviceId, ByteArray sessionKey). It shadows
// the generated uniffi.copypaste_android.syncWithPeer intentionally.

/** Pairing token lifetime in seconds — mirrors the Rust core's PAKE session TTL. */
private const val PAIR_TOKEN_TTL_SECONDS = 120

/** Threshold below which the countdown switches to an urgency color. */
private const val PAIR_TOKEN_URGENT_THRESHOLD_SECONDS = 20

/**
 * Side of the rendered QR image, in dp.
 * 1jms.19: unified to 200dp to match DevicesActivity.DEVICES_QR_IMAGE_DP — both
 * screens display the same pairing QR content and must render at the same size.
 * (was 160dp per bro9; DevicesActivity was already 200dp — aligned upward.)
 */
private const val QR_IMAGE_SIZE_DP = 200

/**
 * Padding of the inset white QR plate, in dp (each side).
 * ioco: the plate is sized only to the QR itself (not the full slot) and rounded
 * with RadiusCard (12dp) so it sits cleanly on the glass surface.
 */
private const val QR_PLATE_PADDING_DP = 10

/**
 * Fixed side of the reserved QR slot, in dp: QR image + plate padding both sides.
 * Every QR-area state renders into a box of exactly this size so the layout stays
 * visually stable (no jitter).
 */
private const val QR_SLOT_SIZE_DP = QR_IMAGE_SIZE_DP + QR_PLATE_PADDING_DP * 2

/**
 * Pixel resolution of the QR bitmap passed to ZXing.
 * CopyPaste-s6cc: raised 512→800 so the bitmap is not downscaled at 3× density
 * (512px < 480 logical px) — downscaling blurs module edges and hurts scanner decoding.
 */
private const val QR_BITMAP_PX = 800

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
            CopyPasteTheme {
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

// CopyPaste-jkbo: encodeQrBitmap was a private duplicate of the same function in
// DevicesActivity. Both are now replaced by the package-level [encodeQrBitmap] in
// QrUtils.kt — call sites in this file reference it directly (same package).
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PairScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
    incomingDeepLinkPayload: String? = null,
    onDeepLinkConsumed: () -> Unit = {},
    incomingDeepLinkError: String? = null,
    onDeepLinkErrorConsumed: () -> Unit = {},
    autoScan: Boolean = false,
    onAutoScanConsumed: () -> Unit = {},
) {
    val context = LocalContext.current
    val settings = remember { Settings(context) }
    val deviceKeyStore = remember { DeviceKeyStore(context) }
    val repository = remember { ClipboardRepository(context) }

    var qr by remember { mutableStateOf<PairingQrResult?>(null) }
    var qrBitmap by remember { mutableStateOf<Bitmap?>(null) }
    var loading by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var scannedInfo by remember { mutableStateOf<String?>(null) }
    var scannedPeer by remember { mutableStateOf<ScannedPairing?>(null) }
    // CopyPaste-tqt0: the raw scanned/deep-linked QR payload, retained so its 6th-field
    // relay/Supabase provisioning can be applied AFTER the user confirms pairing (inside
    // finalizeSync, only once the PAKE bootstrap succeeds) — NOT at scan/parse time. A
    // hostile cppair:// could otherwise silently seed attacker-controlled URLs on a fresh
    // install before any consent.
    var pendingProvisioningRaw by remember { mutableStateOf<String?>(null) }
    var syncing by remember { mutableStateOf(false) }
    var syncResult by remember { mutableStateOf<String?>(null) }
    // Holds the just-paired peer to display in the compact success popup.
    // Set at the end of finalizeSync; cleared when the popup is dismissed.
    var pairedPeerForPopup by remember { mutableStateOf<PairedPeer?>(null) }
    // CopyPaste-1jms.33: holds the BootstrapResult from the PAKE exchange so the
    // peer-review card can display model/OS/appVersion BEFORE the user clicks
    // "Confirm & sync".  Non-null means PAKE succeeded; null means not yet run or
    // the user discarded the result.  Cleared on finalizeSync completion or cancel.
    var pendingBootstrap by remember { mutableStateOf<BootstrapResult?>(null) }
    var remainingSeconds by remember { mutableStateOf(0) }
    // CopyPaste-5917.36: QR blur+reveal — starts blurred (set in LaunchedEffect(Unit));
    // first tap reveals. TTL auto-refresh (generateQr) intentionally does NOT reset this
    // to false — reveal state is user-owned and must persist across token refreshes.
    var qrRevealed by remember { mutableStateOf(false) }
    val toastState = remember { GlassToastState() }
    val scope = rememberCoroutineScope()
    val clipboardManager = LocalClipboardManager.current
    // CopyPaste-jwga: errorMessage now holds a pre-sanitized, user-friendly string
    // from ErrorMessages.friendly*(). No wrapper template is applied at display time
    // so raw exception text, paths, and FFI symbols never reach the user.

    val expired = qr != null && remainingSeconds <= 0

    // Shared helper: generate a new QR and render its bitmap.
    fun generateQr() {
        scope.launch {
            loading = true
            try {
                val result = withContext(Dispatchers.IO) {
                    startPairing(settings.deviceId, android.os.Build.MODEL ?: "Android")
                }
                val bmp = withContext(Dispatchers.Default) {
                    encodeQrBitmap(result.qr, QR_BITMAP_PX)
                }
                qr = result
                qrBitmap = bmp
                // l7n0 (PG-8): do NOT reset qrRevealed here. An auto-refresh of the
                // QR payload must preserve the user's current reveal state — re-blurring
                // on every 120s TTL refresh is privacy-hostile (flickers reveal state).
                // The blur is user-owned: only the initial launch starts blurred (see
                // LaunchedEffect(Unit) below). Reveal persists across payload refreshes,
                // matching macOS behaviour where reveal is sticky.
            } catch (e: Exception) {
                // CopyPaste-jwga: never surface raw exception text; sanitize centrally.
                errorMessage = ErrorMessages.friendlyQrError(e)
            } finally {
                loading = false
            }
        }
    }

    // Camera scanner (ZXing). On a successful scan, parse the payload natively.
    val scanLauncher = rememberLauncherForActivityResult(ScanContract()) { result ->
        val contents = result.contents
            ?: return@rememberLauncherForActivityResult // user cancelled
        try {
            val info = parsePairing(contents)
            // Surface the peer identity and retain it so the user can confirm,
            // then drive the PAKE bootstrap + one sync (initiator side).
            scannedPeer = info
            // CopyPaste-1jms.33: new scan resets the PAKE result so the review card
            // starts fresh (no stale metadata from a previous bootstrap attempt).
            pendingBootstrap = null
            syncResult = null
            scannedInfo = formatScannedInfo(info.deviceName, info.fingerprint)
            // CopyPaste-tqt0: retain the raw QR payload but DO NOT apply its 6th-field
            // relay/Supabase provisioning yet. Applying at scan time let a hostile QR
            // seed attacker-controlled URLs into Settings on a fresh install before the
            // user consented. Provisioning is now applied inside finalizeSync, only
            // after the PAKE bootstrap (SAS confirmation) succeeds.
            pendingProvisioningRaw = contents
        } catch (e: Exception) {
            // CopyPaste-jwga: never surface raw exception text to users.
            errorMessage = ErrorMessages.friendlyPairingError(e)
        }
    }

    fun launchScanner() {
        val options = ScanOptions()
            .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
            .setPrompt("Scan the pairing QR on the other device")
            .setBeepEnabled(false)
            // Route to our portrait-locked capture activity (see
            // PortraitCaptureActivity) so the preview is upright on phones held
            // in portrait. setOrientationLocked(true) keeps ZXing from trying to
            // re-orient on top of our fixed-portrait activity.
            .setOrientationLocked(true)
            .setCaptureActivity(PortraitCaptureActivity::class.java)
        // Launching the scanner can fail (e.g. ActivityNotFoundException if the
        // capture activity is missing, or the camera is unavailable). Surface it
        // as a graceful error instead of letting the activity result launcher
        // crash the host screen.
        try {
            scanLauncher.launch(options)
        } catch (e: Exception) {
            // CopyPaste-jwga: never surface raw exception text to users.
            errorMessage = ErrorMessages.friendlyCameraError(e)
        }
    }

    // Runtime CAMERA permission. ZXing's embedded scanner needs the camera; we
    // request it explicitly so a denial gives a clear message instead of the
    // scanner silently aborting (which the ScanContract reports as "cancelled").
    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            launchScanner()
        } else {
            // CopyPaste-l080: distinguish a recoverable denial from a permanent one.
            // After a permanent denial (the OS will no longer show the dialog) point
            // the user at app-details Settings so re-tapping Scan is not a dead end.
            val activity = context as? android.app.Activity
            if (activity != null && NotificationPermissionHelper.isCameraPermanentlyDenied(activity)) {
                NotificationPermissionHelper.launchFirstResolvable(
                    context, NotificationPermissionHelper.appDetailsSettingsIntents(context),
                )
                // CopyPaste-jwga: use string resource for user-facing message.
                errorMessage = context.getString(R.string.error_camera_permission_permanent)
            } else {
                // CopyPaste-jwga: use string resource for user-facing message.
                errorMessage = context.getString(R.string.error_camera_permission_denied)
            }
        }
    }

    fun startScanFlow() {
        val hasCamera = ContextCompat.checkSelfPermission(
            context, Manifest.permission.CAMERA
        ) == PackageManager.PERMISSION_GRANTED
        if (hasCamera) {
            launchScanner()
            return
        }
        // CopyPaste-l080: if CAMERA is already permanently denied, requesting again is
        // a silent no-op — go straight to app-details Settings instead.
        val activity = context as? android.app.Activity
        if (activity != null && NotificationPermissionHelper.isCameraPermanentlyDenied(activity)) {
            NotificationPermissionHelper.launchFirstResolvable(
                context, NotificationPermissionHelper.appDetailsSettingsIntents(context),
            )
            // CopyPaste-jwga: use string resource for user-facing message.
            errorMessage = context.getString(R.string.error_camera_permission_permanent)
            return
        }
        NotificationPermissionHelper.markCameraRequested(context)
        cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
    }

    // Drive bootstrap PAKE pairing + a single P2P sync against the scanned peer
    // (Android-as-initiator). Runs entirely off the main thread; result text is
    // shown on completion. All FFI errors surface as a snackbar (no crash).
    //
    // NOTE (L4, RESOLVED on the macOS side): the daemon now advertises a real
    // LAN-routable host:port (via copypaste_p2p::interfaces::advertise_sync_addr)
    // in BOTH the QR addr_hint AND the in-band P2P sync-listener address, instead
    // of 127.0.0.1. So `bootstrap.peerSyncAddr` persisted below in
    // `settings.pairedPeerSyncAddr` is now dialable from a real phone over Wi-Fi
    // (it is loopback only when the Mac has no LAN interface at all).
    //
    // REMAINING Android work for UNATTENDED background sync (Android→macOS):
    // this `finalizeSync` only performs ONE sync at pairing time. To sync in
    // the background after pairing, add a periodic caller (e.g. in FgsSyncLoop,
    // gated on a non-blank `settings.pairedPeerSyncAddr` /
    // `settings.pairedPeerFingerprint`) that, on each tick, loads the device
    // cert + session-derived key and calls `syncWithPeer(peerAddr =
    // settings.pairedPeerSyncAddr, ...)` exactly as below. The macOS daemon's
    // accept loop (binds 0.0.0.0) already receives such dials, so no macOS change
    // is needed for that direction. The reverse (macOS→Android) additionally
    // needs an Android-side mTLS LISTENER, which does not exist yet.
    //
    // NOTE: the session key must be persisted/re-derived for repeat syncs — the
    // current flow only has it transiently from `bootstrapPairInitiator`. Persist
    // `bootstrap.sessionKey` securely at pairing time so the background caller can
    // reuse it. Requires an on-device verification (phone + Mac on same Wi-Fi).
    // ── CopyPaste-1jms.33: two-phase pairing flow ────────────────────────────
    //
    // Phase 1 — runBootstrap: runs the PAKE exchange and stores the result in
    // [pendingBootstrap].  The peer's model/OS/appVersion are now visible in the
    // UI so the user can verify them BEFORE confirming the sync.
    //
    // Phase 2 — finalizeSync: uses the cached [BootstrapResult] to apply
    // provisioning, run the initial sync and commit the peer to the roster.
    //
    // The PAKE crypto flow is unchanged — only the UI confirmation step is split.
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * Phase 1: run the PAKE bootstrap and surface the peer's device metadata.
     *
     * On success [pendingBootstrap] is set; the UI switches to the review card
     * which shows model/OS/appVersion and a "Confirm & sync" button.
     * On failure [errorMessage] is set (sanitized) and [pendingBootstrap] stays null.
     */
    fun runBootstrap(peer: ScannedPairing) {
        if (syncing) return
        // CopyPaste-1jms.13: resolve the human-readable device name BEFORE
        // entering the coroutine, where `context` (LocalContext.current) is only
        // accessible on the composition thread and ContentResolver is not
        // capturable via a qualified-this label inside a nested coroutine.
        val deviceNameForPairing: String = run {
            val settingsName = try {
                android.provider.Settings.Global.getString(
                    context.contentResolver,
                    "device_name",
                )
            } catch (_: Exception) { null }
            settingsName?.takeIf { it.isNotBlank() }
                ?: android.os.Build.MODEL
                ?: "Android"
        }
        scope.launch {
            syncing = true
            syncResult = null
            try {
                val bootstrap = withContext(Dispatchers.IO) {
                    // CopyPaste-44rq.55: getOrCreate() zeroes cert.keyDer; re-fetch
                    // via peek() to obtain the KEK-unwrapped key from AndroidKeyStore.
                    deviceKeyStore.getOrCreate()
                    val cert = deviceKeyStore.peek()!!
                    // Path A: advertise THIS device's inbound mTLS listener address
                    // so the macOS peer persists it and can dial back (macOS→Android
                    // direction). The listener is bound by [ClipboardService]; its
                    // OS-assigned port is published in [ClipboardService.activeListenerPort].
                    val listenerPort = ClipboardService.activeListenerPort
                    val lanIp = lanIpv4Address()
                    val ownSyncAddr = if (listenerPort > 0 && lanIp != null) {
                        "$lanIp:$listenerPort"
                    } else {
                        android.util.Log.i(
                            "PairActivity",
                            "Not advertising listener sync_addr (port=$listenerPort, lanIp=${lanIp ?: "none"}) — " +
                                "falling back to Android→macOS dial only until the listener is up",
                        )
                        ""
                    }
                    // ABI 18 (PG-28): collect own WAN address via STUN so the
                    // peer's device record shows a reachable external candidate.
                    val ownPublicIp = StunUtils.queryPublicIp(settings.collectPublicIp)
                    bootstrapPairInitiator(
                        addrHint = peer.addrHint,
                        certDer = cert.certDer,
                        keyDer = cert.keyDer,
                        pakePassword = peer.pakePassword,
                        syncAddr = ownSyncAddr,
                        localProvisioning = null,
                        // HB-1a (ABI 14): send THIS device's own metadata so the
                        // PC's device card shows real Android info.
                        deviceName = deviceNameForPairing,
                        deviceModel = android.os.Build.MODEL ?: "Android",
                        osVersion = "Android " + android.os.Build.VERSION.RELEASE,
                        appVersion = BuildConfig.VERSION_NAME,
                        localIp = lanIp,
                        publicIp = ownPublicIp,
                    )
                }
                // PAKE succeeded — surface the peer metadata in the review card.
                pendingBootstrap = bootstrap
            } catch (e: Exception) {
                // CopyPaste-jwga: never surface raw exception text to users.
                errorMessage = ErrorMessages.friendlyPairingError(e)
            } finally {
                syncing = false
            }
        }
    }

    /**
     * Phase 2: apply provisioning, run the initial P2P sync, and commit the peer
     * to the roster. Called after the user reviews the peer metadata and clicks
     * "Confirm & sync".
     *
     * [bootstrap] is the result from [runBootstrap] — already validated by PAKE.
     * Does NOT re-run the PAKE exchange. The crypto flow is identical to the
     * former single-phase [runPairAndSync], minus the bootstrap step.
     */
    fun finalizeSync(peer: ScannedPairing, bootstrap: BootstrapResult) {
        if (syncing) return
        // CopyPaste-tqt0: snapshot the retained QR payload now; its 6th-field
        // provisioning is applied below ONLY after the PAKE bootstrap succeeds.
        val provisioningRaw = pendingProvisioningRaw
        scope.launch {
            syncing = true
            try {
                val key = settings.encryptionKey
                var pairedFingerprint: String? = null
                val message = withContext(Dispatchers.IO) {
                    val cert = deviceKeyStore.peek()!!
                    // QR full-provisioning: if the paired PC carried its sync
                    // config in the pairing payload, fill any field this device
                    // has not already configured. NEVER overwrite an existing
                    // local value (mirror the daemon's fill-missing rule).
                    bootstrap.peerProvisioning?.let { prov ->
                        val applied = mutableListOf<String>()
                        prov.supabaseUrl?.takeIf { it.isNotBlank() }?.let { url ->
                            if (settings.supabaseUrl.isBlank()) {
                                settings.supabaseUrl = url
                                applied += "supabaseUrl"
                            }
                        }
                        prov.supabaseAnonKey?.takeIf { it.isNotBlank() }?.let { anon ->
                            if (settings.supabaseAnonKey.isBlank()) {
                                settings.supabaseAnonKey = anon
                                applied += "supabaseAnonKey"
                            }
                        }
                        prov.relayUrl?.takeIf { it.isNotBlank() }?.let { relay ->
                            if (settings.relayUrl.isBlank()) {
                                settings.relayUrl = relay
                                applied += "relayUrl"
                            }
                        }
                        // The derived 32-byte cloud sync key: store via the direct
                        // key path (KEK-wrapped) so the phone can decrypt cloud
                        // rows without the passphrase. Only when none is set yet.
                        prov.derivedSyncKey?.takeIf { it.isNotEmpty() }?.let { keyUBytes ->
                            if (settings.cloudSyncKeyDirect == null) {
                                val keyBytes = ByteArray(keyUBytes.size) { keyUBytes[it].toByte() }
                                settings.cloudSyncKeyDirect = keyBytes
                                applied += "derivedSyncKey"
                            }
                        }
                        if (applied.isNotEmpty()) {
                            android.util.Log.i(
                                "PairActivity",
                                "QR provisioning applied (fill-missing): ${applied.joinToString(", ")}",
                            )
                        } else {
                            android.util.Log.i(
                                "PairActivity",
                                "QR provisioning carried by peer but all fields already configured locally — nothing applied",
                            )
                        }
                    }
                    // CopyPaste-tqt0: NOW (post-PAKE = the user confirmed pairing)
                    // apply the QR's 6th-field relay/Supabase provisioning.
                    provisioningRaw?.let { raw ->
                        val prov = extractQrProvisioning(raw)
                        val applied = prov?.let { applyQrProvisioning(it, settings) } ?: emptyList()
                        if (applied.isNotEmpty()) {
                            android.util.Log.i(
                                "PairActivity",
                                "QR provisioning (6th field) applied after pair confirmation: ${applied.joinToString(", ")}",
                            )
                        }
                    }
                    val localItems = repository.localItemsForSync(key)
                    // Denylist: never ingest items from a peer this device revoked.
                    val revoked = runCatching { listRevokedFingerprints(settings.dbPath, key) }
                        .getOrElse { e ->
                            android.util.Log.w("PairActivity", "listRevokedFingerprints failed: ${e.message}")
                            emptyList()
                        }
                    val result = syncWithPeer(
                        peerAddr = bootstrap.peerSyncAddr,
                        peerFingerprint = bootstrap.peerFingerprint,
                        sessionKey = ByteArray(bootstrap.sessionKey.size) { bootstrap.sessionKey[it].toByte() },
                        certDer = cert.certDer,
                        keyDer = cert.keyDer,
                        localItems = localItems,
                        revokedFingerprints = revoked,
                        deviceId = settings.deviceId,
                    )
                    // HB-7b: route each received item BY CONTENT TYPE.
                    var stored = 0
                    for (item in result.items) {
                        val plaintextBytes =
                            ByteArray(item.plaintext.size) { item.plaintext[it].toByte() }
                        val isImage = item.contentType == "image" ||
                            item.contentType.startsWith("image/")
                        val isFile = item.contentType == "file"
                        val didStore = when {
                            isImage -> {
                                if (plaintextBytes.isEmpty()) {
                                    false
                                } else {
                                    val storedId = repository.storeItem(
                                        plaintext = "[image]",
                                        key = key,
                                        overrideId = item.itemId,
                                        contentType = item.contentType,
                                    )
                                    if (storedId.isNotEmpty()) {
                                        repository.storeImageBytes(storedId, plaintextBytes)
                                        SyncThumbnailHelper.generateAndStore(plaintextBytes) { thumb ->
                                            repository.storeThumbnailBytes(storedId, thumb)
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                }
                            }
                            isFile -> {
                                if (plaintextBytes.isEmpty()) {
                                    false
                                } else {
                                    val label = SyncFileHelper.buildFileLabel(item.fileName)
                                    val storedId = repository.storeItem(
                                        plaintext = label,
                                        key = key,
                                        overrideId = item.itemId,
                                        contentType = item.contentType,
                                    )
                                    if (storedId.isNotEmpty()) {
                                        repository.storeFileBytes(storedId, plaintextBytes)
                                        repository.storeFileMeta(storedId, item.fileName, item.mime)
                                        true
                                    } else {
                                        false
                                    }
                                }
                            }
                            else -> {
                                val plaintext = String(plaintextBytes, Charsets.UTF_8)
                                repository.storeItem(plaintext, key, overrideId = item.itemId)
                                    .isNotEmpty()
                            }
                        }
                        if (didStore) stored += 1
                    }
                    // Persist (APPEND) the peer into the multi-peer roster.
                    val rawSessionKey =
                        ByteArray(bootstrap.sessionKey.size) { bootstrap.sessionKey[it].toByte() }
                    val (wrappedB64, ivB64) = settings.wrapSessionKey(rawSessionKey)
                    val nowMs = System.currentTimeMillis()
                    settings.upsertPeer(
                        PairedPeer(
                            fingerprint = bootstrap.peerFingerprint,
                            syncAddr = bootstrap.peerSyncAddr,
                            name = peer.deviceName,
                            sessionKeyWrappedB64 = wrappedB64,
                            sessionKeyIvB64 = ivB64,
                            lastSyncMs = nowMs,
                            pairedAtMs = nowMs,
                            // HB-1b (ABI 14): persist the peer's device metadata
                            // received over the authenticated tunnel.
                            peerModel = bootstrap.peerModel,
                            peerOs = bootstrap.peerOs,
                            peerAppVersion = bootstrap.peerAppVersion,
                            peerLocalIp = bootstrap.peerLocalIp,
                            peerPublicIp = bootstrap.peerPublicIp,
                            // CopyPaste-3k6m (ABI 17): persist the peer's stable device UUID.
                            peerDeviceId = bootstrap.peerDeviceId?.takeIf { it.isNotBlank() }
                                ?: peer.deviceId.takeIf { it.isNotBlank() },
                        )
                    )
                    pairedFingerprint = bootstrap.peerFingerprint
                    val peerCount = settings.pairedPeers.size
                    val skipped = "skipped: legacy ${result.itemsSkippedLegacy} / " +
                        "decrypt ${result.itemsSkippedDecryptFail} / " +
                        "type ${result.itemsSkippedUnknownType} / " +
                        "blob ${result.itemsSkippedMissingBlob}"
                    "Paired with ${peer.deviceName.ifBlank { "device" }} — received ${result.itemsReceived} item(s), stored $stored ($skipped), sent ${result.itemsSent}. ($peerCount paired device(s))"
                }
                // Surface the just-persisted peer for the compact success popup.
                pairedPeerForPopup = settings.pairedPeers
                    .firstOrNull { it.fingerprint == pairedFingerprint }
                syncResult = message
                scannedPeer = null
                pendingBootstrap = null
                // CopyPaste-tqt0: provisioning has been applied; drop the retained payload.
                pendingProvisioningRaw = null
            } catch (e: Exception) {
                // CopyPaste-jwga: never surface raw exception text to users.
                errorMessage = ErrorMessages.friendlyPairingError(e)
            } finally {
                syncing = false
            }
        }
    }

    // Countdown ticker — restarts whenever a fresh QR is issued.
    // When the countdown reaches 0, auto-regenerate the QR.
    // Gated: if the success popup is showing, skip the auto-regenerate so the
    // QR doesn't refresh underneath the dialog and the pairedPeerForPopup state
    // stays stable while the user reads the card.
    //
    // CopyPaste-crh3.28: the countdown + auto-regenerate run ONLY while the
    // screen is RESUMED (repeatOnLifecycle). Backgrounding the activity cancels
    // the block, so the QR no longer silently mints and burns one-time pairing
    // tokens off-screen; it resumes counting when the user returns.
    val lifecycleOwner = LocalLifecycleOwner.current
    LaunchedEffect(qr) {
        if (qr == null) return@LaunchedEffect
        lifecycleOwner.lifecycle.repeatOnLifecycle(Lifecycle.State.RESUMED) {
            remainingSeconds = PAIR_TOKEN_TTL_SECONDS
            // CopyPaste-crh3.33: regenerate with a QR_REFRESH_MARGIN_SECONDS (15s)
            // margin BEFORE the token actually expires (parity with macOS), so a
            // slow scan never reads an already-expired code.
            while (remainingSeconds > QR_REFRESH_MARGIN_SECONDS) {
                delay(1000)
                remainingSeconds -= 1
            }
            // Near expiry — auto-regenerate only when the success popup is not up.
            if (pairedPeerForPopup == null) generateQr()
        }
    }

    // AND2: Auto-start pairing when the screen opens so the QR appears
    // immediately without requiring the user to tap "Start Pairing".
    LaunchedEffect(Unit) {
        if (qr != null || loading) return@LaunchedEffect
        loading = true
        try {
            val result = withContext(Dispatchers.IO) {
                startPairing(settings.deviceId, android.os.Build.MODEL ?: "Android")
            }
            val bmp = withContext(Dispatchers.Default) {
                encodeQrBitmap(result.qr, QR_BITMAP_PX)
            }
            qr = result
            qrBitmap = bmp
            qrRevealed = false  // start blurred
            // Initial load complete.
        } catch (e: Exception) {
            // CopyPaste-jwga: never surface raw exception text to users.
            errorMessage = ErrorMessages.friendlyQrError(e)
        } finally {
            loading = false
        }
    }

    // Consume an incoming cppair:// deep-link payload from an external QR scanner
    // (e.g. Google Lens).  The payload is the raw CPPAIR1/CPPAIR2.… string,
    // identical to what the in-app ZXing scanner would return — feed it through
    // the same parsePairing path and surface the same confirmation UI.
    LaunchedEffect(incomingDeepLinkPayload) {
        val payload = incomingDeepLinkPayload ?: return@LaunchedEffect
        try {
            val info = withContext(Dispatchers.IO) {
                parsePairing(payload)
            }
            scannedPeer = info
            // CopyPaste-1jms.33: new scan resets the PAKE result so the review card
            // starts fresh (no stale metadata from a previous bootstrap attempt).
            pendingBootstrap = null
            syncResult = null
            scannedInfo = formatScannedInfo(info.deviceName, info.fingerprint)
            // CopyPaste-tqt0: retain the raw deep-link payload but DO NOT apply its
            // 6th-field provisioning here. A crafted cppair:// link must not be able to
            // write relay/Supabase URLs into Settings before the user confirms pairing.
            // Applied inside finalizeSync once the PAKE bootstrap succeeds.
            pendingProvisioningRaw = payload
        } catch (e: Exception) {
            // CopyPaste-jwga: never surface raw exception text to users.
            errorMessage = ErrorMessages.friendlyPairingError(e)
        } finally {
            onDeepLinkConsumed()
        }
    }

    // Surface a malformed deep-link (cppair:// with unrecognised payload) as a
    // toast so the user gets explicit feedback instead of nothing happening.
    LaunchedEffect(incomingDeepLinkError) {
        val errMsg = incomingDeepLinkError ?: return@LaunchedEffect
        toastState.show(errMsg, GlassToastKind.DANGER)
        onDeepLinkErrorConsumed()
    }

    LaunchedEffect(errorMessage) {
        val msg = errorMessage ?: return@LaunchedEffect
        // CopyPaste-jwga: msg is already a sanitized, user-friendly string —
        // show it directly without a raw-exception-embedding format template.
        toastState.show(msg, GlassToastKind.DANGER)
        errorMessage = null
    }

    // HB-6: auto-open the scanner when launched in scan mode (from the Devices
    // screen "Scan a device's QR" button). Consumed once so a recomposition does
    // not re-launch it; the QR-display flow above still runs in parallel so this
    // device's own QR is ready behind the scanner.
    LaunchedEffect(autoScan) {
        if (!autoScan) return@LaunchedEffect
        startScanFlow()
        onAutoScanConsumed()
    }

    val c = LocalCpColors.current
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()

    // CopyPaste-wfba: progress-bar pulse removed — static progress bar is calmer.
    // Entrance alpha fade removed — card appears instantly (no idle animation).

    Box(Modifier.fillMaxSize()) {
    // Calm screen backdrop (STYLEGUIDE §6). Frosted only when translucent.
    val scaffoldModifier: Modifier = if (translucent) modifier.screenCanvas(dark) else modifier
    Scaffold(
        modifier = scaffoldModifier,
        containerColor = if (translucent) androidx.compose.ui.graphics.Color.Transparent else c.bg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_pair),
                showBackButton = showBackButton,
                onBack = onBack,
                backContentDescription = stringResource(R.string.cd_back),
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                // innerPadding keeps content BELOW the Scaffold top bar (and snackbar).
                .padding(innerPadding)
                // Make the screen scrollable so a tall scanned-info + cards block
                // (post-QR-scan confirmation, paired-device card, countdown) can be
                // reached instead of being clipped — mirrors Settings/History.
                .verticalScroll(rememberScrollState())
                // Keep the bottom-most content clear of the system navigation bar.
                .windowInsetsPadding(WindowInsets.navigationBars)
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(20.dp, Alignment.Top)
        ) {
            // ── Deliverable 2: hide own-QR once a peer has been scanned ───────
            // When scannedPeer is non-null we are in "confirm peer" mode. The own-QR
            // card, instructions text, and Scan button belong only to the "show my QR"
            // mode and are hidden so the screen focuses on the scanned-peer confirmation.
            if (scannedPeer == null) {
                Text(
                    text = stringResource(R.string.pair_instructions),
                    style = MaterialTheme.typography.bodyLarge,
                    // voyf: use theme-adaptive token instead of hardcoded dark IdeText.
                    color = c.text,
                )
            }

            if (scannedPeer == null) {
            CopyPasteCard {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(28.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(16.dp)
                ) {
                    // Reserve a fixed-size slot for the QR area. Every state
                    // (loading / QR present / placeholder) renders into this same
                    // square, so the layout never reflows as the QR loads,
                    // appears, expires, or the countdown ticks — no jitter.
                    Box(
                        modifier = Modifier.size(QR_SLOT_SIZE_DP.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        val bmp = qrBitmap
                        when {
                            loading -> {
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally,
                                    verticalArrangement = Arrangement.spacedBy(12.dp)
                                ) {
                                    // voyf: use theme-adaptive tokens.
                                    CircularProgressIndicator(color = accentFill())
                                    Text(
                                        text = stringResource(R.string.status_pairing),
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = c.dim,
                                    )
                                }
                            }
                            bmp != null && !expired -> {
                                // First tap reveals (if blurred); tap while revealed regenerates.
                                Box(
                                    modifier = Modifier
                                        .size(QR_SLOT_SIZE_DP.dp)
                                        .clip(RoundedCornerShape(12.dp))
                                        .clickable {
                                            if (!qrRevealed) {
                                                qrRevealed = true
                                            } else {
                                                generateQr()
                                            }
                                        },
                                    contentAlignment = Alignment.Center,
                                ) {
                                    // ioco: small inset white plate sized exactly to the QR
                                    // with RadiusCard corners — NOT a full-bleed white box.
                                    // The glass surface behind it shows through the slot margins.
                                    Box(
                                        modifier = Modifier
                                            .size(QR_SLOT_SIZE_DP.dp)
                                            .padding(QR_PLATE_PADDING_DP.dp)
                                            .clip(RoundedCornerShape(12.dp))
                                            .background(androidx.compose.ui.graphics.Color.White),
                                        contentAlignment = Alignment.Center,
                                    ) {
                                        Image(
                                            bitmap = bmp.asImageBitmap(),
                                            contentDescription = stringResource(R.string.cd_pairing_qr),
                                            modifier = Modifier
                                                .size(QR_IMAGE_SIZE_DP.dp)
                                                .then(
                                                    if (!qrRevealed)
                                                        Modifier.blur(16.dp, BlurredEdgeTreatment.Unbounded)
                                                    else
                                                        Modifier
                                                )
                                        )
                                        // Scan line removed — QR is static after reveal (no idle animation).
                                    }
                                    // 9luz: tap-to-reveal — glass-tinted overlay instead of
                                    // dark 35% scrim. Accent-tinted translucent pill label
                                    // matches the calm glass aesthetic.
                                    if (!qrRevealed) {
                                        Box(
                                            modifier = Modifier
                                                .size(QR_SLOT_SIZE_DP.dp)
                                                .background(
                                                    accentTint(),
                                                    RoundedCornerShape(12.dp),
                                                ),
                                            contentAlignment = Alignment.Center,
                                        ) {
                                            Text(
                                                text = "Tap to reveal",
                                                style = MaterialTheme.typography.labelMedium,
                                                color = accentFill(),
                                                textAlign = TextAlign.Center,
                                                modifier = Modifier
                                                    .background(accentTint(), RadiusChip)
                                                    .padding(horizontal = 12.dp, vertical = 5.dp),
                                            )
                                        }
                                    }
                                }
                            }
                            else -> {
                                Icon(
                                    imageVector = Icons.Filled.QrCode,
                                    // CopyPaste-3nyq: announce the QR-loading state so AT
                                    // is not silent while the code is being generated.
                                    contentDescription = stringResource(R.string.cd_pairing_qr_loading),
                                    // voyf: theme-adaptive dim token.
                                    tint = c.dim,
                                    modifier = Modifier.size(96.dp),
                                )
                            }
                        }
                    }

                    // §10 Countdown text + drain bar — sits INSIDE the grey QR card,
                    // directly under the code, so the expiry is read together with the QR.
                    // CopyPaste-h59h: guard on !loading prevents a 1-frame flash of
                    // remainingSeconds==0 between LaunchedEffect(qr) restarts on
                    // visibility-restore after the previous token expired.
                    if (qr != null && !loading) {
                        when {
                            expired -> {
                                Text(
                                    text = stringResource(R.string.pair_token_expired),
                                    style = MaterialTheme.typography.bodyMedium,
                                    // voyf: theme-adaptive danger token.
                                    color = c.err,
                                )
                            }
                            else -> {
                                // !loading: outer if(qr != null && !loading) guards this
                                // block — no stale 0s frame (CopyPaste-h59h).
                                // Only the countdown timer — no redundant static note (HW-A5).
                                val urgent = remainingSeconds <= PAIR_TOKEN_URGENT_THRESHOLD_SECONDS
                                Text(
                                    text = stringResource(
                                        R.string.pair_token_expires_in_seconds,
                                        remainingSeconds
                                    ),
                                    style = MaterialTheme.typography.bodyMedium,
                                    // voyf: theme-adaptive warning/accent tokens.
                                    color = if (urgent) c.warn else accentFill(),
                                )
                                // Drain bar — 2dp thin track draining left-to-right over the TTL.
                                // Static (no pulse): progress bar pulse removed for calm UI.
                                Box(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(2.dp)
                                        .clip(RoundedCornerShape(999.dp))
                                        .background(c.mute.copy(alpha = 0.35f)),
                                ) {
                                    Box(
                                        modifier = Modifier
                                            .fillMaxWidth(qrCountdownProgress(remainingSeconds, PAIR_TOKEN_TTL_SECONDS))
                                            .height(2.dp)
                                            .background(if (urgent) c.warn else accentFill()),
                                    )
                                }
                            }
                        }
                    }
                }
            }

            // ── Deliverable 2: Scan button — only shown in own-QR mode ──────
            // Hidden when a peer has already been scanned (scannedPeer != null);
            // in that state the screen shows only the peer confirmation UI below.
            if (scannedPeer == null) {
                // Adopt CopyPasteButton secondary (glass) per action-button spec.
                CopyPasteButton(
                    onClick = { startScanFlow() },
                    variant = ButtonVariant.SECONDARY,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(text = stringResource(R.string.btn_scan_qr))
                }
            }
            } // end if (scannedPeer == null) — closes the QR card block

            // ── Deliverable 3: rich scanned-peer confirmation card ────────────
            // Shown INSTEAD of the own-QR once a peer has been scanned.
            //
            // CopyPaste-1jms.33 — two-phase flow:
            //   Phase 1 (pendingBootstrap == null): card shows name/address/fingerprint
            //     (from ScannedPairing — available immediately after scan).
            //     Button: "Pair & verify…" → runs PAKE bootstrap.
            //   Phase 2 (pendingBootstrap != null): card additionally shows the peer's
            //     model/OS/appVersion (from BootstrapResult — available after PAKE).
            //     Button: "Confirm & sync" → finalizes sync and roster commit.
            scannedPeer?.let { peer ->
                // 6i0w: replace raw Material Card with CopyPasteCard (glass surface).
                CopyPasteCard {
                    Column(
                        modifier = Modifier.padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        // lclr: avatar tile — 38dp accent-tint rounded tile with device initial.
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            val displayName = peer.deviceName.ifBlank { "Unknown device" }
                            Box(
                                modifier = Modifier
                                    .size(38.dp)
                                    .clip(RoundedCornerShape(10.dp))
                                    .background(accentTint()),
                                contentAlignment = Alignment.Center,
                            ) {
                                Text(
                                    text = displayName.take(1).uppercase(),
                                    style = MaterialTheme.typography.titleMedium,
                                    color = accentFill(),
                                )
                            }
                            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                Text(
                                    text = "Device to pair with",
                                    style = MaterialTheme.typography.labelLarge,
                                    // voyf: theme-adaptive accent token.
                                    color = accentFill(),
                                )
                                // Device name (from QR payload field 5)
                                Text(
                                    text = displayName,
                                    style = MaterialTheme.typography.titleSmall,
                                    // voyf: theme-adaptive text token.
                                    color = c.text,
                                )
                            }
                        }

                        // 483o: transport chip pill — RadiusChip (7dp) pill + hairline border + glyph.
                        Row(
                            modifier = Modifier
                                .background(c.info.copy(alpha = 0.12f), RadiusChip)
                                .border(0.5.dp, c.info.copy(alpha = 0.5f), RadiusChip)
                                .padding(horizontal = 9.dp, vertical = 3.dp),
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(text = "⟲", color = c.info, fontSize = 11.sp)
                            Text(
                                text = "P2P",
                                color = c.info,
                                fontSize = 11.sp,
                                style = MaterialTheme.typography.labelSmall,
                            )
                        }

                        // Address (host:port from QR payload field 6, if present)
                        if (peer.addrHint.isNotBlank()) {
                            Text(
                                text = "Address: ${peer.addrHint}",
                                style = MaterialTheme.typography.bodySmall,
                                // voyf: theme-adaptive dim token.
                                color = c.dim,
                            )
                        }
                        // 65gv (PG-47): show the FULL fingerprint in the SAS confirmation
                        // card — truncating a security fingerprint during verification
                        // defeats its purpose. The user must compare the whole value with
                        // the peer device. Matches macOS SAS modal which shows 64 chars.
                        Text(
                            text = "Fingerprint: ${peer.fingerprint}",
                            style = MaterialTheme.typography.bodySmall.copy(
                                fontFamily = MonoFontFamily,
                                fontSize = 11.sp,
                            ),
                            // voyf: theme-adaptive faint token (c.faint ≈ styleguide mute).
                            color = c.faint,
                            modifier = Modifier.clickable {
                                clipboardManager.setText(AnnotatedString(peer.fingerprint))
                                scope.launch {
                                    toastState.show("Fingerprint copied", GlassToastKind.ACCENT)
                                }
                            },
                        )

                        // CopyPaste-1jms.33: after PAKE bootstrap completes, show the
                        // peer's model/OS/appVersion in the card before the user
                        // confirms sync — matching macOS pairing-confirmation parity.
                        // peerMetaReviewRows() is a pure helper (DevicesUtils.kt) that
                        // filters out null/blank fields so no empty rows appear.
                        pendingBootstrap?.let { bs ->
                            val metaRows = peerMetaReviewRows(
                                peerModel = bs.peerModel,
                                peerOs = bs.peerOs,
                                peerAppVersion = bs.peerAppVersion,
                            )
                            if (metaRows.isNotEmpty()) {
                                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                    metaRows.forEach { (labelKey, value) ->
                                        // Resolve the string resource by name.
                                        // The label keys map 1:1 to strings.xml entries
                                        // (meta_label_model, meta_label_os, meta_label_version).
                                        val label = when (labelKey) {
                                            "meta_label_model" -> stringResource(R.string.meta_label_model)
                                            "meta_label_os" -> stringResource(R.string.meta_label_os)
                                            "meta_label_version" -> stringResource(R.string.meta_label_version)
                                            else -> labelKey
                                        }
                                        MetaRow(label = label, value = value)
                                    }
                                }
                            }
                        }
                    }
                }

                // CopyPaste-1jms.33: two-phase button.
                //   Phase 1 (pendingBootstrap == null): "Pair & verify…" — runs PAKE.
                //   Phase 2 (pendingBootstrap != null): "Confirm & sync" — finalizes.
                // Both phases use the PRIMARY variant; "Cancel" (ghost) lets the user
                // discard a verified result and go back to re-scan.
                if (pendingBootstrap == null) {
                    CopyPasteButton(
                        enabled = !syncing,
                        onClick = { runBootstrap(peer) },
                        variant = ButtonVariant.PRIMARY,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(text = if (syncing) stringResource(R.string.pair_verifying) else stringResource(R.string.pair_btn_verify))
                    }
                } else {
                    val bs = pendingBootstrap!!
                    CopyPasteButton(
                        enabled = !syncing,
                        onClick = { finalizeSync(peer, bs) },
                        variant = ButtonVariant.PRIMARY,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(text = if (syncing) stringResource(R.string.pair_verifying) else stringResource(R.string.pair_btn_confirm_sync))
                    }
                    // Cancel: discard the verified result so the user can re-scan.
                    CopyPasteButton(
                        enabled = !syncing,
                        onClick = {
                            pendingBootstrap = null
                            scannedPeer = null
                            pendingProvisioningRaw = null
                        },
                        variant = ButtonVariant.GHOST,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(text = stringResource(R.string.dialog_cancel))
                    }
                }
            }

            if (syncing) {
                // voyf: theme-adaptive accent token.
                CircularProgressIndicator(color = accentFill())
            }

            // ── Post-pair success popup ────────────────────────────────────────
            // Shown as a compact AlertDialog overlay once pairing completes.
            // The full syncResult string is still set (for logging / snackbar
            // fallback) but no longer displayed inline — the popup card takes over.
            pairedPeerForPopup?.let { justPaired ->
                PairedSuccessPopup(
                    peer = justPaired,
                    onDismiss = {
                        pairedPeerForPopup = null
                        onBack()
                    },
                )
            }

            // ── Paired-device roster (own-QR mode only) ───────────────────────
            // Show the persisted paired peer so the user can confirm which device
            // is paired. Only shown when not in the scanned-peer confirmation flow.
            if (scannedPeer == null && syncResult == null) {
                val pairedFingerprint = settings.pairedPeerFingerprint
                val pairedAddr = settings.pairedPeerSyncAddr
                if (pairedFingerprint.isNotBlank()) {
                    // 6i0w: replace raw Material Card with CopyPasteCard.
                    CopyPasteCard {
                        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            // lclr: avatar tile — 38dp accent-tint rounded tile.
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                            ) {
                                Box(
                                    modifier = Modifier
                                        .size(38.dp)
                                        .clip(RoundedCornerShape(10.dp))
                                        .background(accentTint()),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    // Device glyph placeholder — phone icon initial.
                                    Text(
                                        text = "📱",
                                        fontSize = 18.sp,
                                    )
                                }
                                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                    Text(
                                        text = "Paired device",
                                        style = MaterialTheme.typography.labelLarge,
                                        // voyf: theme-adaptive accent token.
                                        color = accentFill(),
                                    )
                                    // prld: status dot — danger for offline (unknown reachability here),
                                    // no redundant "Online/Offline" text label per styleguide.
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                                    ) {
                                        Box(
                                            modifier = Modifier
                                                .size(8.dp)
                                                .clip(CircleShape)
                                                // CopyPaste-5917.49: was c.err (hardcoded red even
                                                // when peer is reachable). PairScreen has no liveness
                                                // signal for the peer, so use c.faint (neutral grey)
                                                // to avoid misleading the user. Danger would only be
                                                // appropriate when confirmed unreachable.
                                                .background(c.faint),
                                        )
                                        if (pairedAddr.isNotBlank()) {
                                            Text(
                                                text = pairedAddr,
                                                style = MaterialTheme.typography.bodySmall,
                                                // voyf: theme-adaptive dim token.
                                                color = c.dim,
                                            )
                                        }
                                    }
                                }
                            }

                            // 10hh: fingerprint mono + 16…8 truncation.
                            val truncatedFp = formatPeerFingerprint(pairedFingerprint)
                            Text(
                                text = truncatedFp,
                                style = MaterialTheme.typography.bodySmall.copy(
                                    fontFamily = MonoFontFamily,
                                    fontSize = 11.sp,
                                ),
                                // voyf: theme-adaptive faint token.
                                color = c.faint,
                                modifier = Modifier.clickable {
                                    clipboardManager.setText(AnnotatedString(pairedFingerprint))
                                    scope.launch {
                                        toastState.show("Fingerprint copied", GlassToastKind.ACCENT)
                                    }
                                },
                            )
                        }
                    }
                }
            }
        }
    }
    GlassToastHost(state = toastState)
    } // end Box
}
