package com.copypaste.android

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.repeatOnLifecycle
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.CpSpacing
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

// CopyPaste-vp63.38: PairScreen is now Scaffold/dialog orchestration ONLY.
// The pairing state machine (QR gen/TTL refresh, scan/bootstrap/finalize) lives
// in [PairController] (+ PairBootstrapSync.kt); the QR card and peer-identity
// cards live in PairQrCard.kt / PairedPeerList.kt.

// CopyPaste-jkbo: encodeQrBitmap was a private duplicate of the same function in
// DevicesActivity. Both are now replaced by the package-level [encodeQrBitmap] in
// QrUtils.kt — call sites reference it directly (same package) via [PairingApi].
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
    val scope = rememberCoroutineScope()
    val controller = remember { PairController(context, settings, deviceKeyStore, repository, scope) }
    val toastState = remember { GlassToastState() }
    val clipboardManager = LocalClipboardManager.current
    val fingerprintCopiedMsg = stringResource(R.string.pair_fingerprint_copied)
    // CopyPaste-jwga: errorMessage now holds a pre-sanitized, user-friendly string
    // from ErrorMessages.friendly*(). No wrapper template is applied at display time
    // so raw exception text, paths, and FFI symbols never reach the user.

    fun copyFingerprint(fingerprint: String) {
        clipboardManager.setText(AnnotatedString(fingerprint))
        scope.launch {
            toastState.show(fingerprintCopiedMsg, GlassToastKind.ACCENT)
        }
    }

    // Camera scanner (ZXing). On a successful scan, parse the payload natively.
    val scanLauncher = rememberLauncherForActivityResult(ScanContract()) { result ->
        val contents = result.contents
            ?: return@rememberLauncherForActivityResult // user cancelled
        controller.handleScanResult(contents)
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
            controller.errorMessage = ErrorMessages.friendlyCameraError(e)
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
                controller.errorMessage = context.getString(R.string.error_camera_permission_permanent)
            } else {
                // CopyPaste-jwga: use string resource for user-facing message.
                controller.errorMessage = context.getString(R.string.error_camera_permission_denied)
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
            controller.errorMessage = context.getString(R.string.error_camera_permission_permanent)
            return
        }
        NotificationPermissionHelper.markCameraRequested(context)
        cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
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
    LaunchedEffect(controller.qr) {
        if (controller.qr == null) return@LaunchedEffect
        lifecycleOwner.lifecycle.repeatOnLifecycle(Lifecycle.State.RESUMED) {
            controller.remainingSeconds = PAIR_TOKEN_TTL_SECONDS
            // CopyPaste-crh3.33: regenerate with a QR_REFRESH_MARGIN_SECONDS (15s)
            // margin BEFORE the token actually expires (parity with macOS), so a
            // slow scan never reads an already-expired code.
            while (controller.remainingSeconds > QR_REFRESH_MARGIN_SECONDS) {
                delay(1000)
                controller.remainingSeconds -= 1
            }
            // Near expiry — auto-regenerate only when the success popup is not up.
            if (controller.pairedPeerForPopup == null) controller.generateQr()
        }
    }

    // AND2: Auto-start pairing when the screen opens so the QR appears
    // immediately without requiring the user to tap "Start Pairing".
    LaunchedEffect(Unit) {
        if (controller.qr != null || controller.loading) return@LaunchedEffect
        controller.generateQr(resetReveal = true)
    }

    // Consume an incoming cppair:// deep-link payload from an external QR scanner
    // (e.g. Google Lens).  The payload is the raw CPPAIR1/CPPAIR2.… string,
    // identical to what the in-app ZXing scanner would return — feed it through
    // the same parsePairing path and surface the same confirmation UI.
    LaunchedEffect(incomingDeepLinkPayload) {
        val payload = incomingDeepLinkPayload ?: return@LaunchedEffect
        controller.handleDeepLinkPayload(payload)
        onDeepLinkConsumed()
    }

    // Surface a malformed deep-link (cppair:// with unrecognised payload) as a
    // toast so the user gets explicit feedback instead of nothing happening.
    LaunchedEffect(incomingDeepLinkError) {
        val errMsg = incomingDeepLinkError ?: return@LaunchedEffect
        toastState.show(errMsg, GlassToastKind.DANGER)
        onDeepLinkErrorConsumed()
    }

    LaunchedEffect(controller.errorMessage) {
        val msg = controller.errorMessage ?: return@LaunchedEffect
        // CopyPaste-jwga: msg is already a sanitized, user-friendly string —
        // show it directly without a raw-exception-embedding format template.
        toastState.show(msg, GlassToastKind.DANGER)
        controller.errorMessage = null
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

    // CopyPaste-wfba: progress-bar pulse removed — static progress bar is calmer.
    // Entrance alpha fade removed — card appears instantly (no idle animation).

    Box(Modifier.fillMaxSize()) {
    Scaffold(
        modifier = modifier,
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
            verticalArrangement = Arrangement.spacedBy(CpSpacing.s8, Alignment.Top)
        ) {
            // ── Deliverable 2: hide own-QR once a peer has been scanned ───────
            // When scannedPeer is non-null we are in "confirm peer" mode. The own-QR
            // card, instructions text, and Scan button belong only to the "show my QR"
            // mode and are hidden so the screen focuses on the scanned-peer confirmation.
            if (controller.scannedPeer == null) {
                Text(
                    text = stringResource(R.string.pair_instructions),
                    style = MaterialTheme.typography.bodyLarge,
                )

                PairQrCard(
                    loading = controller.loading,
                    qrBitmap = controller.qrBitmap,
                    hasQr = controller.qr != null,
                    expired = controller.expired,
                    qrRevealed = controller.qrRevealed,
                    remainingSeconds = controller.remainingSeconds,
                    onTap = {
                        if (!controller.qrRevealed) {
                            controller.qrRevealed = true
                        } else {
                            controller.generateQr()
                        }
                    },
                )

                // ── Deliverable 2: Scan button — only shown in own-QR mode ──────
                // Hidden when a peer has already been scanned (scannedPeer != null);
                // in that state the screen shows only the peer confirmation UI below.
                // Adopt CopyPasteButton secondary (glass) per action-button spec.
                CopyPasteButton(
                    onClick = { startScanFlow() },
                    variant = ButtonVariant.SECONDARY,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(text = stringResource(R.string.btn_scan_qr))
                }
            }

            // ── Deliverable 3: rich scanned-peer confirmation card ────────────
            // Shown INSTEAD of the own-QR once a peer has been scanned.
            controller.scannedPeer?.let { peer ->
                ScannedPeerReviewCard(
                    peer = peer,
                    pendingBootstrap = controller.pendingBootstrap,
                    syncing = controller.syncing,
                    onVerify = { controller.runBootstrap(peer) },
                    onConfirmSync = { bs -> controller.finalizeSync(peer, bs) },
                    onCancel = { controller.cancelReview() },
                    onCopyFingerprint = ::copyFingerprint,
                )
            }

            if (controller.syncing) {
                CircularProgressIndicator()
            }

            // ── Post-pair success popup ────────────────────────────────────────
            // Shown as a compact AlertDialog overlay once pairing completes.
            // The full syncResult string is still set (for logging / snackbar
            // fallback) but no longer displayed inline — the popup card takes over.
            controller.pairedPeerForPopup?.let { justPaired ->
                PairedSuccessPopup(
                    peer = justPaired,
                    onDismiss = {
                        controller.pairedPeerForPopup = null
                        onBack()
                    },
                )
            }

            // ── Paired-device roster (own-QR mode only) ───────────────────────
            // Show the persisted paired peer so the user can confirm which device
            // is paired. Only shown when not in the scanned-peer confirmation flow.
            if (controller.scannedPeer == null && controller.syncResult == null) {
                val pairedFingerprint = settings.pairedPeerFingerprint
                val pairedAddr = settings.pairedPeerSyncAddr
                if (pairedFingerprint.isNotBlank()) {
                    PairedDeviceSummaryCard(
                        fingerprint = pairedFingerprint,
                        syncAddr = pairedAddr,
                        onCopyFingerprint = ::copyFingerprint,
                    )
                }
            }
        }
    }
    GlassToastHost(state = toastState)
    } // end Box
}
