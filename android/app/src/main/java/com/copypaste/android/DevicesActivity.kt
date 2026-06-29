package com.copypaste.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
// TextButton removed — replaced by CopyPasteButton (CopyPaste-bdac.8)
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.SectionLabel
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.screenCanvas
import com.copypaste.android.ui.theme.rememberTranslucency
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// ─────────────────────────────────────────────────────────────────────────────
// Activity
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Devices screen — shows the full roster of paired P2P peers, each as a card
 * with a real-presence online dot, model, OS, version, IP fields, last-sync time,
 * and per-peer Unpair / Revoke actions. Parity with the macOS DevicesView.
 *
 * Navigation: launched from the DEVICES tab in [MainActivity] bottom nav, and
 * also accessible as a standalone activity from [SettingsActivity] (General tab
 * "Devices" row).
 */
class DevicesActivity : ComponentActivity() {

    companion object {
        /**
         * Boolean Intent extra: when true, [DevicesScreen] auto-opens the SAS modal on
         * resume if [pairGetSas] returns `awaiting_sas`. Set by
         * [ClipboardService.postIncomingPairNotification] so tapping the pairing-request
         * notification takes the user directly to the SAS confirm dialog.
         */
        const val EXTRA_AUTO_OPEN_SAS = "auto_open_sas"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // CopyPaste-1g00: screenshot protection is now pref-driven (Settings.allowScreenshots).
        // CopyPasteTheme applies FLAG_SECURE centrally when allowScreenshots=false (the default).
        applyScreenshotPolicy(Settings(this))
        enableEdgeToEdge()
        val autoOpenSas = intent?.getBooleanExtra(EXTRA_AUTO_OPEN_SAS, false) ?: false
        setContent {
            CopyPasteTheme {
                DevicesScreen(
                    showBackButton = true,
                    onBack = { finish() },
                    autoOpenSasOnEntry = autoOpenSas,
                )
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Composable screen (also embedded in MainShell's DEVICES tab)
// ─────────────────────────────────────────────────────────────────────────────

@Composable
fun DevicesScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
    /**
     * When true (set by tapping the incoming-pair notification), the screen
     * immediately polls [pairGetSas] once on composition and auto-opens the SAS
     * modal if the state is `awaiting_sas`. Consumed after the first check.
     */
    autoOpenSasOnEntry: Boolean = false,
    /** §1: paint the canvas backdrop here (standalone) vs. via MainShell (embedded). */
    paintCanvasBackdrop: Boolean = true,
) {
    val ctx = LocalContext.current
    val c = LocalIdeColors.current
    val settings = remember { Settings(ctx) }
    val deviceKeyStore = remember { DeviceKeyStore(ctx) }
    val scope = rememberCoroutineScope()
    // Calm screen backdrop (glass surfaces frost over real colour).
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()

    // ── Direct camera scan launcher (Deliverable 2) ───────────────────────────
    // The scan button on this screen launches the ZXing scanner directly —
    // no PairActivity intermediary. The scan result (a CPPAIR1.… payload) is
    // forwarded to PairActivity as a cppair:// deep-link so the full pair &
    // sync flow (PAKE bootstrap, key persistence, provisioning apply) still
    // runs there unmodified.
    var scanError by remember { mutableStateOf<String?>(null) }

    val scanLauncher = rememberLauncherForActivityResult(ScanContract()) { result ->
        val contents = result.contents ?: return@rememberLauncherForActivityResult
        // Forward the raw CPPAIR1.… payload to PairActivity via the deep-link
        // path so PAKE + provisioning logic runs there.
        val intent = Intent(ctx, PairActivity::class.java).apply {
            action = Intent.ACTION_VIEW
            data = android.net.Uri.parse("cppair://pair?p=${android.net.Uri.encode(contents)}")
        }
        ctx.startActivity(intent)
    }

    fun launchScanner() {
        val opts = ScanOptions()
            .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
            .setPrompt("Scan the pairing QR on the other device")
            .setBeepEnabled(false)
            .setOrientationLocked(true)
            .setCaptureActivity(PortraitCaptureActivity::class.java)
        try {
            scanLauncher.launch(opts)
        } catch (e: Exception) {
            // CopyPaste-jwga: never surface raw exception detail to users.
            scanError = ErrorMessages.friendlyCameraError(e)
        }
    }

    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            launchScanner()
        } else {
            // CopyPaste-jwga: use sanitized, user-friendly permission message.
            scanError = ctx.getString(R.string.error_camera_permission_denied)
        }
    }

    fun startScanFlow() {
        val hasCamera = ContextCompat.checkSelfPermission(
            ctx, Manifest.permission.CAMERA
        ) == PackageManager.PERMISSION_GRANTED
        if (hasCamera) launchScanner() else cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
    }

    // Refresh the roster every poll interval so the online dots and last-sync
    // labels update as FgsSyncLoop stamps presence.
    var peers by remember { mutableStateOf(settings.pairedPeers) }
    var ownIdentity by remember { mutableStateOf(settings.p2pIdentity) }

    // CopyPaste-6qq1: own public IP from a one-shot STUN query (StunUtils.queryPublicIp).
    // Null until the coroutine resolves or when collectPublicIp is disabled.
    var ownPublicIp by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) {
        ownPublicIp = withContext(Dispatchers.IO) {
            StunUtils.queryPublicIp(settings.collectPublicIp)
        }
    }

    // ── 1-second clock tick ───────────────────────────────────────────────────
    // Drives smooth "Xm ago" / "Xs ago" updates and the online dot recomputation
    // without a separate per-card timer. Also used to re-read the local IP on a
    // coarser cadence (every ~5 s) so a Wi-Fi handoff is reflected promptly.
    var nowMs by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(1_000L)
            nowMs = System.currentTimeMillis()
        }
    }

    // ── LAN discovery + SAS pairing state ─────────────────────────────────────
    // P2P must be enabled for discovery (parity with the daemon gating discovery
    // behind start_p2p). When disabled we neither advertise nor browse.
    val p2pEnabled = remember { settings.p2pSyncEnabled }
    // Non-paired, SAS-capable peers discovered on the LAN (refreshed by the poll
    // effect below). Paired peers are filtered out natively via `paired`.
    var discovered by remember { mutableStateOf<List<DiscoveredPeer>>(emptyList()) }
    // The peer a SAS pairing modal is currently open for, or null. Setting it
    // non-null opens the modal (which begins polling pair_get_sas).
    var pairingPeer by remember { mutableStateOf<DiscoveredPeer?>(null) }
    // True while pair_with_discovered is in flight (before the modal opens).
    var pairStarting by remember { mutableStateOf(false) }
    // Inline error shown beneath the discovered list (e.g. another pairing busy).
    var discoverError by remember { mutableStateOf<String?>(null) }

    fun refresh() {
        peers = settings.pairedPeers
        ownIdentity = settings.p2pIdentity
    }

    LaunchedEffect(Unit) {
        while (true) {
            delay(PEER_POLL_MS)
            refresh()
        }
    }

    // ── SINGLE SOURCE OF TRUTH: online map keyed by fingerprint ───────────────
    //
    // A paired peer is ONLINE iff:
    //   (a) its IP host (from syncAddr or peerLocalIp) appears in the live mDNS
    //       `discovered` set (IP-correlation — mDNS device_id is a UUID, NOT a
    //       cert fingerprint, so we match on IP only), OR
    //   (b) its lastSyncMs falls within ONLINE_WINDOW_MS of nowMs (fallback for
    //       peers not currently advertising on mDNS, e.g. were online recently).
    //
    // Computed ONCE here and threaded to every site that shows an online
    // indicator: PeerCard dot AND the footer count via [DevicesOnlineState].
    // Removes the prior divergence where the footer counted configured targets
    // while each card independently called peer.isOnline().
    val discoveredIps: Set<String> = remember(discovered) {
        discovered.flatMap { it.ipAddrs }.toHashSet()
    }
    val onlineByFingerprint: Map<String, Boolean> = remember(peers, discoveredIps, nowMs) {
        peers.associate { peer ->
            val peerIpHosts = listOfNotNull(
                // host part of "host:port" (substringBeforeLast tolerates bare host).
                peer.syncAddr.takeIf { it.isNotEmpty() }?.substringBeforeLast(':'),
                peer.peerLocalIp?.takeIf { it.isNotEmpty() },
            )
            val viaMdns = peerIpHosts.any { host -> discoveredIps.contains(host) }
            // CopyPaste-d6z3: use isPeerOnline (recentSync OR mDNS-discovered) instead of
            // the old peer.isOnline() which only checked the 60 s ONLINE_WINDOW_MS gate.
            // isPeerOnline uses RECENT_SYNC_MS (5 min) matching macOS parity.
            val online = isPeerOnline(
                lastSyncMs = peer.lastSyncMs,
                isMdnsDiscovered = viaMdns,
                nowMs = nowMs,
                onlineWindowMs = ONLINE_WINDOW_MS,
                recentSyncMs = RECENT_SYNC_MS,
            )
            peer.fingerprint to online
        }
    }

    // ── Deliverable 1: auto-open SAS modal on screen entry ────────────────────
    // Triggered when: (a) user tapped the incoming-pair notification
    // (autoOpenSasOnEntry=true), OR (b) general entry — poll once to catch
    // awaiting_sas for EITHER role so the modal appears regardless of who
    // initiated. Uses a sentinel DiscoveredPeer with the state machine's peer
    // info; if the native library is absent this is a safe no-op.
    LaunchedEffect(Unit) {
        if (!isNativeLibraryLoaded) return@LaunchedEffect
        // Give mDNS a moment to start on first composition before probing.
        if (!autoOpenSasOnEntry) delay(800L)
        try {
            val st = withContext(Dispatchers.IO) { pairGetSas() }
            if (st.state == "awaiting_sas" && pairingPeer == null) {
                // Build a sentinel DiscoveredPeer so SasPairingDialog can open.
                // deviceId/deviceName are best-effort; the dialog only uses them
                // for the title and (for responder) skips pairWithDiscovered.
                pairingPeer = DiscoveredPeer(
                    deviceId = st.peerFingerprint ?: "unknown",
                    deviceName = "",   // unknown at this stage for responder role
                    ipAddrs = emptyList(),
                    port = 0u,
                    bport = null,
                    paired = false,
                )
            }
        } catch (_: Exception) {
            // pairGetSas not yet available — safe to ignore on first composition.
        }
    }


    // Publish live count + most-recent peer activity so SyncStatusBadge (footer)
    // reads the SAME values as the peer cards — single source, zero divergence.
    // maxLastSyncMs drives the PG-11 RECENT_SYNC_MS recency gate in the badge.
    val maxLastSyncMs = remember(peers) { peers.maxOfOrNull { it.lastSyncMs } ?: 0L }
    DevicesOnlineState.publish(
        count = onlineByFingerprint.count { it.value },
        maxLastSyncMs = maxLastSyncMs,
    )

    // ── mDNS discovery lifecycle lives in ClipboardService (HB-2) ─────────────
    // Discovery (the mDNS advert + the standing SAS-pairing responder on
    // [SAS_BPORT]) is started/stopped by the always-on [ClipboardService] FGS,
    // NOT here. Hosting it on this screen meant the responder died the moment the
    // Devices screen closed, so a Mac→Android pair got "Connection refused". The
    // FGS keeps it alive for the lifetime of the service; this screen only
    // browses the resulting peer snapshot below.

    // ── Poll the discovered peer list every ~2 s ──────────────────────────────
    // HB-4: listDiscovered marks `paired` by IP-correlation now (the mDNS
    // device_id is a UUID, not a cert fingerprint, so the old fingerprint-compare
    // never matched). We pass the set of IP hosts we have paired with — each
    // peer's syncAddr host plus its peerLocalIp — and drop the matched entries.
    LaunchedEffect(p2pEnabled) {
        if (!p2pEnabled) {
            discovered = emptyList()
            return@LaunchedEffect
        }
        while (true) {
            try {
                val pairedIps = settings.pairedPeers.flatMap { peer ->
                    listOfNotNull(
                        // host part of "host:port" (substringBeforeLast tolerates a
                        // bare host with no port).
                        peer.syncAddr.takeIf { it.isNotEmpty() }?.substringBeforeLast(':'),
                        peer.peerLocalIp?.takeIf { it.isNotEmpty() },
                    )
                }.distinct()
                val list = withContext(Dispatchers.IO) { listDiscovered(pairedIps) }
                discovered = list.filterNot { it.paired }
            } catch (e: Exception) {
                // Discovery is best-effort — keep the previous snapshot, log only.
                Log.w(TAG, "listDiscovered failed: ${e.message}")
            }
            delay(DISCOVERED_POLL_MS)
        }
    }

    // Begin a discovery-initiated SAS pairing as initiator, then open the modal.
    fun startPairing(peer: DiscoveredPeer) {
        if (pairStarting || pairingPeer != null) return
        discoverError = null
        pairStarting = true
        scope.launch {
            try {
                // CopyPaste-44rq.55: getOrCreate() zeroes cert.keyDer before returning;
                // peek() re-fetches the KEK-unwrapped identity from AndroidKeyStore.
                val cert = withContext(Dispatchers.IO) {
                    deviceKeyStore.peek() ?: deviceKeyStore.getOrCreate().let { deviceKeyStore.peek()!! }
                }
                withContext(Dispatchers.IO) {
                    pairWithDiscovered(
                        deviceId = peer.deviceId,
                        certDer = cert.certDer,
                        keyDer = cert.keyDer,
                        // The peer (a configured Mac) provides provisioning; the
                        // phone advertises no sync address / carries no config.
                        syncAddr = "",
                        localProvisioning = null,
                        // HB-1a (ABI 14): advertise this device's own metadata.
                        deviceName = android.os.Build.MODEL ?: "Android",
                        deviceModel = android.os.Build.MODEL ?: "Android",
                        osVersion = "Android " + android.os.Build.VERSION.RELEASE,
                        appVersion = BuildConfig.VERSION_NAME,
                        localIp = lanIpv4Address(),
                        // ABI 18 (PG-28): STUN-derived WAN address collected at
                        // screen entry (LaunchedEffect above). Null when
                        // collectPublicIp is disabled or STUN failed.
                        publicIp = ownPublicIp,
                    )
                }
                pairingPeer = peer
            } catch (e: Exception) {
                Log.w(TAG, "pairWithDiscovered failed: ${e.message}", e)
                // CopyPaste-jwga: never surface raw exception detail to users.
                discoverError = ErrorMessages.friendlyPairingError(e)
                // HB-8: pairWithDiscovered may have claimed the native SM (via
                // try_begin) before failing — reset defensively so a retry is not
                // refused with "a pairing is already in flight".
                try {
                    withContext(Dispatchers.IO) { pairReset() }
                } catch (re: Exception) {
                    Log.w(TAG, "pairReset after failed start failed: ${re.message}")
                }
            } finally {
                pairStarting = false
            }
        }
    }

    // Per-peer dialog targets (null = no dialog showing).
    var unpairTarget by remember { mutableStateOf<PairedPeer?>(null) }
    var revokeTarget by remember { mutableStateOf<PairedPeer?>(null) }
    // Non-null when an async revokeDeviceAudit IO call failed — surfaced to the user.
    var revokeError by remember { mutableStateOf<String?>(null) }
    // CopyPaste-8qcm: Revoke+rotate state — non-null when the passphrase dialog is open.
    // Holds the peer selected for revoke+rotate; [revokePassphrase] is the current input.
    var revokeRotateTarget by remember { mutableStateOf<PairedPeer?>(null) }
    var revokePassphrase by remember { mutableStateOf("") }
    // True while the revokeDeviceAndRotateKey FFI call is in-flight.
    var revokeRotateInFlight by remember { mutableStateOf(false) }

    // CopyPaste-crh3.34: "Revoke all" state — mirrors macOS revokeAllConfirm/revokeAllPending.
    var revokeAllConfirmOpen by remember { mutableStateOf(false) }
    var revokeAllInFlight by remember { mutableStateOf(false) }

    // ── Unpair confirmation ──────────────────────────────────────────────────
    unpairTarget?.let { target ->
        // §8 glass dialog (audit #10) — appearance only; unpair logic unchanged.
        GlassAlertDialog(
            onDismissRequest = { unpairTarget = null },
            // CopyPaste-bdac.51: standardized to "Unpair" — was "Forget" (terminology conflict).
            title = { Text("Unpair device?") },
            text = {
                Text(
                    "This device will no longer sync with ${target.displayName()} over P2P. " +
                    "You can re-pair at any time by scanning a new QR code."
                )
            },
            confirmButton = {
                CopyPasteButton(onClick = {
                    unpairTarget = null
                    unpairPeer(settings, target.fingerprint)
                    refresh()
                }, variant = ButtonVariant.DANGER) { Text("Unpair") }
            },
            dismissButton = {
                CopyPasteButton(onClick = { unpairTarget = null }, variant = ButtonVariant.GHOST) { Text("Cancel") }
            },
        )
    }

    // ── Revoke confirmation (CopyPaste-8qcm: two-path dialog) ─────────────────
    // First dialog: presents the user with two revoke options:
    //   • "Revoke only"        → plain audit + roster removal (RevokeMode.AUDIT_ONLY).
    //   • "Revoke & rotate key" → opens the passphrase dialog (RevokeMode.REVOKE_AND_ROTATE).
    //
    // The "Revoke only" path preserves the atomic CopyPaste-94o4 ordering:
    //   revokeDeviceAudit (IO) → removePeer only if audit succeeded.
    //
    // The "Revoke & rotate key" path defers to [revokeRotateTarget] passphrase dialog below.
    revokeTarget?.let { target ->
        GlassAlertDialog(
            onDismissRequest = { revokeTarget = null },
            title = { Text("Revoke pairing?") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        "${target.displayName()} will no longer connect over P2P and a " +
                        "revocation record is kept.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.text,
                    )
                    Text(
                        "A revoked device that still knows the sync passphrase can " +
                        "keep reading new relay and cloud items. To close that gap, " +
                        "choose “Revoke & rotate key” below.",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.dim,
                    )
                }
            },
            // "Revoke & rotate key" is the primary action (right-side confirm button).
            // Tapping it closes this dialog and opens the passphrase dialog.
            confirmButton = {
                CopyPasteButton(onClick = {
                    val t = revokeTarget
                    revokeTarget = null
                    if (t != null) {
                        revokePassphrase = ""
                        revokeRotateTarget = t
                    }
                }, variant = ButtonVariant.DANGER) {
                    Text("Revoke & rotate key")
                }
            },
            dismissButton = {
                Row(horizontalArrangement = Arrangement.spacedBy(0.dp)) {
                    // "Revoke only" — left; performs the plain audit+remove path.
                    CopyPasteButton(onClick = {
                        val t = revokeTarget ?: return@CopyPasteButton
                        revokeTarget = null
                        // CopyPaste-94o4: atomic revoke — write the audit record FIRST
                        // on the IO dispatcher; only remove the peer from the local
                        // roster once the DB write succeeds. A mid-write crash or DB
                        // error no longer leaves asymmetric state (peer gone locally
                        // but no audit record). On failure the peer is untouched and
                        // an error dialog is shown so the user can retry.
                        scope.launch {
                            val ok = withContext(Dispatchers.IO) {
                                runCatching {
                                    revokeDeviceAudit(
                                        dbPath = settings.dbPath,
                                        key = settings.encryptionKey,
                                        fingerprint = t.fingerprint,
                                        name = t.displayName(),
                                    )
                                }
                            }.fold(
                                onSuccess = { true },
                                onFailure = { e ->
                                    Log.e(
                                        TAG,
                                        "revokeDeviceAudit failed for ${t.fingerprint.take(8)}: ${e.message}",
                                        e,
                                    )
                                    false
                                },
                            )
                            if (ok) {
                                settings.removePeer(t.fingerprint)
                                // CopyPaste-1jms.8: log the missing peer-signal limitation
                                // (same constraint as unpairPeer — no durable pending-unpair queue).
                                Log.w(
                                    TAG,
                                    "revokeOnly: peer ${t.fingerprint.take(16)}… removed locally. " +
                                        "No unpair signal sent to peer — Android lacks a durable " +
                                        "pending-unpair queue (see CopyPaste-1jms.8).",
                                )
                                refresh()
                            } else {
                                revokeError = "Failed to record revocation. The device was NOT removed — please try again."
                            }
                        }
                    }, variant = ButtonVariant.DANGER) { Text("Revoke only") }

                    CopyPasteButton(onClick = { revokeTarget = null }, variant = ButtonVariant.GHOST) { Text("Cancel") }
                }
            },
        )
    }

    // ── Revoke + rotate key passphrase dialog (CopyPaste-8qcm) ─────────────────
    // Shown after the user selects "Revoke & rotate key" above. The user enters
    // the new passphrase (min 8 chars); "Confirm" calls revokeDeviceAndRotateKey.
    //
    // Security ordering (mirrors macOS revoke_and_rotate semantics):
    //   1. revokeDeviceAndRotateKey derives the new key from [newPassphrase] via
    //      Argon2id BEFORE any DB write — a bad passphrase leaves state unchanged.
    //   2. On success: the new sync key is persisted in Settings, the peer is
    //      removed from the roster, and updateP2pListenerPeers is called with the
    //      revoked fingerprint in the denylist.
    //   3. On failure: the peer is untouched (same CopyPaste-94o4 guarantee).
    //
    // The returned new key bytes are NEVER logged (SECURITY: secret material).
    revokeRotateTarget?.let { target ->
        GlassAlertDialog(
            onDismissRequest = {
                if (!revokeRotateInFlight) {
                    revokeRotateTarget = null
                    revokePassphrase = ""
                }
            },
            title = { Text("Set new sync passphrase") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        "Enter a new passphrase to rotate the sync key. All trusted " +
                        "devices will need to re-enter this passphrase to keep syncing.",
                        style = MaterialTheme.typography.bodySmall,
                        color = c.dim,
                    )
                    // Passphrase text field — skin-aware surface colors, password masking.
                    OutlinedTextField(
                        value = revokePassphrase,
                        onValueChange = { revokePassphrase = it },
                        label = { Text("New passphrase (min 8 chars)") },
                        visualTransformation = PasswordVisualTransformation(),
                        singleLine = true,
                        enabled = !revokeRotateInFlight,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    if (!isValidRotatePassphrase(revokePassphrase) && revokePassphrase.isNotEmpty()) {
                        Text(
                            "Passphrase must be at least 8 characters.",
                            style = MaterialTheme.typography.labelSmall,
                            color = c.danger,
                        )
                    }
                }
            },
            confirmButton = {
                CopyPasteButton(
                    enabled = isValidRotatePassphrase(revokePassphrase) && !revokeRotateInFlight,
                    onClick = {
                        val t = revokeRotateTarget ?: return@CopyPasteButton
                        val passphrase = revokePassphrase
                        if (!isValidRotatePassphrase(passphrase)) return@CopyPasteButton
                        revokeRotateInFlight = true
                        scope.launch {
                            val result = withContext(Dispatchers.IO) {
                                runCatching {
                                    // revokeDeviceAndRotateKey: derives new key FIRST
                                    // (bad passphrase → DecryptionFailed, no DB write),
                                    // then writes the audit record + removes the peer row.
                                    // Returns the new 32-byte raw sync key.
                                    val newKey = revokeDeviceAndRotateKey(
                                        dbPath = settings.dbPath,
                                        key = settings.encryptionKey,
                                        fingerprint = t.fingerprint,
                                        name = t.displayName(),
                                        newPassphrase = passphrase,
                                    )
                                    newKey
                                }
                            }
                            revokeRotateInFlight = false
                            result.fold(
                                onSuccess = { newKeyBytes ->
                                    // Persist the new passphrase so the next sync re-derives
                                    // the key identically. NEVER log the passphrase or bytes.
                                    settings.cloudSyncPassphrase = passphrase
                                    newKeyBytes.fill(0) // zero raw key bytes after persisting
                                    // Remove peer from roster (audit record already written by FFI).
                                    settings.removePeer(t.fingerprint)
                                    revokeRotateTarget = null
                                    revokePassphrase = ""
                                    refresh()
                                },
                                onFailure = { e ->
                                    Log.e(
                                        TAG,
                                        "revokeDeviceAndRotateKey failed for ${t.fingerprint.take(8)}: ${e.message}",
                                        e,
                                    )
                                    revokeError = "Revoke + key rotation failed: ${e.message ?: "unknown error"}. " +
                                        "The device was NOT removed — please try again."
                                    revokeRotateTarget = null
                                    revokePassphrase = ""
                                },
                            )
                        }
                    },
                    variant = ButtonVariant.DANGER,
                ) {
                    if (revokeRotateInFlight) {
                        CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                    } else {
                        Text("Confirm revoke & rotate")
                    }
                }
            },
            dismissButton = {
                CopyPasteButton(
                    enabled = !revokeRotateInFlight,
                    onClick = {
                        revokeRotateTarget = null
                        revokePassphrase = ""
                    },
                    variant = ButtonVariant.GHOST,
                ) { Text("Cancel") }
            },
        )
    }

    // ── Revoke failure surface ────────────────────────────────────────────────
    revokeError?.let { msg ->
        GlassAlertDialog(
            onDismissRequest = { revokeError = null },
            title = { Text("Revocation incomplete") },
            text = { Text(msg) },
            confirmButton = {
                CopyPasteButton(onClick = { revokeError = null }, variant = ButtonVariant.GHOST) { Text("OK") }
            },
        )
    }

    // ── CopyPaste-crh3.34: "Revoke all" confirmation dialog ──────────────────
    // Mirrors macOS: title "Revoke all paired devices?" + two-sentence body +
    // DANGER confirm button + GHOST cancel button.
    if (revokeAllConfirmOpen) {
        GlassAlertDialog(
            onDismissRequest = { if (!revokeAllInFlight) revokeAllConfirmOpen = false },
            title = { Text("Revoke all paired devices?") },
            text = { Text(revokeAllConfirmBody()) },
            confirmButton = {
                CopyPasteButton(
                    enabled = !revokeAllInFlight,
                    onClick = {
                        revokeAllConfirmOpen = false
                        revokeAllInFlight = true
                        // Snapshot the peer list before launching the coroutine so
                        // mutations during the IO loop don't see a stale iterator.
                        val peersToRevoke = peers.toList()
                        scope.launch {
                            var anyFailed = false
                            for (p in peersToRevoke) {
                                // CopyPaste-94o4 ordering: write the audit record first;
                                // only remove the peer from the roster once the DB write
                                // succeeds — same guarantee as the single-peer "Revoke only" path.
                                val ok = withContext(Dispatchers.IO) {
                                    runCatching {
                                        revokeDeviceAudit(
                                            dbPath = settings.dbPath,
                                            key = settings.encryptionKey,
                                            fingerprint = p.fingerprint,
                                            name = p.displayName(),
                                        )
                                    }
                                }.fold(
                                    onSuccess = { true },
                                    onFailure = { e ->
                                        Log.e(
                                            TAG,
                                            "revokeAll: audit failed for ${p.fingerprint.take(8)}: ${e.message}",
                                            e,
                                        )
                                        false
                                    },
                                )
                                if (ok) {
                                    // CopyPaste-1jms.8: local removal only — no outbound unpair
                                    // signal on Android (no durable pending-unpair queue yet).
                                    settings.removePeer(p.fingerprint)
                                } else {
                                    anyFailed = true
                                }
                            }
                            revokeAllInFlight = false
                            if (anyFailed) {
                                revokeError = "Some devices could not be fully revoked. " +
                                    "Remaining devices were NOT removed — please retry."
                            }
                            refresh()
                        }
                    },
                    variant = ButtonVariant.DANGER,
                ) {
                    if (revokeAllInFlight) {
                        CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                    } else {
                        Text("Revoke all")
                    }
                }
            },
            dismissButton = {
                CopyPasteButton(
                    enabled = !revokeAllInFlight,
                    onClick = { revokeAllConfirmOpen = false },
                    variant = ButtonVariant.GHOST,
                ) { Text("Cancel") }
            },
        )
    }

    // ── SAS pairing modal (port of macOS SasPairingModal) ─────────────────────
    pairingPeer?.let { peer ->
        SasPairingDialog(
            peer = peer,
            settings = settings,
            onClose = { pairingPeer = null },
            onPaired = { refresh() },
        )
    }

    // ── Scan error surface ────────────────────────────────────────────────────
    scanError?.let { msg ->
        GlassAlertDialog(
            onDismissRequest = { scanError = null },
            title = { Text("Scanner unavailable") },
            text = { Text(msg) },
            confirmButton = {
                CopyPasteButton(onClick = { scanError = null }, variant = ButtonVariant.GHOST) { Text("OK") }
            },
        )
    }

    // Calm screen backdrop (STYLEGUIDE §6). Frosted only when translucent
    // and this screen owns its backdrop (standalone, not embedded in MainShell).
    val paintCanvas = shouldPaintCanvas(translucent, paintCanvasBackdrop)
    val scaffoldModifier = if (paintCanvas) modifier.screenCanvas(dark) else modifier

    Scaffold(
        modifier = scaffoldModifier,
        containerColor = if (translucent) androidx.compose.ui.graphics.Color.Transparent else c.bg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_devices),
                showBackButton = showBackButton,
                onBack = onBack,
                backContentDescription = "Back",
                // CopyPaste-crh3.34: "Revoke all" action mirrors macOS DevicesView
                // actions bar. DANGER variant matches macOS border-ide-danger/35 styling
                // (STYLEGUIDE §9.1). Disabled when no peers or an operation is in flight.
                actions = {
                    CopyPasteButton(
                        onClick = { revokeAllConfirmOpen = true },
                        variant = ButtonVariant.DANGER,
                        enabled = revokeAllEnabled(peers.size) && !revokeAllInFlight,
                        modifier = Modifier.padding(end = 8.dp),
                    ) {
                        Text("Revoke all")
                    }
                },
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {

            // ── Deliverable 1: own QR at the top, always visible, blurred ────
            // Shows THIS device's pairing QR at the top of the screen so the
            // user doesn't need to navigate to PairActivity to get scanned.
            // The QR is blurred by default (tap to reveal) because it encodes
            // the PAKE password + sync provisioning material. Reuses the same
            // blur/reveal pattern as PairActivity (Modifier.blur(16.dp) + overlay
            // label, first-tap reveals, second-tap regenerates and stays visible).
            // The QR is generated lazily in OwnQrSection. DevicesActivity now sets
            // FLAG_SECURE in onCreate (CopyPaste-92qs), so the reveal flow here is
            // screenshot-protected just like PairActivity's; the blur-at-rest is a
            // second layer of defence.
            OwnQrSection(settings = settings)

            // ── Single grouped inset device list (PARITY-SPEC §8) ─────────────
            // Apple Settings-style: this device first, then every paired peer,
            // then discovered (unpaired) LAN peers — ALL inside ONE glass
            // CopyPasteCard, rows separated by a single 1dp hairline divider.
            // Replaces the former stack of individually-elevated Cards.
            // CopyPaste-9ln4: renamed from "Devices" to "Paired devices" — avoids
            // duplicate with the TopBar title and matches the web SectionLabel fix.
            // bdac.48: sentence case to match all other section headers on this screen.
            SectionLabel("Paired devices")

            // CopyPaste-crh3.30: the device's active secondary (non-P2P) transport,
            // mirroring the macOS daemon's relay>supabase priority. Read from the
            // live sync config so a cloud-only peer is labelled Relay vs Cloud
            // instead of collapsing both into "Cloud".
            val cloudTransport = activeCloudTransport(
                relayActive = settings.relayEnabled && settings.isRelayConfigured,
                supabaseActive = settings.supabaseEnabled && settings.isSupabaseConfigured,
            )

            // Assemble the ordered row list so we know where dividers go (a
            // divider is drawn BEFORE every row except the first).
            val deviceRows: List<@Composable () -> Unit> = buildList {
                // This device — always first.
                ownIdentity?.let { identity ->
                    add { OwnDeviceRow(identity = identity, nowMs = nowMs, ownPublicIp = ownPublicIp) }
                }
                // Paired peers — pass the pre-computed online flag so the row dot
                // and the footer badge are always in sync.
                for (peer in peers) {
                    add {
                        PeerRow(
                            peer = peer,
                            online = onlineByFingerprint[peer.fingerprint] ?: false,
                            nowMs = nowMs,
                            cloudTransport = cloudTransport,
                            onUnpair = { unpairTarget = peer },
                            onRevoke = { revokeTarget = peer },
                        )
                    }
                }
                // Discovered (unpaired) LAN peers — only when P2P is enabled
                // (discovery is gated on it). Always show the section label + an
                // empty-state row while scanning so the LAN feature stays visible
                // instead of silently vanishing (pkd0 regression). RowDivider
                // between rows is added by the forEachIndexed renderer below.
                if (p2pEnabled) {
                    add {
                        // 1jms.20: use SectionLabel for visual consistency with all other
                        // section headers (Paired Devices, Your QR code, etc.).
                        SectionLabel("Discovered on your network")
                    }
                    if (discovered.isEmpty()) {
                        // CopyPaste-0nd4: add DiscoveryRingsIcon + text in a Row so the
                        // empty-state has an icon anchor and visual breathing room, matching
                        // the macOS .network-rings icon + text pattern in DevicesView.tsx.
                        add {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 16.dp, vertical = 12.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(12.dp),
                            ) {
                                DiscoveryRingsIcon(size = 36.dp)
                                Text(
                                    text = stringResource(R.string.no_devices_nearby),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = c.faint,
                                )
                            }
                        }
                    } else {
                        for (peer in discovered) {
                            add {
                                DiscoveredPeerRow(
                                    peer = peer,
                                    busy = pairStarting || pairingPeer != null,
                                    onPair = { startPairing(peer) },
                                )
                            }
                        }
                    }
                }
            }

            if (deviceRows.isNotEmpty()) {
                CopyPasteCard(accent = c.border) {
                    // STYLEGUIDE §3.2: rows separated by a single hairline divider.
                    deviceRows.forEachIndexed { index, row ->
                        if (index > 0) {
                            RowDivider()
                        }
                        row()
                    }
                }
            } else {
                // Empty state — no own-device row to anchor the list.
                NoPeerCard(
                    onPair = {
                        ctx.startActivity(Intent(ctx, PairActivity::class.java))
                    }
                )
            }

            if (p2pEnabled) {
                discoverError?.let { msg ->
                    Text(
                        text = msg,
                        color = c.danger,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }

            // ── Deliverable 2: Scan button opens the camera directly ─────────
            // Launches PortraitCaptureActivity (ZXing) via ScanContract without
            // routing through PairActivity. The scan result is forwarded to
            // PairActivity as a cppair:// deep-link so PAKE + provisioning still
            // run there unmodified.
            // CopyPaste-jkbo: replaced raw OutlinedButton with shared CopyPasteButton(SECONDARY).
            CopyPasteButton(
                onClick = { startScanFlow() },
                variant = ButtonVariant.SECONDARY,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(stringResource(R.string.btn_scan_qr))
            }

            Spacer(Modifier.height(24.dp))
        }
    }
}

private const val TAG = "DevicesActivity"
