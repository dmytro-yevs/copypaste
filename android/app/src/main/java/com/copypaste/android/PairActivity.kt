package com.copypaste.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCode
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.BorderStroke
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.IdeAccent
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeText
import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.copypaste_android.ScannedPairing
import uniffi.copypaste_android.bootstrapPairInitiator
import uniffi.copypaste_android.syncWithPeer

/** Pairing token lifetime in seconds — mirrors the Rust core's PAKE session TTL. */
private const val PAIR_TOKEN_TTL_SECONDS = 120

/** Threshold below which the countdown switches to an urgency color. */
private const val PAIR_TOKEN_URGENT_THRESHOLD_SECONDS = 15

/**
 * Side of the rendered QR image, in dp. The QR, the placeholder icon, and the
 * loading spinner all sit inside a reserved box of this size (plus
 * [QR_PLATE_PADDING_DP] of white backing) so the layout never reflows when the
 * content swaps between loading / present / placeholder. This is the single
 * source of truth for the QR's on-screen size, keeping the image and its
 * reserved container in lock-step (BUG 2).
 */
private const val QR_IMAGE_SIZE_DP = 240

/** White backing-plate padding around the QR, in dp (each side). */
private const val QR_PLATE_PADDING_DP = 12

/**
 * Fixed side of the reserved QR slot, in dp: the QR image plus its white plate
 * padding on both sides. Every QR-area state renders into a box of exactly this
 * size so the screen stays visually stable (no jitter — BUG 1).
 */
private const val QR_SLOT_SIZE_DP = QR_IMAGE_SIZE_DP + QR_PLATE_PADDING_DP * 2

