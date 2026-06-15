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
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.alpha
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
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.RadiusChip
import com.copypaste.android.ui.theme.CopyPasteTopBar
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import com.copypaste.android.ui.theme.LocalLiquidTokens
import com.copypaste.android.ui.theme.LocalPalette
import com.copypaste.android.ui.theme.Motion
import com.copypaste.android.ui.theme.auroraCanvas
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.motionDuration
import com.copypaste.android.ui.theme.paletteAurora
import com.copypaste.android.ui.theme.rememberReducedMotion
import com.copypaste.android.ui.theme.rememberTranslucency
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
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
 * bro9: reduced from 240dp to 160dp — closer to styleguide's ~132px calm scale
 * while still scannable on a phone.
 */
private const val QR_IMAGE_SIZE_DP = 160

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

/**
 * Build the human-readable label shown after a successful scan, e.g.
 * `"Pixel 8 (a1b2c3…)"`. Pure (no Android/FFI deps) so it is unit-testable on
 * the JVM. A blank device name falls back to the literal "device".
 */
internal fun formatScannedInfo(deviceName: String, fingerprint: String): String =
    "${deviceName.ifBlank { "device" }} ($fingerprint)"

/**
 * Sync-account provisioning extracted from the optional 6th field of a CPPAIR1/CPPAIR2
 * payload (H4: QR full provisioning). All fields are non-secret:
 * - [relayUrl]: HTTP relay base URL.
 * - [supabaseUrl]: Supabase project URL.
 * - [supabaseAnonKey]: Supabase publishable anon JWT (safe per Supabase docs).
 */
/** Holds the optional sync-provisioning data embedded in a CPPAIR2 QR payload. */
internal data class QrProvisioningData(
    val relayUrl: String?,
    val supabaseUrl: String?,
    val supabaseAnonKey: String?,
)

/**
 * Extract sync-provisioning from the optional 6th field of a bare CPPAIR2 payload.
 *
 * CPPAIR2 wire format (body after magic prefix):
 *   [0] fp_b64url  [1] token_b64url  [2] device_id_b64url  [3] name_b64url
 *   [4] addr_b64url  [5] prov_b64url (optional)
 *
 * All 6 body fields are base64url (no dots), so `split(".", limit=7)` on the
 * full string cleanly isolates the provisioning field at index 6 (0-based,
 * counting the magic prefix at index 0).
 *
 * For CPPAIR1 payloads provisioning is not supported — addr_hint in v1 is the
 * raw address string and may contain IPv4 dots that collide with the delimiter.
 *
 * Returns `null` when the field is absent, empty, or cannot be decoded. A
 * decode failure here is always silent: provisioning is advisory and must never
 * break pairing.
 *
 * Pure Kotlin; no FFI dependency, so it works even in stub mode.
 */
internal fun extractQrProvisioning(barePayload: String): QrProvisioningData? {
    // Only handle CPPAIR2; CPPAIR1 addr_hint contains IPv4 dots that make
    // field 5 ambiguous without knowing the addr_hint length.
    val bare = barePayload.trim()
    if (!bare.startsWith("CPPAIR2.")) return null
    // Full string: CPPAIR2 . fp . tok . id . name . addr_b64 [. prov_b64]
    // Indices:        0       1    2    3    4       5           6
    val parts = bare.split(".", limit = 7)
    if (parts.size < 7) return null  // no provisioning field present
    val provB64 = parts[6].trim()
    if (provB64.isEmpty()) return null
    return try {
        // base64url: replace url-safe chars to standard before decoding.
        val bytes = android.util.Base64.decode(
            provB64.replace('-', '+').replace('_', '/'),
            android.util.Base64.NO_WRAP or android.util.Base64.NO_PADDING,
        )
        val json = String(bytes, Charsets.UTF_8)
        QrProvisioningData(
            relayUrl = extractJsonString(json, "ru"),
            supabaseUrl = extractJsonString(json, "su"),
            supabaseAnonKey = extractJsonString(json, "sk"),
        )
    } catch (_: Exception) {
        null // Corrupt/unknown field �� silently ignore; pairing is unaffected.
    }
}

