package com.copypaste.android

import android.Manifest
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material.icons.filled.QrCode
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CopyPasteTheme
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
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            CopyPasteTheme {
                PairScreen(onBack = { finish() })
            }
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PairScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
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
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val errorTemplate = stringResource(R.string.error_pairing)
    val dismissLabel = stringResource(R.string.snackbar_dismiss)

    val expired = qr != null && remainingSeconds <= 0

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
            scannedInfo = "${info.deviceName.ifBlank { "device" }} (${info.fingerprint})"
        } catch (e: Exception) {
            errorMessage = e.message ?: "Invalid pairing code"
        }
    }

    fun launchScanner() {
        val options = ScanOptions()
            .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
            .setPrompt("Scan the pairing QR on the other device")
            .setBeepEnabled(false)
            .setOrientationLocked(false)
        scanLauncher.launch(options)
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
    // NOTE (L4): the macOS side currently advertises addr_hint as 127.0.0.1:<port>
    // (loopback only), so a live emulator↔host connection will fail here until
    // macOS advertises a host-reachable address. That is a separate task.
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
                        if (repository.storeItem(plaintext, key)) stored += 1
                    }
                    // Persist the peer for future syncs.
                    settings.pairedPeerFingerprint = bootstrap.peerFingerprint
                    settings.pairedPeerSyncAddr = bootstrap.peerSyncAddr
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
    LaunchedEffect(qr) {
        if (qr == null) return@LaunchedEffect
        remainingSeconds = PAIR_TOKEN_TTL_SECONDS
        while (remainingSeconds > 0) {
            delay(1000)
            remainingSeconds -= 1
        }
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
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.title_pair)) },
                navigationIcon = {
                    if (showBackButton) {
                        IconButton(onClick = onBack) {
                            Icon(Icons.Filled.ArrowBack, contentDescription = stringResource(R.string.cd_back))
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.primary,
                    titleContentColor = MaterialTheme.colorScheme.onPrimary,
                    navigationIconContentColor = MaterialTheme.colorScheme.onPrimary,
                )
            )
        },
        snackbarHost = { SnackbarHost(hostState = snackbarHostState) }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(20.dp, Alignment.Top)
        ) {
            Text(
                text = stringResource(R.string.pair_instructions),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface
            )

            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.secondaryContainer
                )
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(28.dp),
                    contentAlignment = Alignment.Center
                ) {
                    val bmp = qrBitmap
                    when {
                        loading -> {
                            Column(
                                horizontalAlignment = Alignment.CenterHorizontally,
                                verticalArrangement = Arrangement.spacedBy(12.dp)
                            ) {
                                CircularProgressIndicator(
                                    color = MaterialTheme.colorScheme.onSecondaryContainer
                                )
                                Text(
                                    text = stringResource(R.string.status_pairing),
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSecondaryContainer
                                )
                            }
                        }
                        bmp != null && !expired -> {
                            Image(
                                bitmap = bmp.asImageBitmap(),
                                contentDescription = "Pairing QR code",
                                modifier = Modifier.size(240.dp)
                            )
                        }
                        else -> {
                            Icon(
                                imageVector = Icons.Filled.QrCode,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSecondaryContainer,
                                modifier = Modifier.size(96.dp)
                            )
                        }
                    }
                }
            }

            Button(
                enabled = !loading,
                onClick = {
                    scope.launch {
                        loading = true
                        try {
                            val result = withContext(Dispatchers.IO) {
                                startPairing(settings.deviceId, android.os.Build.MODEL ?: "Android")
                            }
                            val bmp = withContext(Dispatchers.Default) {
                                encodeQrBitmap(result.qr, 512)
                            }
                            qr = result
                            qrBitmap = bmp
                        } catch (e: Exception) {
                            errorMessage = e.message ?: e.javaClass.simpleName
                        } finally {
                            loading = false
                        }
                    }
                },
                modifier = Modifier.fillMaxWidth()
            ) {
                Text(
                    text = stringResource(
                        when {
                            qr == null -> R.string.btn_pair_start
                            expired -> R.string.pair_request_new_token
                            else -> R.string.btn_pair_regenerate
                        }
                    )
                )
            }

            OutlinedButton(
                onClick = { startScanFlow() },
                modifier = Modifier.fillMaxWidth()
            ) {
                Text(text = "Scan a device's QR")
            }

            scannedInfo?.let { info ->
                Text(
                    text = "Scanned: $info",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface
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
                    color = MaterialTheme.colorScheme.primary
                )
            }

            syncResult?.let { msg ->
                Text(
                    text = msg,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.primary
                )
            }

            if (qr != null) {
                when {
                    expired -> {
                        Text(
                            text = stringResource(R.string.pair_token_expired),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.error
                        )
                    }
                    else -> {
                        val urgent = remainingSeconds <= PAIR_TOKEN_URGENT_THRESHOLD_SECONDS
                        Text(
                            text = stringResource(
                                R.string.pair_token_expires_in_seconds,
                                remainingSeconds
                            ),
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (urgent) {
                                MaterialTheme.colorScheme.error
                            } else {
                                MaterialTheme.colorScheme.onSurfaceVariant
                            }
                        )
                        Text(
                            text = stringResource(R.string.pair_token_note),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }
        }
    }
}
