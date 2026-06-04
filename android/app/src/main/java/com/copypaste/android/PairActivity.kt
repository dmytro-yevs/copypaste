package com.copypaste.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Color
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
// syncWithPeer is the package-local wrapper in CopypasteBindings.kt (ABI-9
// signature: revokedFingerprints + deviceId, ByteArray sessionKey). It shadows
// the generated uniffi.copypaste_android.syncWithPeer intentionally.

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
                    settings.upsertPeer(
                        PairedPeer(
                            fingerprint = bootstrap.peerFingerprint,
                            syncAddr = bootstrap.peerSyncAddr,
                            name = peer.deviceName,
                            sessionKeyWrappedB64 = wrappedB64,
                            sessionKeyIvB64 = ivB64,
                            lastSyncMs = System.currentTimeMillis(),
                            // HB-1b (ABI 14): persist the peer's device metadata
                            // received over the authenticated tunnel so Wave 3
                            // renders the device card.
                            peerModel = bootstrap.peerModel,
                            peerOs = bootstrap.peerOs,
                            peerAppVersion = bootstrap.peerAppVersion,
                            peerLocalIp = bootstrap.peerLocalIp,
                            peerPublicIp = bootstrap.peerPublicIp,
                        )
                    )
                    val peerCount = settings.pairedPeers.size
                    // HB-7a (ABI 14): surface the per-reason drop counters so a
                    // "received N stored 0" outcome reveals WHY items dropped.
                    val skipped = "skipped: legacy ${result.itemsSkippedLegacy} / " +
                        "decrypt ${result.itemsSkippedDecryptFail} / " +
                        "type ${result.itemsSkippedUnknownType} / " +
                        "blob ${result.itemsSkippedMissingBlob}"
                    "Paired with ${peer.deviceName.ifBlank { "device" }} — received ${result.itemsReceived} item(s), stored $stored ($skipped), sent ${result.itemsSent}. ($peerCount paired device(s))"
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

    // HB-6: auto-open the scanner when launched in scan mode (from the Devices
    // screen "Scan a device's QR" button). Consumed once so a recomposition does
    // not re-launch it; the QR-display flow above still runs in parallel so this
    // device's own QR is ready behind the scanner.
    LaunchedEffect(autoScan) {
        if (!autoScan) return@LaunchedEffect
        startScanFlow()
        onAutoScanConsumed()
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
            // ── Deliverable 2: hide own-QR once a peer has been scanned ───────
            // When scannedPeer is non-null we are in "confirm peer" mode. The own-QR
            // card, instructions text, and Scan button belong only to the "show my QR"
            // mode and are hidden so the screen focuses on the scanned-peer confirmation.
            if (scannedPeer == null) {
                Text(
                    text = stringResource(R.string.pair_instructions),
                    style = MaterialTheme.typography.bodyLarge,
                    color = IdeText
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

            // ── Deliverable 2: Scan button — only shown in own-QR mode ──────
            // Hidden when a peer has already been scanned (scannedPeer != null);
            // in that state the screen shows only the peer confirmation UI below.
            if (scannedPeer == null) {
                OutlinedButton(
                    onClick = { startScanFlow() },
                    modifier = Modifier.fillMaxWidth()
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
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
                    border = BorderStroke(1.dp, IdeBorder),
                ) {
                    Column(
                        modifier = Modifier.padding(16.dp),
                        verticalArrangement = androidx.compose.foundation.layout.Arrangement.spacedBy(8.dp),
                    ) {
                        Text(
                            text = "Device to pair with",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        // Device name (from QR payload field 5)
                        val displayName = peer.deviceName.ifBlank { "Unknown device" }
                        Text(
                            text = displayName,
                            style = MaterialTheme.typography.titleSmall,
                            color = IdeText,
                        )
                        // Address (host:port from QR payload field 6, if present)
                        if (peer.addrHint.isNotBlank()) {
                            Text(
                                text = "Address: ${peer.addrHint}",
                                style = MaterialTheme.typography.bodySmall,
                                color = IdeDim,
                            )
                        }
                        // Fingerprint (from QR payload field 2) — tappable to copy
                        Text(
                            text = "Fingerprint: ${peer.fingerprint}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.clickable {
                                clipboardManager.setText(AnnotatedString(peer.fingerprint))
                                scope.launch {
                                    snackbarHostState.showSnackbar("Fingerprint copied")
                                }
                            },
                        )
                        // NOTE: model/OS/appVersion become available after the PAKE
                        // bootstrap completes — see TODO above.
                    }
                }

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

            // ── Post-pair success message ──────────────────────────────────────
            syncResult?.let { msg ->
                Text(
                    text = msg,
                    style = MaterialTheme.typography.bodyMedium,
                    color = IdeAccent
                )
            }

            // ── Paired-device roster (own-QR mode only) ───────────────────────
            // Show the persisted paired peer so the user can confirm which device
            // is paired. Only shown when not in the scanned-peer confirmation flow.
            if (scannedPeer == null && syncResult == null) {
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
            }
        }
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