/**
 * Minimal JSON string extractor for a flat `{"k":"v",...}` object.
 * Returns the string value for [key], or `null` when absent or not a string.
 * Handles `\"` and `\\` escapes; sufficient for URLs and JWTs.
 */
private fun extractJsonString(json: String, key: String): String? {
    val needle = "\"$key\":\""
    val start = json.indexOf(needle).takeIf { it >= 0 } ?: return null
    val valueStart = start + needle.length
    val sb = StringBuilder()
    var i = valueStart
    while (i < json.length) {
        when (val c = json[i]) {
            '"' -> return sb.toString().takeIf { it.isNotEmpty() }
            '\\' -> {
                i++
                if (i >= json.length) return null
                when (json[i]) {
                    '"' -> sb.append('"')
                    '\\' -> sb.append('\\')
                    'n' -> sb.append('\n')
                    'r' -> sb.append('\r')
                    't' -> sb.append('\t')
                    else -> { sb.append('\\'); sb.append(json[i]) }
                }
            }
            else -> sb.append(c)
        }
        i++
    }
    return null // Unterminated string
}

/**
 * Apply [prov] to [settings] using fill-missing semantics: only write a field when
 * the corresponding settings value is currently blank. Never overwrites an existing
 * local configuration — the user may have set up their own relay/Supabase/passphrase.
 *
 * Returns a list of field names that were actually written (for logging).
 * Call only from a background thread (Settings uses SharedPreferences I/O).
 */