/** Pixel resolution of the QR bitmap passed to ZXing. Single source of truth. */
private const val QR_BITMAP_PX = 512

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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // Extract payload from a cold-start deep-link intent, if present.
        handleDeepLinkIntent(intent)
        setContent {
            CopyPasteTheme {
                PairScreen(
                    onBack = { finish() },
                    incomingDeepLinkPayload = deepLinkPayload.value,
                    onDeepLinkConsumed = { deepLinkPayload.value = null },
                    incomingDeepLinkError = deepLinkError.value,
                    onDeepLinkErrorConsumed = { deepLinkError.value = null },
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
     * Route an incoming intent: valid CPPAIR1 payload → deepLinkPayload;
     * cppair:// URI with unrecognised payload → deepLinkError for user feedback.
     */
    private fun handleDeepLinkIntent(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        val uri: Uri = intent.data ?: return
        if (uri.scheme != "cppair" || uri.host != "pair") return
        val p = uri.getQueryParameter("p") ?: return
        if (p.startsWith("CPPAIR1.")) {
            deepLinkPayload.value = p
        } else {
            // Payload present but not a recognised CPPAIR1 token — surface to user.
            deepLinkError.value = "Invalid pairing link"
        }
    }
}

/** Render `text` as a square QR [Bitmap] of `sizePx` pixels using ZXing. */
private fun encodeQrBitmap(text: String, sizePx: Int): Bitmap {
    val matrix = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, sizePx, sizePx)
    val bmp = Bitmap.createBitmap(sizePx, sizePx, Bitmap.Config.RGB_565)
    for (x in 0 until sizePx) {
        for (y in 0 until sizePx) {
            bmp.setPixel(x, y, if (matrix[x, y]) Color.BLACK else Color.WHITE)
        }
    }
    return bmp
}

/**
 * Build the human-readable label shown after a successful scan, e.g.
 * `"Pixel 8 (a1b2c3…)"`. Pure (no Android/FFI deps) so it is unit-testable on
 * the JVM. A blank device name falls back to the literal "device".
 */
internal fun formatScannedInfo(deviceName: String, fingerprint: String): String =
    "${deviceName.ifBlank { "device" }} ($fingerprint)"

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
    var syncing by remember { mutableStateOf(false) }
    var syncResult by remember { mutableStateOf<String?>(null) }
    var remainingSeconds by remember { mutableStateOf(0) }
    // QR is blurred until the user taps to reveal. Once revealed it stays
    // visible — including after a tap-triggered regeneration (HW-A5).
    var qrBlurred by remember { mutableStateOf(true) }
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val clipboardManager = LocalClipboardManager.current
    val errorTemplate = stringResource(R.string.error_pairing)
    val dismissLabel = stringResource(R.string.snackbar_dismiss)

    val expired = qr != null && remainingSeconds <= 0

    // Shared helper: generate a new QR and render its bitmap.
    // keepVisible=true keeps qrBlurred=false after generation (used for
    // tap-regen and auto-regen on expiry so the QR stays readable).
    fun generateQr(keepVisible: Boolean) {
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
                if (keepVisible) qrBlurred = false
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
            errorMessage = "Camera permission is required to scan a pairing QR code. " +
                "Grant it in Settings, or use the QR display flow on this device instead."
        }
    }

    fun startScanFlow() {
        val hasCamera = ContextCompat.checkSelfPermission(
            context, Manifest.permission.CAMERA
        ) == PackageManager.PERMISSION_GRANTED
        if (hasCamera) {
            launchScanner()
        } else {
            cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
        }
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
        scope.launch {
            syncing = true
            syncResult = null
            try {
                val key = settings.encryptionKey
                val message = withContext(Dispatchers.IO) {
                    val cert = deviceKeyStore.getOrCreate()
                    val bootstrap = bootstrapPairInitiator(
                        addrHint = peer.addrHint,
                        certDer = cert.certDer,
                        keyDer = cert.keyDer,
                        pakePassword = peer.pakePassword,
                        syncAddr = "",
                    )
                    val localItems = repository.localItemsForSync(key)
                    val result = syncWithPeer(
                        peerAddr = bootstrap.peerSyncAddr,
                        peerFingerprint = bootstrap.peerFingerprint,
                        sessionKey = bootstrap.sessionKey,
                        certDer = cert.certDer,
                        keyDer = cert.keyDer,
                        localItems = localItems,
                    )
                    var stored = 0
                    for (item in result.items) {
                        val plaintext = String(
                            ByteArray(item.plaintext.size) { item.plaintext[it].toByte() },
                            Charsets.UTF_8,
                        )
                        // Persist under the peer's STABLE item_id so a later
                        // re-sync of this clip reuses it (no duplicate on the
                        // originating device). itemId doubles as the dedup key.
                        if (repository.storeItem(plaintext, key, overrideId = item.itemId)
                                .isNotEmpty()
                        ) {
                            stored += 1
                        }
                    }
                    // Persist the peer for future syncs. The session key is
                    // stored securely (KEK-wrapped) so the background dialer in
                    // FgsSyncLoop can re-open a sync session unattended without
                    // re-running the PAKE bootstrap / re-scanning a QR.
                    settings.pairedPeerFingerprint = bootstrap.peerFingerprint
                    settings.pairedPeerSyncAddr = bootstrap.peerSyncAddr
                    settings.pairedPeerSessionKey =
                        ByteArray(bootstrap.sessionKey.size) { bootstrap.sessionKey[it].toByte() }
                    "Paired with ${peer.deviceName.ifBlank { "device" }} — received ${result.itemsReceived} item(s), stored $stored, sent ${result.itemsSent}."
                }
                syncResult = message
                scannedPeer = null
            } catch (e: Exception) {
                errorMessage = e.message ?: e.javaClass.simpleName
            } finally {
                syncing = false
            }
        }
    }

    // Countdown ticker — restarts whenever a fresh QR is issued.
    // When the countdown reaches 0, auto-regenerate the QR and keep it
    // visible (qrBlurred=false) so the user doesn't need to tap again (HW-A5).
    LaunchedEffect(qr) {
        if (qr == null) return@LaunchedEffect
        remainingSeconds = PAIR_TOKEN_TTL_SECONDS
        while (remainingSeconds > 0) {
            delay(1000)
            remainingSeconds -= 1
        }
        // QR expired — auto-regenerate and show unblurred.
        generateQr(keepVisible = true)
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
            // Initial load: keep blurred so user taps to reveal.
        } catch (e: Exception) {
            errorMessage = e.message ?: e.javaClass.simpleName
        } finally {
            loading = false
        }
    }

    // Consume an incoming cppair:// deep-link payload from an external QR scanner
    // (e.g. Google Lens).  The payload is the raw CPPAIR1.… string, identical to
    // what the in-app ZXing scanner would return — so we feed it through exactly
    // the same parsePairing path and surface the same confirmation UI.
    LaunchedEffect(incomingDeepLinkPayload) {
        val payload = incomingDeepLinkPayload ?: return@LaunchedEffect
        try {
            val info = withContext(Dispatchers.IO) { parsePairing(payload) }
            scannedPeer = info
            syncResult = null
            scannedInfo = formatScannedInfo(info.deviceName, info.fingerprint)
        } catch (e: Exception) {
            errorMessage = e.message ?: "Invalid pairing code"
        } finally {
            onDeepLinkConsumed()
        }
    }

    // Surface a malformed deep-link (cppair:// with unrecognised payload) as a
    // snackbar so the user gets explicit feedback instead of nothing happening.
    LaunchedEffect(incomingDeepLinkError) {
        val errMsg = incomingDeepLinkError ?: return@LaunchedEffect
        snackbarHostState.showSnackbar(
            message = errMsg,
            actionLabel = dismissLabel,
        )
        onDeepLinkErrorConsumed()
    }

    LaunchedEffect(errorMessage) {
        val msg = errorMessage ?: return@LaunchedEffect
        snackbarHostState.showSnackbar(
            message = errorTemplate.format(msg),
            actionLabel = dismissLabel,
        )
        errorMessage = null
    }

    Scaffold(
        modifier = modifier,
        containerColor = IdeBg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_pair),
                showBackButton = showBackButton,
                onBack = onBack,
                backContentDescription = stringResource(R.string.cd_back),
            )
        },
        snackbarHost = { SnackbarHost(hostState = snackbarHostState) }
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
            Text(
                text = stringResource(R.string.pair_instructions),
                style = MaterialTheme.typography.bodyLarge,
                color = IdeText
            )

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
                                    CircularProgressIndicator(
                                        color = IdeAccent
                                    )
                                    Text(
                                        text = stringResource(R.string.status_pairing),
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = IdeDim
                                    )
                                }
                            }
                            bmp != null && !expired -> {
                                // QR needs a light, high-contrast backing to scan
                                // reliably — sit the code on a white rounded plate
                                // that fills the reserved slot exactly.
                                // First tap reveals the QR; second tap regenerates it
                                // and keeps it visible (HW-A5: no re-blur after regen).
                                Box(
                                    modifier = Modifier
                                        .size(QR_SLOT_SIZE_DP.dp)
                                        .clip(RoundedCornerShape(12.dp))
                                        // Blur applied after clip so the rounded corners
                                        // uniformly contain the blur — QR edges near the
                                        // padding are fully obscured, not just the image.
                                        .then(
                                            if (qrBlurred) Modifier.blur(16.dp)
                                            else Modifier
                                        )
                                        .clickable {
                                            if (qrBlurred) {
                                                // First tap: reveal the QR.
                                                qrBlurred = false
                                            } else {
                                                // Second tap: regenerate and stay visible.
                                                generateQr(keepVisible = true)
                                            }
                                        },
                                    contentAlignment = Alignment.Center,
                                ) {
                                    // White plate with QR image.
                                    Box(
                                        modifier = Modifier
                                            .size(QR_SLOT_SIZE_DP.dp)
                                            .background(androidx.compose.ui.graphics.Color.White)
                                            .padding(QR_PLATE_PADDING_DP.dp),
                                        contentAlignment = Alignment.Center,
                                    ) {
                                        Image(
                                            bitmap = bmp.asImageBitmap(),
                                            contentDescription = "Pairing QR code",
                                            modifier = Modifier.size(QR_IMAGE_SIZE_DP.dp)
                                        )
                                    }
                                    // Overlay hint shown only while blurred.
                                    if (qrBlurred) {
                                        Text(
                                            text = "Tap to reveal",
                                            style = MaterialTheme.typography.labelLarge,
                                            color = IdeText,
                                            textAlign = TextAlign.Center,
                                        )
                                    }
                                }
                            }
                            else -> {
                                Icon(
                                    imageVector = Icons.Filled.QrCode,
                                    contentDescription = null,
                                    tint = IdeDim,
                                    modifier = Modifier.size(96.dp)
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
                                    color = IdeDanger
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
                                    color = if (urgent) IdeDanger else IdeDim
                                )
                            }
                        }
                    }
                }
            }

            OutlinedButton(
                onClick = { startScanFlow() },
                modifier = Modifier.fillMaxWidth()
            ) {
                Text(text = stringResource(R.string.btn_scan_qr))
            }

            // ── Paired device display ─────────────────────────────────────
            // Show the persisted paired peer (fingerprint + sync address) so the
            // user can confirm which device is paired after navigating away and
            // returning. Reads directly from Settings each recomposition — the
            // values are written by runPairAndSync and are stable once set.
            val pairedFingerprint = settings.pairedPeerFingerprint
            val pairedAddr = settings.pairedPeerSyncAddr
            if (pairedFingerprint.isNotBlank()) {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceVariant
                    ),
                    border = BorderStroke(1.dp, IdeBorder),
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text(
                            text = "Paired device",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        Text(
                            text = pairedFingerprint,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.clickable {
                                clipboardManager.setText(AnnotatedString(pairedFingerprint))
                                scope.launch {
                                    snackbarHostState.showSnackbar("Fingerprint copied")
                                }
                            },
                        )
                        if (pairedAddr.isNotBlank()) {
                            Text(
                                text = pairedAddr,
                                style = MaterialTheme.typography.bodySmall,
                                color = IdeDim,
                            )
                        }
                    }
                }
            }

            scannedInfo?.let { info ->
                Text(
                    text = "Scanned: $info",
                    style = MaterialTheme.typography.bodyMedium,
                    color = IdeText,
                    modifier = Modifier.clickable {
                        val fp = scannedPeer?.fingerprint ?: info
                        clipboardManager.setText(AnnotatedString(fp))
                        scope.launch {
                            snackbarHostState.showSnackbar("Fingerprint copied")
                        }
                    },
                )
            }

            scannedPeer?.let { peer ->
                Button(
                    enabled = !syncing,
                    onClick = { runPairAndSync(peer) },
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(text = if (syncing) "Pairing…" else "Pair & sync")
                }
            }

            if (syncing) {
                CircularProgressIndicator(
                    color = IdeAccent
                )
            }

            syncResult?.let { msg ->
                Text(
                    text = msg,
                    style = MaterialTheme.typography.bodyMedium,
                    color = IdeAccent
                )
            }
        }
    }
}