internal fun applyQrProvisioning(prov: QrProvisioningData, settings: Settings): List<String> {
    val applied = mutableListOf<String>()
    prov.relayUrl?.takeIf { it.isNotBlank() }?.let { url ->
        if (settings.relayUrl.isBlank()) {
            settings.relayUrl = url
            applied += "relayUrl"
        }
    }
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
    return applied
}

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
    // runPairAndSync, only once the PAKE bootstrap succeeds) — NOT at scan/parse time. A
    // hostile cppair:// could otherwise silently seed attacker-controlled URLs on a fresh
    // install before any consent.
    var pendingProvisioningRaw by remember { mutableStateOf<String?>(null) }
    var syncing by remember { mutableStateOf(false) }
    var syncResult by remember { mutableStateOf<String?>(null) }
    // Holds the just-paired peer to display in the compact success popup.
    // Set at the end of runPairAndSync; cleared when the popup is dismissed.
    var pairedPeerForPopup by remember { mutableStateOf<PairedPeer?>(null) }
    var remainingSeconds by remember { mutableStateOf(0) }
    // QR blur+reveal: starts blurred; first tap reveals; regeneration re-blurs.
    var qrRevealed by remember { mutableStateOf(false) }
    val toastState = remember { GlassToastState() }
    val scope = rememberCoroutineScope()
    val clipboardManager = LocalClipboardManager.current
    val errorTemplate = stringResource(R.string.error_pairing)

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
                qrRevealed = false  // re-blur whenever a fresh QR is generated
            } catch (e: Exception) {
                errorMessage = e.message ?: e.javaClass.simpleName
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
            syncResult = null
            scannedInfo = formatScannedInfo(info.deviceName, info.fingerprint)
            // CopyPaste-tqt0: retain the raw QR payload but DO NOT apply its 6th-field
            // relay/Supabase provisioning yet. Applying at scan time let a hostile QR
            // seed attacker-controlled URLs into Settings on a fresh install before the
            // user consented. Provisioning is now applied inside runPairAndSync, only
            // after the PAKE bootstrap (SAS confirmation) succeeds.
            pendingProvisioningRaw = contents
        } catch (e: Exception) {
            errorMessage = e.message ?: "Invalid pairing code"
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
            errorMessage = "Could not open the camera scanner: " +
                (e.message ?: e.javaClass.simpleName) +
                ". You can pair by displaying this device's QR instead."
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
                errorMessage = "Camera permission is permanently denied. Enable it in " +
                    "Settings (just opened), or use the QR display flow on this device instead."
            } else {
                errorMessage = "Camera permission is required to scan a pairing QR code. " +
                    "Grant it in Settings, or use the QR display flow on this device instead."
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
            errorMessage = "Camera permission is permanently denied. Enable it in " +
                "Settings (just opened) to scan a pairing QR."
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
    // this `runPairAndSync` only performs ONE sync at pairing time. To sync in
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
    fun runPairAndSync(peer: ScannedPairing) {
        if (syncing) return
        // CopyPaste-tqt0: snapshot the retained QR payload now; its 6th-field
        // provisioning is applied below ONLY after the PAKE bootstrap succeeds.
        val provisioningRaw = pendingProvisioningRaw
        scope.launch {
            syncing = true
            syncResult = null
            try {
                val key = settings.encryptionKey
                // Captured inside the IO block (where `bootstrap` is in scope) so the
                // success popup below can look the freshly-paired peer up by fingerprint.
                var pairedFingerprint: String? = null
                val message = withContext(Dispatchers.IO) {
                    val cert = deviceKeyStore.getOrCreate()
                    // Path A: advertise THIS device's inbound mTLS listener address
                    // so the macOS peer persists it and can dial back (macOS→Android
                    // direction). The listener is bound by [ClipboardService]; its
                    // OS-assigned port is published in [ClipboardService.activeListenerPort].
                    // When the listener is not running yet (port == 0) or this device
                    // has no LAN IPv4, fall back to "" — the old behavior, where only
                    // the Android→macOS dial works until the listener comes up.
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
                    val bootstrap = bootstrapPairInitiator(
                        addrHint = peer.addrHint,
                        certDer = cert.certDer,
                        keyDer = cert.keyDer,
                        pakePassword = peer.pakePassword,
                        // Advertise our dialable listener address so the peer dials back.
                        syncAddr = ownSyncAddr,
                        // A phone scanning a configured PC carries nothing of its
                        // own — it RECEIVES the PC's sync provisioning in the
                        // response (bootstrap.peerProvisioning), applied below.
                        localProvisioning = null,
                        // HB-1a (ABI 14): send THIS device's own metadata so the
                        // PC's device card shows real Android info. public_ip is
                        // not collected here (lib.rs passes None).
                        deviceName = android.os.Build.MODEL ?: "Android",
                        deviceModel = android.os.Build.MODEL ?: "Android",
                        osVersion = "Android " + android.os.Build.VERSION.RELEASE,
                        appVersion = BuildConfig.VERSION_NAME,
                        localIp = lanIp,
                    )

                    // QR full-provisioning: if the paired PC carried its sync
                    // config in the pairing payload, fill any field this device
                    // has not already configured. NEVER overwrite an existing
                    // local value (mirror the daemon's fill-missing rule) — the
                    // user may have set up their own Supabase/relay/passphrase.
                    // Runs inside withContext(Dispatchers.IO) so the wrapped-key
                    // write + SharedPreferences IO stay off the main thread.
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
                        // Convert the FFI List<UByte> to ByteArray.
                        prov.derivedSyncKey?.takeIf { it.isNotEmpty() }?.let { keyUBytes ->
                            if (settings.cloudSyncKeyDirect == null) {
                                val keyBytes = ByteArray(keyUBytes.size) { keyUBytes[it].toByte() }
                                settings.cloudSyncKeyDirect = keyBytes
                                applied += "derivedSyncKey"
                            }
                        }
                        // Log WHAT was provisioned, never the key bytes.
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
                    // CopyPaste-tqt0: NOW (post-bootstrap = the user confirmed pairing and
                    // PAKE/SAS succeeded) apply the QR's 6th-field relay/Supabase provisioning.
                    // Fill-missing only (applyQrProvisioning never overwrites a configured
                    // field). This is the off-LAN / relay-only path; the PAKE-response
                    // provisioning above covers the on-LAN bootstrap case. Deferring to here
                    // ensures a hostile cppair:// cannot seed Settings without consent.
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
                    // Pass the local denylist into the ABI-9 syncWithPeer and skip a
                    // dial entirely if THIS peer is itself revoked.
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
                    // HB-7b: route each received item BY CONTENT TYPE, mirroring
                    // FgsSyncLoop.storeSyncedItem. The old code force-decoded EVERY
                    // item as UTF-8 text, so image/file frames became garbage and
                    // were dropped (a cause of "received N stored 0"). All items
                    // persist under the peer's STABLE item_id (overrideId) so a
                    // later re-sync reuses it instead of minting a duplicate.
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
                    // Persist (APPEND) the peer into the multi-peer roster for
                    // future syncs — a second pairing must NOT discard the first.
                    // The session key is KEK-wrapped before it touches the roster
                    // JSON so the background dialer in FgsSyncLoop can re-open a
                    // sync session unattended without re-running the PAKE bootstrap
                    // / re-scanning a QR.
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
                            // received over the authenticated tunnel so Wave 3
                            // renders the device card.
                            peerModel = bootstrap.peerModel,
                            peerOs = bootstrap.peerOs,
                            peerAppVersion = bootstrap.peerAppVersion,
                            peerLocalIp = bootstrap.peerLocalIp,
                            peerPublicIp = bootstrap.peerPublicIp,
                            // CopyPaste-3k6m (ABI 17): persist the peer's stable device UUID so
                            // OriginDeviceFilter resolves clipboard item names by UUID.
                            // Primary: from the bootstrap tunnel's PeerMeta (ABI 17).
                            // Fallback: the ScannedPairing.deviceId from the QR payload carries the
                            // same UUID, so pairs made before the macOS daemon sends it in PeerMeta
                            // still populate this field immediately at pair time.
                            peerDeviceId = bootstrap.peerDeviceId?.takeIf { it.isNotBlank() }
                                ?: peer.deviceId.takeIf { it.isNotBlank() },
                        )
                    )
                    pairedFingerprint = bootstrap.peerFingerprint
                    val peerCount = settings.pairedPeers.size
                    // HB-7a (ABI 14): surface the per-reason drop counters so a
                    // "received N stored 0" outcome reveals WHY items dropped.
                    val skipped = "skipped: legacy ${result.itemsSkippedLegacy} / " +
                        "decrypt ${result.itemsSkippedDecryptFail} / " +
                        "type ${result.itemsSkippedUnknownType} / " +
                        "blob ${result.itemsSkippedMissingBlob}"
                    "Paired with ${peer.deviceName.ifBlank { "device" }} — received ${result.itemsReceived} item(s), stored $stored ($skipped), sent ${result.itemsSent}. ($peerCount paired device(s))"
                }
                // Surface the just-persisted peer for the compact success popup.
                // Look it up from the roster by fingerprint so all ABI-14 fields
                // (model/OS/version/IPs) are present for the card.
                pairedPeerForPopup = settings.pairedPeers
                    .firstOrNull { it.fingerprint == pairedFingerprint }
                syncResult = message
                scannedPeer = null
                // CopyPaste-tqt0: provisioning has been applied (post-confirmation);
                // drop the retained raw payload so it cannot leak into a later pairing.
                pendingProvisioningRaw = null
            } catch (e: Exception) {
                errorMessage = e.message ?: e.javaClass.simpleName
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
    LaunchedEffect(qr) {
        if (qr == null) return@LaunchedEffect
        remainingSeconds = PAIR_TOKEN_TTL_SECONDS
        while (remainingSeconds > 0) {
            delay(1000)
            remainingSeconds -= 1
        }
        // QR expired — auto-regenerate only when the success popup is not up.
        if (pairedPeerForPopup == null) generateQr()
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
            errorMessage = e.message ?: e.javaClass.simpleName
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
            syncResult = null
            scannedInfo = formatScannedInfo(info.deviceName, info.fingerprint)
            // CopyPaste-tqt0: retain the raw deep-link payload but DO NOT apply its
            // 6th-field provisioning here. A crafted cppair:// link must not be able to
            // write relay/Supabase URLs into Settings before the user confirms pairing.
            // Applied inside runPairAndSync once the PAKE bootstrap succeeds.
            pendingProvisioningRaw = payload
        } catch (e: Exception) {
            errorMessage = e.message ?: "Invalid pairing code"
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
        toastState.show(errorTemplate.format(msg), GlassToastKind.DANGER)
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

    val c = LocalIdeColors.current
    val lt = LocalLiquidTokens.current
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()
    val reduced = rememberReducedMotion()

    // Entrance alpha for the QR card — fades in on first composition.
    var pairEntered by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { pairEntered = true }
    val cardEntranceAlpha by animateFloatAsState(
        targetValue = if (pairEntered) 1f else 0f,
        animationSpec = tween(if (reduced) 0 else motionDuration(Motion.Slow)),
        label = "pairCardAlpha",
    )

    Box(Modifier.fillMaxSize()) {
    // 9g57: aurora canvas backdrop — mirrors DevicesScreen pattern.
    Scaffold(
        modifier = if (translucent) modifier.auroraCanvas(dark, paletteAurora(LocalPalette.current)) else modifier,
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
            CopyPasteCard(modifier = Modifier.alpha(cardEntranceAlpha)) {
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
                                    CircularProgressIndicator(color = c.accent)
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
                                        // Animated scan line — drawn over the QR when revealed and
                                        // reduced motion is OFF. Sweeps top→bottom in 2.5 s on a
                                        // FastOutSlowIn curve, matching the web qrScan keyframe.
                                        // The line is a 2dp accent gradient with a glow shadow,
                                        // zeroed (invisible) at both ends so it fades gracefully.
                                        if (qrRevealed && !reduced) {
                                            val scanTransition = rememberInfiniteTransition(label = "qrScan")
                                            val scanProgress by scanTransition.animateFloat(
                                                initialValue = 0f,
                                                targetValue = 1f,
                                                animationSpec = infiniteRepeatable(
                                                    animation = tween(
                                                        // 2500ms matches web qrScan 2.5s.
                                                        durationMillis = (2500 * lt.motionScale).toInt(),
                                                        easing = FastOutSlowInEasing,
                                                    ),
                                                    repeatMode = RepeatMode.Restart,
                                                ),
                                                label = "qrScanProgress",
                                            )
                                            // Fade the line in from 0 → opaque at 12% → opaque at
                                            // 88% → back to 0, matching the web opacity keyframes.
                                            val scanAlpha = when {
                                                scanProgress < 0.12f -> scanProgress / 0.12f
                                                scanProgress > 0.88f -> (1f - scanProgress) / 0.12f
                                                else -> 1f
                                            } * 0.9f

                                            Box(
                                                modifier = Modifier
                                                    .size(QR_IMAGE_SIZE_DP.dp)
                                                    .drawBehind {
                                                        // Compute Y position of the scan line.
                                                        val scanY = size.height * scanProgress
                                                        // 2dp glow line: accent gradient fades to transparent at edges.
                                                        drawRect(
                                                            brush = Brush.horizontalGradient(
                                                                colors = listOf(
                                                                    androidx.compose.ui.graphics.Color.Transparent,
                                                                    c.accent.copy(alpha = scanAlpha),
                                                                    c.accent.copy(alpha = scanAlpha),
                                                                    androidx.compose.ui.graphics.Color.Transparent,
                                                                ),
                                                            ),
                                                            topLeft = Offset(0f, scanY - 1.dp.toPx()),
                                                            size = androidx.compose.ui.geometry.Size(size.width, 2.dp.toPx()),
                                                        )
                                                        // Soft glow halo below the line (widens the visible effect).
                                                        drawRect(
                                                            brush = Brush.verticalGradient(
                                                                colors = listOf(
                                                                    c.accent.copy(alpha = scanAlpha * 0.35f),
                                                                    androidx.compose.ui.graphics.Color.Transparent,
                                                                ),
                                                                startY = scanY,
                                                                endY = scanY + 18.dp.toPx(),
                                                            ),
                                                            topLeft = Offset(0f, scanY),
                                                            size = androidx.compose.ui.geometry.Size(size.width, 18.dp.toPx()),
                                                        )
                                                    },
                                            )
                                        }
                                    }
                                    // 9luz: tap-to-reveal — glass-tinted overlay instead of
                                    // dark 35% scrim. Accent-tinted translucent pill label
                                    // matches the calm glass aesthetic.
                                    if (!qrRevealed) {
                                        Box(
                                            modifier = Modifier
                                                .size(QR_SLOT_SIZE_DP.dp)
                                                .background(
                                                    c.accentDim,
                                                    RoundedCornerShape(12.dp),
                                                ),
                                            contentAlignment = Alignment.Center,
                                        ) {
                                            Text(
                                                text = "Tap to reveal",
                                                style = MaterialTheme.typography.labelMedium,
                                                color = c.accent,
                                                textAlign = TextAlign.Center,
                                                modifier = Modifier
                                                    .background(c.accentDim, RadiusChip)
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

                    // Countdown sits INSIDE the grey QR card, directly under the
                    // code, so the expiry is read together with the QR.
                    if (qr != null) {
                        when {
                            expired -> {
                                Text(
                                    text = stringResource(R.string.pair_token_expired),
                                    style = MaterialTheme.typography.bodyMedium,
                                    // voyf: theme-adaptive danger token.
                                    color = c.danger,
                                )
                            }
                            else -> {
                                // Only the countdown timer — no redundant static note (HW-A5).
                                val urgent = remainingSeconds <= PAIR_TOKEN_URGENT_THRESHOLD_SECONDS
                                Text(
                                    text = stringResource(
                                        R.string.pair_token_expires_in_seconds,
                                        remainingSeconds
                                    ),
                                    style = MaterialTheme.typography.bodyMedium,
                                    // voyf: theme-adaptive warning/accent tokens.
                                    color = if (urgent) c.warning else c.accent,
                                )
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
            // Shown INSTEAD of the own-QR once a peer has been scanned. Surfaces
            // all available fields from ScannedPairing: device name, address
            // (host:port from addrHint), and fingerprint. Model/OS/appVersion are
            // not available until after the PAKE meta-exchange (BootstrapResult
            // fields); they will be shown on the post-pair success screen once
            // runPairAndSync() completes and stores them in the peer roster.
            // TODO(post-PAKE-meta): show peerModel/peerOs/peerAppVersion here
            //   once the BootstrapResult is threaded back to the UI after
            //   bootstrapPairInitiator completes. Those fields live in
            //   BootstrapResult.peerModel/peerOs/peerAppVersion (ABI 14) but
            //   runPairAndSync currently only persists them to Settings.pairedPeers.
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
                                    .background(c.accentDim),
                                contentAlignment = Alignment.Center,
                            ) {
                                Text(
                                    text = displayName.take(1).uppercase(),
                                    style = MaterialTheme.typography.titleMedium,
                                    color = c.accent,
                                )
                            }
                            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                                Text(
                                    text = "Device to pair with",
                                    style = MaterialTheme.typography.labelLarge,
                                    // voyf: theme-adaptive accent token.
                                    color = c.accent,
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
                                .background(c.infoDim, RadiusChip)
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
                        // 10hh: fingerprint mono + 16…8 truncation (matches DevicesActivity).
                        val truncatedFp = formatPeerFingerprint(peer.fingerprint)
                        Text(
                            text = "Fingerprint: $truncatedFp",
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
                        // NOTE: model/OS/appVersion become available after the PAKE
                        // bootstrap completes — see TODO above.
                    }
                }

                // Adopt CopyPasteButton primary for the pairing action.
                CopyPasteButton(
                    enabled = !syncing,
                    onClick = { runPairAndSync(peer) },
                    variant = ButtonVariant.PRIMARY,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(text = if (syncing) "Pairing…" else "Pair & sync")
                }
            }

            if (syncing) {
                // voyf: theme-adaptive accent token.
                CircularProgressIndicator(color = c.accent)
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
                                        .background(c.accentDim),
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
                                        color = c.accent,
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
                                                // prld: offline = danger (not faint grey).
                                                .background(c.danger),
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

// ─────────────────────────────────────────────────────────────────────────────
// Post-pairing success popup
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Compact AlertDialog shown immediately after QR pairing succeeds.
 *
 * Renders the just-paired device as a tidy card — name + status dot (always
 * "Paired ✓" since we just finished), model/OS if the peer sent them over the
 * authenticated tunnel (ABI 14 peerModel/peerOs), and a short fingerprint.
 * The full verbose sync summary is intentionally omitted here (it remains in
 * [syncResult] for debug logging); this card surfaces only what the user cares
 * about: "which device did I just pair with?"
 *
 * Dismisses via "Done" → [onDismiss], which clears [pairedPeerForPopup] and
 * calls [onBack] to return to the Devices list.
 */
@Composable
private fun PairedSuccessPopup(
    peer: PairedPeer,
    onDismiss: () -> Unit,
) {
    // voyf: read theme-adaptive ramp — no hardcoded Ide* constants.
    val c = LocalIdeColors.current
    GlassAlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = "Paired successfully",
                style = MaterialTheme.typography.titleMedium,
                // voyf: theme-adaptive success token.
                color = c.success,
            )
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                // ── Avatar + name + status row ────────────────────────────────
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    // lclr: 38dp avatar tile in success-tint (peer is now online/paired).
                    val displayName = peer.name.ifBlank { "Paired device" }
                    Box(
                        modifier = Modifier
                            .size(38.dp)
                            .clip(RoundedCornerShape(10.dp))
                            .background(c.successDim),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = displayName.take(1).uppercase(),
                            style = MaterialTheme.typography.titleMedium,
                            color = c.success,
                        )
                    }
                    Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            // prld: status dot 8dp (not 10dp), success color for paired state.
                            Box(
                                modifier = Modifier
                                    .size(8.dp)
                                    .clip(CircleShape)
                                    .background(c.success),
                            )
                            Text(
                                text = displayName,
                                style = MaterialTheme.typography.titleSmall,
                                // voyf: theme-adaptive text token.
                                color = c.text,
                            )
                        }
                        Text(
                            text = "Paired ✓",
                            style = MaterialTheme.typography.labelMedium,
                            color = c.success,
                        )
                    }
                }

                Spacer(Modifier.height(4.dp))

                // ── Device metadata rows (only non-blank fields) ─────────────
                Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    peer.peerModel?.takeIf { it.isNotBlank() }?.let {
                        PopupMetaRow(label = "Model", value = it)
                    }
                    peer.peerOs?.takeIf { it.isNotBlank() }?.let {
                        PopupMetaRow(label = "OS", value = it)
                    }
                    peer.peerAppVersion?.takeIf { it.isNotBlank() }?.let {
                        PopupMetaRow(label = "Version", value = it)
                    }
                    // 10hh: mono truncated fingerprint — 16…8 (via formatPeerFingerprint).
                    val shortFp = formatPeerFingerprint(peer.fingerprint)
                    PopupMetaRow(label = "Fingerprint", value = shortFp)
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                // voyf: theme-adaptive accent token.
                Text("Done", color = c.accent)
            }
        },
    )
}

/** Single label+value row for [PairedSuccessPopup]. */
@Composable
private fun PopupMetaRow(label: String, value: String) {
    // voyf: read theme-adaptive ramp — no hardcoded Ide* constants.
    val c = LocalIdeColors.current
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodySmall,
            // voyf: theme-adaptive dim token.
            color = c.dim,
            modifier = Modifier.width(72.dp),
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodySmall,
            // voyf: theme-adaptive text token.
            color = c.text,
        )
    }
}

/**
 * Best-effort lookup of this device's site-local IPv4 address (e.g. 192.168.x.x,
 * 10.x.x.x), used to build the inbound listener's advertised `sync_addr` at pair
 * time so the macOS peer can dial back over the LAN.
 *
 * Enumerates active, non-loopback interfaces and returns the first site-local
 * IPv4 (skipping link-local 169.254.x.x and IPv6). Returns null when no such
 * address exists (no Wi-Fi / cellular-only), in which case the caller falls back
 * to advertising no address (Android→macOS dial only).
 *
 * No WifiManager dependency: NetworkInterface enumeration works for both Wi-Fi
 * and other LAN interfaces without the ACCESS_WIFI_STATE permission.
 *
 * `internal` so the discovery pairing path ([DevicesActivity]) reuses the SAME
 * helper for HB-1a `local_ip` instead of duplicating the enumeration.
 */
internal fun lanIpv4Address(): String? {
    return try {
        java.net.NetworkInterface.getNetworkInterfaces()?.toList()
            ?.asSequence()
            ?.filter { runCatching { it.isUp && !it.isLoopback }.getOrDefault(false) }
            ?.flatMap { it.inetAddresses.toList().asSequence() }
            ?.filterIsInstance<java.net.Inet4Address>()
            ?.firstOrNull { it.isSiteLocalAddress }
            ?.hostAddress
    } catch (e: Exception) {
        android.util.Log.w("PairActivity", "lanIpv4Address lookup failed: ${e.message}")
        null
    }
}

