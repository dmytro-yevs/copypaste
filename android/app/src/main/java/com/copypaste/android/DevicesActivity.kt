package com.copypaste.android

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.util.Log
import java.text.DateFormat
import java.util.Date
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.IdeAccent
import com.copypaste.android.ui.theme.IdeAccentDim
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeElevated
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdeInfo
import com.copypaste.android.ui.theme.IdeInfoDim
import com.copypaste.android.ui.theme.IdeSuccess
import com.copypaste.android.ui.theme.IdeText
import com.copypaste.android.ui.theme.IdeWarning
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.SectionLabel
import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// ─────────────────────────────────────────────────────────────────────────────
// §7 Liquid Glass Devices parity — pure logic helpers (testable without SDK)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Transport chip variants shown on each peer card.
 * P2P = direct local network; Cloud = relay/Supabase.
 */
internal enum class TransportChip { P2P, Cloud }

/**
 * Derive the transport chip for [peer]:
 * - P2P when [PairedPeer.syncAddr] or [PairedPeer.peerLocalIp] is non-blank,
 *   meaning we have a local-network address for this peer.
 * - Cloud otherwise (relay or Supabase-only peer).
 *
 * Defensive: never throws on null/blank fields.
 */
internal fun transportChipFor(peer: PairedPeer): TransportChip =
    if (peer.syncAddr.isNotBlank() || peer.peerLocalIp?.isNotBlank() == true)
        TransportChip.P2P
    else
        TransportChip.Cloud

/**
 * Format the own-device fingerprint: always shown in full (no truncation).
 * Mirrors §7 "full fingerprint+copy on own".
 */
internal fun formatOwnFingerprint(fp: String): String = fp

/**
 * Format a peer fingerprint: take(16)+"…"+takeLast(8).
 * Mirrors §7 "16…8 truncated+hover-copy on peers".
 */
internal fun formatPeerFingerprint(fp: String): String =
    fp.take(16) + "…" + fp.takeLast(8)

/**
 * QR countdown drain-bar progress in [0f, 1f].
 * [remainingSeconds] / [totalSeconds], clamped to [0f, 1f].
 */
internal fun qrCountdownProgress(remainingSeconds: Int, totalSeconds: Int): Float =
    (remainingSeconds.toFloat() / totalSeconds.toFloat()).coerceIn(0f, 1f)

/**
 * True when the QR is in the warning zone (≤15 s remaining).
 * Matches [DEVICES_QR_URGENT_THRESHOLD_SECONDS].
 */
internal fun isQrWarning(remainingSeconds: Int): Boolean =
    remainingSeconds <= DEVICES_QR_URGENT_THRESHOLD_SECONDS

/**
 * True when the PulseDot should animate: [online] && ![ reducedMotion].
 * Extracted so unit tests can verify the gate without Compose.
 */
internal fun shouldPulse(online: Boolean, reducedMotion: Boolean): Boolean =
    online && !reducedMotion

/**
 * "Online" recency threshold for the per-peer green dot.
 *
 * A peer that completed a successful P2P sync within the last [ONLINE_WINDOW_MS]
 * is rendered online (green dot); otherwise offline (grey). This mirrors the
 * macOS daemon's `ONLINE_THRESHOLD_SECS` (60 s) so both platforms agree on what
 * "online" means. The presence signal is [PairedPeer.lastSyncMs], stamped by
 * [FgsSyncLoop] (via [Settings.updatePeerLastSync]) on each successful dial —
 * NOT the old `lastSupabasePollWallTime` poll-cursor proxy.
 */
internal const val ONLINE_WINDOW_MS = 60_000L

/** True when [peer] synced within [ONLINE_WINDOW_MS] of [nowMs]. */
internal fun PairedPeer.isOnline(nowMs: Long = System.currentTimeMillis()): Boolean =
    lastSyncMs > 0L && (nowMs - lastSyncMs) <= ONLINE_WINDOW_MS

/**
 * Shared online-count state published by [DevicesScreen] and consumed by
 * [com.copypaste.android.ui.SyncStatusBadge] so both the footer dot+count AND
 * every PeerCard dot are driven by the SAME single computation.
 *
 * A paired peer is ONLINE iff its IP host appears in the current live mDNS
 * `discovered` set (IP-correlation — mDNS device_id is a UUID, NOT a cert
 * fingerprint, so we match on IP only), OR its lastSyncMs falls within
 * [ONLINE_WINDOW_MS] as a fallback.
 *
 * [DevicesScreen] updates this every ~1 s via [publish]. When the Devices tab
 * is not visible, [SyncStatusBadge] falls back to its own configured-target
 * count (value stays at whatever was last published).
 */
object DevicesOnlineState {
    private val _onlineCount = MutableStateFlow(-1)

    /** -1 = not yet computed (badge may fall back to its own logic). */
    val onlineCount: StateFlow<Int> = _onlineCount.asStateFlow()

    internal fun publish(count: Int) {
        _onlineCount.value = count
    }
}

/**
 * Forget a single paired peer locally: remove its roster entry (fingerprint,
 * sync address, KEK-wrapped session key). The peer is NOT notified; it may keep
 * trying to contact us until it is also unpaired on its side.
 *
 * Does NOT touch this device's P2P identity (cert/key) — we keep our own
 * identity so our OTHER pairings keep working and re-pairing needs no new cert.
 */
fun unpairPeer(settings: Settings, fingerprint: String) {
    settings.removePeer(fingerprint)
}

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
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val deviceKeyStore = remember { DeviceKeyStore(ctx) }
    val scope = rememberCoroutineScope()

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
            scanError = "Could not open the camera scanner: " +
                (e.message ?: e.javaClass.simpleName)
        }
    }

    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            launchScanner()
        } else {
            scanError = "Camera permission required to scan a pairing QR. " +
                "Grant it in Settings → Apps → CopyPaste → Permissions."
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
            val viaRecent = peer.isOnline(nowMs)
            peer.fingerprint to (viaMdns || viaRecent)
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


    // Publish live count so SyncStatusBadge (footer) reads the SAME value as the
    // peer cards — single source, zero divergence.
    DevicesOnlineState.publish(onlineByFingerprint.count { it.value })

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
                val cert = withContext(Dispatchers.IO) {
                    deviceKeyStore.peek() ?: deviceKeyStore.getOrCreate()
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
                    )
                }
                pairingPeer = peer
            } catch (e: Exception) {
                Log.w(TAG, "pairWithDiscovered failed: ${e.message}", e)
                discoverError = e.message ?: "Failed to start pairing."
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

    // ── Unpair confirmation ──────────────────────────────────────────────────
    unpairTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { unpairTarget = null },
            title = { Text("Forget paired device?") },
            text = {
                Text(
                    "This device will no longer sync with ${target.displayName()} over P2P. " +
                    "You can re-pair at any time by scanning a new QR code."
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    unpairTarget = null
                    unpairPeer(settings, target.fingerprint)
                    refresh()
                }) { Text("Forget", color = IdeDanger) }
            },
            dismissButton = {
                TextButton(onClick = { unpairTarget = null }) { Text("Cancel") }
            },
        )
    }

    // ── Revoke confirmation ──────────────────────────────────────────────────
    revokeTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { revokeTarget = null },
            title = { Text("Revoke pairing?") },
            text = {
                Text(
                    "${target.displayName()} will no longer connect over P2P, and a " +
                    "revocation record is kept. But a revoked device that still holds " +
                    "the shared sync key can keep reading cloud and relay items until " +
                    "you rotate the sync key. To rotate it, change the Sync Passphrase " +
                    "in Settings — every device must then re-enter the new passphrase " +
                    "(or re-pair) to keep syncing."
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    revokeTarget = null
                    // Forget the PEER locally (never our own p2pIdentity), then
                    // write a durable audit/revocation record on the IO dispatcher.
                    settings.removePeer(target.fingerprint)
                    refresh()
                    scope.launch {
                        val ok = withContext(Dispatchers.IO) {
                            runCatching {
                                revokeDeviceAudit(
                                    dbPath = settings.dbPath,
                                    key = settings.encryptionKey,
                                    fingerprint = target.fingerprint,
                                    name = target.displayName(),
                                )
                            }
                        }.fold(
                            onSuccess = { true },
                            onFailure = { e ->
                                Log.e(
                                    TAG,
                                    "revokeDeviceAudit failed for ${target.fingerprint.take(8)}: ${e.message}",
                                    e,
                                )
                                false
                            },
                        )
                        if (!ok) revokeError = "Failed to record revocation. The peer was unpaired locally."
                    }
                }) { Text("Revoke", color = IdeDanger) }
            },
            dismissButton = {
                TextButton(onClick = { revokeTarget = null }) { Text("Cancel") }
            },
        )
    }

    // ── Revoke failure surface ────────────────────────────────────────────────
    revokeError?.let { msg ->
        AlertDialog(
            onDismissRequest = { revokeError = null },
            title = { Text("Revocation incomplete") },
            text = { Text(msg) },
            confirmButton = {
                TextButton(onClick = { revokeError = null }) { Text("OK") }
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
        AlertDialog(
            onDismissRequest = { scanError = null },
            title = { Text("Scanner unavailable") },
            text = { Text(msg) },
            confirmButton = {
                TextButton(onClick = { scanError = null }) { Text("OK") }
            },
        )
    }

    Scaffold(
        modifier = modifier,
        containerColor = IdeBg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_devices),
                showBackButton = showBackButton,
                onBack = onBack,
                backContentDescription = "Back",
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
            // The QR is generated lazily in OwnQrSection; the Scaffold FLAG_SECURE
            // from PairActivity is NOT needed here because the QR is blurred at
            // rest and FLAG_SECURE on PairActivity still protects the reveal flow
            // when the user taps through to that screen.
            OwnQrSection(settings = settings)

            // ── Single unified device list ───────────────────────────────────
            // Parity with macOS DevicesView: this device first, then every
            // paired peer, then discovered (unpaired) LAN peers — all in one
            // continuous column, no separate section headers per list.
            SectionLabel("Devices")

            // This device — always first.
            ownIdentity?.let { identity ->
                OwnDeviceCard(identity = identity, nowMs = nowMs)
            }

            // Paired peers — pass the pre-computed online flag so the card dot
            // and the footer badge are always in sync.
            if (peers.isNotEmpty()) {
                for (peer in peers) {
                    PeerCard(
                        peer = peer,
                        online = onlineByFingerprint[peer.fingerprint] ?: false,
                        nowMs = nowMs,
                        onUnpair = { unpairTarget = peer },
                        onRevoke = { revokeTarget = peer },
                    )
                }
            } else if (ownIdentity == null) {
                // Show the empty state only when we also have no own-device card
                // to anchor the list (avoids a redundant prompt when the own
                // card is already present).
                NoPeerCard(
                    onPair = {
                        ctx.startActivity(Intent(ctx, PairActivity::class.java))
                    }
                )
            }

            // Discovered on your network (unpaired LAN peers).
            // Only shown when P2P is enabled (discovery is gated on it).
            if (p2pEnabled) {
                if (discovered.isNotEmpty()) {
                    for (peer in discovered) {
                        DiscoveredPeerCard(
                            peer = peer,
                            busy = pairStarting || pairingPeer != null,
                            onPair = { startPairing(peer) },
                        )
                    }
                }
                discoverError?.let { msg ->
                    Text(
                        text = msg,
                        color = IdeDanger,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }

            // ── Deliverable 2: Scan button opens the camera directly ─────────
            // Launches PortraitCaptureActivity (ZXing) via ScanContract without
            // routing through PairActivity. The scan result is forwarded to
            // PairActivity as a cppair:// deep-link so PAKE + provisioning still
            // run there unmodified.
            OutlinedButton(
                onClick = { startScanFlow() },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(stringResource(R.string.btn_scan_qr))
            }

            Spacer(Modifier.height(24.dp))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Own QR section (Deliverable 1)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Pixel side of the QR bitmap generated here — matches [QR_BITMAP_PX] in
 * PairActivity so both screens produce identical-quality codes.
 */
private const val DEVICES_QR_BITMAP_PX = 512

/**
 * On-screen dp side of the QR image inside the plate.
 * Slightly smaller than PairActivity's 240 dp to fit compactly in the
 * Devices list above the device cards.
 */
private const val DEVICES_QR_IMAGE_DP = 200

/** White backing-plate padding (each side, dp). */
private const val DEVICES_QR_PLATE_PADDING_DP = 10

/** Total reserved slot size: image + plate padding on both sides. */
private const val DEVICES_QR_SLOT_DP = DEVICES_QR_IMAGE_DP + DEVICES_QR_PLATE_PADDING_DP * 2

/** Mirrors PAIR_TOKEN_TTL_SECONDS in PairActivity (private there). */
private const val DEVICES_QR_TTL_SECONDS = 120

/** Mirrors PAIR_TOKEN_URGENT_THRESHOLD_SECONDS in PairActivity (private there). */
private const val DEVICES_QR_URGENT_THRESHOLD_SECONDS = 15

/**
 * Generates a QR [Bitmap] for [text] at [sizePx] pixels. Identical to
 * [encodeQrBitmap] in PairActivity — duplicated to avoid a cross-file
 * private reference; both produce the same ZXing QR_CODE output.
 */
private fun encodeDevicesQrBitmap(text: String, sizePx: Int): Bitmap {
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
 * Shows this device's pairing QR at the top of the Devices screen.
 *
 * Privacy model — identical to [PairActivity]:
 *  - QR is blurred ([Modifier.blur] 16 dp) by default; a "Tap to reveal"
 *    overlay guides the user.
 *  - First tap → unblurred (revealed).
 *  - Second tap → regenerates + stays visible (mirrors HW-A5 from PairActivity).
 *  - On expiry (2-minute TTL) the QR auto-regenerates and stays visible.
 *
 * The QR is generated on first composition via [startPairing] (same FFI call
 * as PairActivity). Failures show a muted error label so the rest of the
 * Devices screen still renders.
 *
 * FLAG_SECURE: this composable lives in DevicesScreen, which does NOT set
 * FLAG_SECURE. The QR is blurred at rest, so the secret material is not
 * readable from a screenshot in the default state. Users who tap to reveal
 * accept the exposure; a future hardening pass could set FLAG_SECURE on
 * DevicesActivity too, but that blocks the rest of the screen uselessly.
 */
@Composable
private fun OwnQrSection(settings: Settings) {
    val scope = rememberCoroutineScope()
    var qr by remember { mutableStateOf<PairingQrResult?>(null) }
    var qrBitmap by remember { mutableStateOf<Bitmap?>(null) }
    var loading by remember { mutableStateOf(false) }
    var errorMsg by remember { mutableStateOf<String?>(null) }
    var remainingSeconds by remember { mutableStateOf(0) }
    // Blurred by default; tap reveals; second tap regenerates (stays unblurred).
    var qrBlurred by remember { mutableStateOf(true) }

    val expired = qr != null && remainingSeconds <= 0

    fun generateQr(keepVisible: Boolean) {
        scope.launch {
            loading = true
            try {
                val result = withContext(Dispatchers.IO) {
                    startPairing(settings.deviceId, android.os.Build.MODEL ?: "Android")
                }
                val bmp = withContext(Dispatchers.Default) {
                    encodeDevicesQrBitmap(result.qr, DEVICES_QR_BITMAP_PX)
                }
                qr = result
                qrBitmap = bmp
                if (keepVisible) qrBlurred = false
            } catch (e: Exception) {
                errorMsg = e.message ?: e.javaClass.simpleName
            } finally {
                loading = false
            }
        }
    }

    // Countdown ticker — restarts whenever a fresh QR is issued. Auto-regenerates
    // on expiry and keeps the QR visible (mirrors PairActivity HW-A5).
    LaunchedEffect(qr) {
        if (qr == null) return@LaunchedEffect
        remainingSeconds = DEVICES_QR_TTL_SECONDS
        while (remainingSeconds > 0) {
            delay(1_000L)
            remainingSeconds -= 1
        }
        generateQr(keepVisible = true)
    }

    // Generate QR on first composition.
    LaunchedEffect(Unit) {
        if (qr != null || loading) return@LaunchedEffect
        generateQr(keepVisible = false)
    }

    SectionLabel("Your QR code")

    CopyPasteCard {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(20.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = "Let another device scan this to pair",
                style = MaterialTheme.typography.bodySmall,
                color = IdeDim,
                textAlign = TextAlign.Center,
            )

            Box(
                modifier = Modifier.size(DEVICES_QR_SLOT_DP.dp),
                contentAlignment = Alignment.Center,
            ) {
                val bmp = qrBitmap
                when {
                    loading -> {
                        CircularProgressIndicator(
                            modifier = Modifier.size(32.dp),
                            color = IdeAccent,
                            strokeWidth = 2.dp,
                        )
                    }
                    bmp != null && !expired -> {
                        Box(
                            modifier = Modifier
                                .size(DEVICES_QR_SLOT_DP.dp)
                                .clip(RoundedCornerShape(10.dp))
                                .then(
                                    if (qrBlurred) Modifier.blur(16.dp) else Modifier
                                )
                                .clickable {
                                    if (qrBlurred) {
                                        qrBlurred = false
                                    } else {
                                        generateQr(keepVisible = true)
                                    }
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            // White backing plate.
                            Box(
                                modifier = Modifier
                                    .size(DEVICES_QR_SLOT_DP.dp)
                                    .background(androidx.compose.ui.graphics.Color.White)
                                    .padding(DEVICES_QR_PLATE_PADDING_DP.dp),
                                contentAlignment = Alignment.Center,
                            ) {
                                Image(
                                    bitmap = bmp.asImageBitmap(),
                                    contentDescription = "Your pairing QR code — tap to reveal",
                                    modifier = Modifier.size(DEVICES_QR_IMAGE_DP.dp),
                                )
                            }
                            // Reveal overlay (only while blurred).
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
                        // Expired placeholder while auto-regeneration is in flight.
                        Text(
                            text = "Refreshing…",
                            style = MaterialTheme.typography.bodySmall,
                            color = IdeDim,
                        )
                    }
                }
            }

            // §7 Countdown / expiry label + drain bar.
            if (qr != null && !expired) {
                val urgent = isQrWarning(remainingSeconds)
                Text(
                    text = stringResource(R.string.pair_token_expires_in_seconds, remainingSeconds),
                    style = MaterialTheme.typography.bodySmall,
                    color = if (urgent) IdeDanger else IdeFaint,
                )
                // §7 QR countdown drain bar: thin determinate progress bar that
                // drains from full (1f) to empty (0f) over the 120 s TTL.
                // Colour switches to IdeWarning when ≤15 s remain (spec: "warning <20s";
                // we use the same threshold as DEVICES_QR_URGENT_THRESHOLD_SECONDS = 15).
                LinearProgressIndicator(
                    progress = { qrCountdownProgress(remainingSeconds, DEVICES_QR_TTL_SECONDS) },
                    modifier = Modifier.fillMaxWidth(),
                    color = if (urgent) IdeWarning else IdeFaint,
                    trackColor = IdeBorder,
                )
            }

            errorMsg?.let { msg ->
                Text(
                    text = "QR unavailable: $msg",
                    style = MaterialTheme.typography.bodySmall,
                    color = IdeDanger,
                    textAlign = TextAlign.Center,
                )
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

/** Display label for a peer: its name when set, else a short fingerprint. */
private fun PairedPeer.displayName(): String =
    name.ifBlank { "device ${fingerprint.take(8)}" }

// ─────────────────────────────────────────────────────────────────────────────
// Peer card
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Fixed width of the label column in the two-column metadata table.
 * Sized to fit the longest label ("Local IP" / "Public IP") at 11 sp so
 * values in all three card types (Own, Peer, Discovered) start at the same
 * horizontal position regardless of which card they appear in.
 */
private val META_LABEL_WIDTH: Dp = 72.dp

@Composable
private fun PeerCard(
    peer: PairedPeer,
    /**
     * Pre-computed online flag from [DevicesScreen] — the SINGLE source of truth
     * for this peer's online/offline state. Replaces the former per-card call to
     * [PairedPeer.isOnline] which diverged from the footer badge computation.
     */
    online: Boolean,
    /** Current epoch millis from the 1-second ticker in [DevicesScreen]. */
    nowMs: Long,
    onUnpair: () -> Unit,
    onRevoke: () -> Unit,
) {
    val ctx = LocalContext.current
    val dotColor = if (online) IdeSuccess else IdeFaint
    val chip = transportChipFor(peer)

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = IdeElevated),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, IdeBorder),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            // ── Header row: pulse dot + name + status + transport chip ───────
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                // §7 online pulse ring (replaces plain dot).
                PulseDot(online = online, modifier = Modifier.size(10.dp))
                Text(
                    text = peer.name.ifBlank { "Paired device" },
                    color = IdeText,
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f, fill = false),
                )
                Text(
                    text = if (online) "Online" else "Offline",
                    color = dotColor,
                    style = MaterialTheme.typography.labelMedium,
                )
                // §7 transport chip: P2P (IdeInfo) or Cloud (IdeAccent).
                TransportChipLabel(chip = chip)
            }

            Spacer(Modifier.height(10.dp))

            // ── Two-column aligned table ─────────────────────────────────────
            // Label column is [META_LABEL_WIDTH] wide; value column takes the
            // rest. Each row uses verticalAlignment = CenterVertically so
            // multi-line values don't cause the label to sit misaligned.
            // Only rows with non-blank values rendered — legacy pre-ABI-14
            // roster entries simply show fewer rows.
            val lastSyncText: String? = if (peer.lastSyncMs > 0L) {
                val elapsed = (nowMs - peer.lastSyncMs) / 1_000L
                when {
                    elapsed < 60 -> "${elapsed}s ago"
                    elapsed < 3600 -> "${elapsed / 60}m ago"
                    elapsed < 86400 -> "${elapsed / 3600}h ago"
                    else -> formatEpochMs(peer.lastSyncMs)
                }
            } else null

            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                peer.peerModel?.takeIf { it.isNotBlank() }?.let {
                    MetaRow(label = "Model", value = it)
                }
                peer.peerOs?.takeIf { it.isNotBlank() }?.let {
                    MetaRow(label = "OS", value = it)
                }
                peer.peerAppVersion?.takeIf { it.isNotBlank() }?.let {
                    MetaRow(label = "Version", value = it)
                }
                peer.peerLocalIp?.takeIf { it.isNotBlank() }?.let {
                    MetaRow(label = "Local IP", value = it)
                }
                peer.peerPublicIp?.takeIf { it.isNotBlank() }?.let {
                    MetaRow(label = "Public IP", value = it)
                }
                if (peer.pairedAtMs > 0L) {
                    MetaRow(label = "Paired", value = formatEpochMs(peer.pairedAtMs))
                }
                lastSyncText?.let {
                    MetaRow(label = "Last sync", value = it)
                }
                // RTT: shown when FgsSyncLoop has measured a live round-trip time.
                // FgsSyncLoop instrumentation (Ping/Pong over mTLS) deferred to CopyPaste-8dd.
                peer.latencyMs?.let {
                    MetaRow(label = "RTT", value = "$it ms")
                }
                // §7 Fingerprint row: peer shows take(16)+…+takeLast(8) + tap-to-copy.
                // Defensive: only shown when fingerprint is non-blank.
                peer.fingerprint.takeIf { it.isNotBlank() }?.let { fp ->
                    val truncated = formatPeerFingerprint(fp)
                    MonoMetaRow(
                        label = "Fingerprint",
                        value = truncated,
                        onTap = { copyToSystemClipboard(ctx, fp) },
                    )
                }
            }

            HorizontalDivider(
                modifier = Modifier.padding(vertical = 12.dp),
                color = IdeBorder.copy(alpha = 0.5f),
                thickness = 0.5.dp,
            )

            // ── Actions ─────────────────────────────────────────────────────
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(
                    onClick = onUnpair,
                    modifier = Modifier.weight(1f),
                ) {
                    Text("Unpair", color = IdeDanger)
                }
                Button(
                    onClick = onRevoke,
                    modifier = Modifier.weight(1f),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = IdeDanger.copy(alpha = 0.15f),
                        contentColor = IdeDanger,
                    ),
                ) {
                    Text("Revoke")
                }
            }
        }
    }
}

@Composable
private fun NoPeerCard(onPair: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = IdeElevated),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, IdeBorder),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = "No device paired",
                color = IdeDim,
                style = MaterialTheme.typography.bodyLarge,
            )
            Text(
                text = "Pair with a Mac running CopyPaste to enable P2P clipboard sync over your local network.",
                color = IdeFaint,
                style = MaterialTheme.typography.bodySmall,
            )
            Button(onClick = onPair) {
                Text("Pair a device")
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Own-device card
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun OwnDeviceCard(
    identity: P2pIdentity,
    /** Current epoch millis from the 1-second ticker — drives live IP refresh. */
    nowMs: Long,
) {
    // HB-1c: render THIS device's info at parity with the macOS "This Mac" card.
    // ABI 14 sends these same fields to peers (own gather in PairActivity /
    // DevicesActivity startPairing); we surface them locally too. Gathered live —
    // P2pIdentity only carries the id/fingerprint, the rest comes from the
    // platform (Build/BuildConfig) and a LAN-IPv4 enumeration. No synchronous
    // public-IP source on-device, so that row is omitted (matches the bootstrap
    // path, which sends public_ip = None for this device).
    val ctx = LocalContext.current
    val model = Build.MODEL.orEmpty().ifBlank { "Android" }
    val osVersion = "Android " + Build.VERSION.RELEASE
    val appVersion = BuildConfig.VERSION_NAME

    // Live local IP — re-read every ~5 s (keyed on nowMs / 5000) so a network
    // change (Wi-Fi handoff, VPN connect) is reflected promptly.
    // The bare `remember { lanIpv4Address() }` snapshot was stale on network
    // change because it was only evaluated once at first composition.
    val localIp = remember(nowMs / 5_000L) { lanIpv4Address() }

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = IdeElevated),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, IdeBorder),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            // Header: §7 pulse dot (always online) + model name + "Online"
            // + §7 "This Device" accent badge (parity with macOS "This Mac").
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                // Own device is always online — pulse ring always animates (unless
                // reduced motion is enabled).
                PulseDot(online = true, modifier = Modifier.size(10.dp))
                Text(
                    text = model,
                    color = IdeText,
                    style = MaterialTheme.typography.titleSmall,
                    modifier = Modifier.weight(1f, fill = false),
                )
                Text(
                    text = "Online",
                    color = IdeSuccess,
                    style = MaterialTheme.typography.labelMedium,
                )
                // §7 "This Device" accent badge.
                Text(
                    text = "This Device",
                    color = IdeAccent,
                    fontSize = 10.sp,
                    letterSpacing = 0.4.sp,
                    style = MaterialTheme.typography.labelSmall,
                    modifier = Modifier
                        .background(IdeAccentDim, RoundedCornerShape(4.dp))
                        .padding(horizontal = 6.dp, vertical = 2.dp),
                )
            }

            Spacer(Modifier.height(10.dp))

            // Two-column aligned table — same [META_LABEL_WIDTH] as PeerCard.
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                MetaRow(label = "Model", value = model)
                MetaRow(label = "OS", value = osVersion)
                MetaRow(label = "Version", value = appVersion)
                localIp?.let { MetaRow(label = "Local IP", value = it) }
                // §7 Fingerprint: own device shows FULL fingerprint + tap-to-copy.
                // Defensive: only shown when fingerprint is non-blank.
                identity.fingerprint.takeIf { it.isNotBlank() }?.let { fp ->
                    MonoMetaRow(
                        label = "Fingerprint",
                        value = formatOwnFingerprint(fp),
                        onTap = { copyToSystemClipboard(ctx, fp) },
                    )
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Discovered-peer card (LAN, unpaired)
// ─────────────────────────────────────────────────────────────────────────────

/** Short label for a discovered peer: name when set, else a short device id. */
private fun DiscoveredPeer.displayName(): String =
    deviceName.ifBlank { "Device ${deviceId.take(8)}" }

/**
 * One discovered (unpaired) LAN device row with a Pair button. Mirrors the macOS
 * DiscoveredRow: the Pair button is DISABLED when the peer advertises no
 * bootstrap port ([DiscoveredPeer.bport] == null) — a v1 peer that cannot do SAS
 * pairing — or while another pairing is in flight ([busy]).
 */
@Composable
private fun DiscoveredPeerCard(
    peer: DiscoveredPeer,
    busy: Boolean,
    onPair: () -> Unit,
) {
    // v1 peers (no bootstrap port) cannot do SAS pairing → disable Pair.
    val pairable = peer.bport != null
    val ip = peer.ipAddrs.firstOrNull()

    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = IdeElevated),
        border = androidx.compose.foundation.BorderStroke(0.5.dp, IdeBorder),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = peer.displayName(),
                    color = IdeText,
                    style = MaterialTheme.typography.titleSmall,
                )
                Spacer(Modifier.height(4.dp))
                // Fingerprint omitted; IP shown as an aligned table row matching
                // the layout of OwnDeviceCard and PeerCard.
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    ip?.let { MetaRow(label = "Local IP", value = it) }
                }
            }
            Button(
                onClick = onPair,
                enabled = pairable && !busy,
            ) {
                Text("Pair")
            }
        }
        if (!pairable) {
            Text(
                text = "This device does not support secure pairing.",
                color = IdeFaint,
                style = MaterialTheme.typography.labelSmall,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SAS pairing modal (port of macOS DevicesView SasPairingModal)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Modal that drives a discovery-initiated SAS pairing to completion.
 *
 * Behaviour mirrors the macOS [SasPairingModal]:
 *  - polls [pairGetSas] every [SAS_POLL_MS];
 *  - `initiating` → spinner ("Connecting…");
 *  - `awaiting_sas` with a code → shows the 6-digit SAS + Match / Doesn't match;
 *  - `awaiting_sas` without a code → "Waiting for the other device…";
 *  - `confirmed` → persists the peer (KEK-wrapped session key + fill-missing
 *    provisioning) and shows success;
 *  - `rejected` / `aborted` / `timed_out` → error;
 *  - a TRAILING `idle` observed AFTER an active state is itself terminal
 *    ("pairing ended"): if the user already accepted locally, treat as success,
 *    else show a neutral "ended" close state — never loop on idle.
 *
 * Closing before a terminal state calls [pairAbort] exactly once; after any
 * terminal state [pairReset] is called to clear the native state machine.
 *
 * SECURITY: the SAS code is shown on screen but NEVER logged; the session-key
 * bytes are wrapped + zeroized and never logged.
 */
@Composable
private fun SasPairingDialog(
    peer: DiscoveredPeer,
    settings: Settings,
    onClose: () -> Unit,
    onPaired: () -> Unit,
) {
    val scope = rememberCoroutineScope()

    // Current pairing status; starts optimistically at "initiating".
    var status by remember {
        mutableStateOf(
            PairStatus(
                state = "initiating",
                sas = null,
                role = null,
                peerFingerprint = null,
                peerSyncAddr = null,
                sessionKey = null,
                peerProvisioning = null,
                // ABI 14 (HB-1b): peer metadata, populated by the native side on confirm.
                peerModel = null,
                peerOs = null,
                peerAppVersion = null,
                peerLocalIp = null,
                peerPublicIp = null,
            )
        )
    }
    // Transient (non-terminal) poll/confirm error.
    var error by remember { mutableStateOf<String?>(null) }
    // True while a pairConfirmSas call is in flight (disables the buttons).
    var confirmPending by remember { mutableStateOf(false) }
    // Neutral terminal close state — handshake ended on a trailing idle without a
    // local confirm. Distinct from the wire `aborted` state.
    var ended by remember { mutableStateOf(false) }
    // True once a terminal Confirmed has been observed — closing then must NOT
    // call pairAbort (the pairing already succeeded).
    val confirmedRef = remember { mutableStateOf(false) }
    // True once the user locally accepted (clicked Match): disambiguates a
    // trailing idle (local-accepted + idle ⇒ success).
    val localAcceptedRef = remember { mutableStateOf(false) }

    val terminal = ended ||
        status.state == "confirmed" ||
        status.state == "rejected" ||
        status.state == "aborted" ||
        status.state == "timed_out"

    // Persist a confirmed pairing: KEK-wrap the session key, upsert the peer, and
    // apply peer provisioning fill-missing (copied from PairActivity). Runs on IO.
    suspend fun persistConfirmed(st: PairStatus) {
        val fingerprint = st.peerFingerprint ?: return
        val keyUBytes = st.sessionKey ?: return
        withContext(Dispatchers.IO) {
            val rawSessionKey = ByteArray(keyUBytes.size) { keyUBytes[it].toByte() }
            try {
                val (wrappedB64, ivB64) = settings.wrapSessionKey(rawSessionKey)
                val nowMs = System.currentTimeMillis()
                settings.upsertPeer(
                    PairedPeer(
                        fingerprint = fingerprint,
                        syncAddr = st.peerSyncAddr ?: "",
                        name = peer.deviceName,
                        sessionKeyWrappedB64 = wrappedB64,
                        sessionKeyIvB64 = ivB64,
                        lastSyncMs = nowMs,
                        pairedAtMs = nowMs,
                        // HB-1b (ABI 14): persist the peer's device metadata received
                        // over the discovery/SAS pairing for the Wave-3 device card.
                        peerModel = st.peerModel,
                        peerOs = st.peerOs,
                        peerAppVersion = st.peerAppVersion,
                        peerLocalIp = st.peerLocalIp,
                        peerPublicIp = st.peerPublicIp,
                    )
                )

                // Apply peer provisioning fill-missing — NEVER overwrite a value
                // this device already configured (mirror the daemon's rule and the
                // PairActivity QR block). Never log the derived key bytes.
                st.peerProvisioning?.let { prov ->
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
                    prov.derivedSyncKey?.takeIf { it.isNotEmpty() }?.let { keyU ->
                        if (settings.cloudSyncKeyDirect == null) {
                            val keyBytes = ByteArray(keyU.size) { keyU[it].toByte() }
                            settings.cloudSyncKeyDirect = keyBytes
                            applied += "derivedSyncKey"
                        }
                    }
                    if (applied.isNotEmpty()) {
                        Log.i(TAG, "SAS provisioning applied (fill-missing): ${applied.joinToString(", ")}")
                    }
                }
            } finally {
                // Zero the raw session key copy once it has been wrapped.
                rawSessionKey.fill(0)
            }
        }
    }

    // Poll pair_get_sas until a terminal state. The native state machine resets to
    // idle after a terminal outcome, so a trailing idle (after an active state) is
    // itself terminal — never re-poll on it.
    LaunchedEffect(peer.deviceId) {
        var sawActive = false
        while (true) {
            val next = try {
                withContext(Dispatchers.IO) { pairGetSas() }
            } catch (e: Exception) {
                error = e.message ?: "Pairing status unavailable"
                return@LaunchedEffect
            }

            when (next.state) {
                "initiating", "awaiting_sas" -> {
                    sawActive = true
                    status = next
                    delay(SAS_POLL_MS)
                }
                "confirmed" -> {
                    confirmedRef.value = true
                    status = next
                    persistConfirmed(next)
                    onPaired()
                    pairReset()
                    return@LaunchedEffect
                }
                "rejected", "aborted", "timed_out" -> {
                    status = next
                    pairReset()
                    return@LaunchedEffect
                }
                else -> {
                    // state == "idle"
                    if (sawActive) {
                        if (confirmedRef.value || localAcceptedRef.value) {
                            confirmedRef.value = true
                            // Persist from the last status we held the keys on.
                            persistConfirmed(status)
                            status = PairStatus(
                                state = "confirmed",
                                sas = null,
                                role = null,
                                peerFingerprint = status.peerFingerprint,
                                peerSyncAddr = status.peerSyncAddr,
                                sessionKey = null,
                                peerProvisioning = null,
                                // HB-1b: carry forward the peer metadata we last held.
                                peerModel = status.peerModel,
                                peerOs = status.peerOs,
                                peerAppVersion = status.peerAppVersion,
                                peerLocalIp = status.peerLocalIp,
                                peerPublicIp = status.peerPublicIp,
                            )
                            onPaired()
                        } else {
                            ended = true
                        }
                        pairReset()
                        return@LaunchedEffect
                    }
                    // Idle before any active state — keep waiting.
                    status = next
                    delay(SAS_POLL_MS)
                }
            }
        }
    }

    // Close: abort the pairing unless it already succeeded (exactly once), then
    // ALWAYS reset the native pairing state machine.
    //
    // HB-8: pairAbort() moves the SM to the terminal `Aborted` state but leaves
    // `try_begin` claimed, so without a follow-up pairReset() every later pairing
    // attempt failed with "a pairing is already in flight". pairReset() returns
    // the SM to Idle. It is idempotent and safe whether we aborted, already hit a
    // terminal state, or the pairing succeeded.
    fun handleClose() {
        if (!confirmedRef.value && !terminal) {
            // Abort branch: abort, then reset, on the same IO dispatcher so the
            // reset is ordered AFTER the abort.
            scope.launch(Dispatchers.IO) {
                pairAbort()
                pairReset()
            }
        } else {
            // Already-terminal / confirmed branch: nothing to abort, but still
            // clear the SM so the next pairing can claim it.
            scope.launch(Dispatchers.IO) { pairReset() }
        }
        onClose()
    }

    fun handleConfirm(accept: Boolean) {
        confirmPending = true
        error = null
        // Record the local accept up-front so a trailing idle is read as success.
        if (accept) localAcceptedRef.value = true
        scope.launch {
            try {
                withContext(Dispatchers.IO) { pairConfirmSas(accept) }
                if (!accept) {
                    // User said it doesn't match — abort path already handled by
                    // the native side; close immediately.
                    onClose()
                    return@launch
                }
                // On accept keep polling; the next tick reflects confirmed/rejected.
            } catch (e: Exception) {
                // The decision never reached the native side — undo the optimistic
                // accept flag so a later trailing idle isn't misread as success.
                localAcceptedRef.value = false
                error = e.message ?: "Failed to send decision"
            } finally {
                confirmPending = false
            }
        }
    }

    val title = peer.displayName()

    AlertDialog(
        onDismissRequest = { handleClose() },
        title = { Text("Pair “$title”") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                when {
                    ended -> {
                        Text(
                            "Pairing ended — check the other device.",
                            color = IdeDim,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                    status.state == "confirmed" -> {
                        Text(
                            "Paired ✓",
                            color = IdeSuccess,
                            style = MaterialTheme.typography.titleSmall,
                        )
                    }
                    status.state == "rejected" || status.state == "aborted" || status.state == "timed_out" -> {
                        Text(
                            when (status.state) {
                                "timed_out" -> "Pairing timed out."
                                "rejected" -> "Pairing was rejected."
                                else -> "Pairing was cancelled."
                            },
                            color = IdeDanger,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                    status.state == "awaiting_sas" && status.sas != null -> {
                        Text(
                            "Confirm this code matches the one shown on the other device.",
                            color = IdeDim,
                            style = MaterialTheme.typography.bodySmall,
                        )
                        Text(
                            text = status.sas ?: "",
                            color = IdeText,
                            textAlign = TextAlign.Center,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 32.sp,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 8.dp),
                        )
                    }
                    status.state == "awaiting_sas" -> {
                        // Accepted locally; waiting for the peer to also accept.
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            CircularProgressIndicator(modifier = Modifier.size(18.dp))
                            Text(
                                "Waiting for the other device…",
                                color = IdeDim,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                    else -> {
                        // initiating / idle-before-active → connecting spinner.
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            CircularProgressIndicator(modifier = Modifier.size(18.dp))
                            Text(
                                "Connecting…",
                                color = IdeDim,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                }
                error?.let { msg ->
                    if (!terminal) {
                        Text(msg, color = IdeDanger, style = MaterialTheme.typography.labelSmall)
                    }
                }
            }
        },
        confirmButton = {
            when {
                terminal -> {
                    TextButton(onClick = { onClose() }) { Text("Close") }
                }
                status.state == "awaiting_sas" && status.sas != null -> {
                    TextButton(
                        enabled = !confirmPending,
                        onClick = { handleConfirm(true) },
                    ) { Text(if (confirmPending) "…" else "Match") }
                }
                else -> {}
            }
        },
        dismissButton = {
            when {
                terminal -> {}
                status.state == "awaiting_sas" && status.sas != null -> {
                    TextButton(
                        enabled = !confirmPending,
                        onClick = { handleConfirm(false) },
                    ) { Text("Doesn't match", color = IdeDim) }
                }
                else -> {
                    TextButton(onClick = { handleClose() }) { Text("Cancel", color = IdeFaint) }
                }
            }
        },
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// §7 Liquid Glass Devices parity — Compose helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Read the system "remove animations" / "reduce motion" accessibility setting.
 * Returns true when the user has disabled animations (scale = 0) so [PulseDot]
 * shows a static dot instead of the expanding ring.
 */
@Composable
private fun rememberReducedMotion(): Boolean {
    val ctx = LocalContext.current
    return remember {
        val scale = android.provider.Settings.Global.getFloat(
            ctx.contentResolver,
            android.provider.Settings.Global.ANIMATOR_DURATION_SCALE,
            1f,
        )
        scale == 0f
    }
}

/**
 * Online presence indicator: a solid [IdeSuccess] dot with an expanding
 * semi-transparent ring when [online] is true and reduced-motion is off.
 *
 * Animation: [rememberInfiniteTransition] drives a scale 1→2 + alpha 0.4→0
 * on a 2 s tween loop. Ring is drawn BEHIND the solid dot so the dot itself
 * stays crisply visible while the ring expands and fades.
 *
 * Gate: animated only when [online] == true and the system "remove animations"
 * scale is not 0 (matches §7 / §8 "Respect prefers-reduced-motion").
 */
@Composable
private fun PulseDot(online: Boolean, modifier: Modifier = Modifier) {
    val reducedMotion = rememberReducedMotion()
    val dotColor = if (online) IdeSuccess else IdeFaint
    val animate = shouldPulse(online = online, reducedMotion = reducedMotion)

    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        if (animate) {
            val infiniteTransition = rememberInfiniteTransition(label = "pulse")
            val pulseScale by infiniteTransition.animateFloat(
                initialValue = 1f,
                targetValue = 2.2f,
                animationSpec = infiniteRepeatable(
                    animation = tween(durationMillis = 2000, easing = FastOutSlowInEasing),
                    repeatMode = RepeatMode.Restart,
                ),
                label = "pulseScale",
            )
            val pulseAlpha by infiniteTransition.animateFloat(
                initialValue = 0.4f,
                targetValue = 0f,
                animationSpec = infiniteRepeatable(
                    animation = tween(durationMillis = 2000, easing = FastOutSlowInEasing),
                    repeatMode = RepeatMode.Restart,
                ),
                label = "pulseAlpha",
            )
            Box(
                modifier = Modifier
                    .size(10.dp)
                    .scale(pulseScale)
                    .clip(CircleShape)
                    .background(IdeSuccess.copy(alpha = pulseAlpha)),
            )
        }
        // Solid dot always on top.
        Box(
            modifier = Modifier
                .size(10.dp)
                .clip(CircleShape)
                .background(dotColor),
        )
    }
}

/**
 * Transport chip pill: 10 sp uppercase label in a tinted rounded pill.
 * P2P = [IdeInfo] / [IdeInfoDim]; Cloud = [IdeAccent] / [IdeAccentDim].
 * Defensive: never crashes on absent transport info — callers derive [chip]
 * via [transportChipFor] which is always non-null.
 */
@Composable
private fun TransportChipLabel(chip: TransportChip) {
    val (text, fg, bg) = when (chip) {
        TransportChip.P2P -> Triple("P2P", IdeInfo, IdeInfoDim)
        TransportChip.Cloud -> Triple("CLOUD", IdeAccent, IdeAccentDim)
    }
    Text(
        text = text,
        color = fg,
        fontSize = 10.sp,
        letterSpacing = 0.6.sp,
        style = MaterialTheme.typography.labelSmall,
        modifier = Modifier
            .background(bg, RoundedCornerShape(4.dp))
            .padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Two-column aligned table row used in device cards.
 *
 * The label column is [META_LABEL_WIDTH] wide (fixed) so all labels in
 * OwnDeviceCard, PeerCard, and DiscoveredPeerCard start at the same horizontal
 * offset. Both text nodes are vertically centred within the row
 * (verticalAlignment = Alignment.CenterVertically) so multi-line values don't
 * cause the label to sit misaligned — fixing the former "Mac" misalignment in
 * the Model row.
 */
@Composable
private fun MetaRow(label: String, value: String) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = IdeDim,
            fontSize = 11.sp,
            modifier = Modifier.width(META_LABEL_WIDTH),
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
            color = IdeText,
            fontSize = 11.sp,
            modifier = Modifier.weight(1f),
        )
    }
}

/**
 * Like [MetaRow] but uses [MonoFontFamily] for the value and wraps the whole row
 * in a [clickable] that calls [onTap] — used for fingerprint rows where tap-to-copy
 * is desired (§7). [onTap] may be null if copy is not needed.
 */
@Composable
private fun MonoMetaRow(label: String, value: String, onTap: (() -> Unit)? = null) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .then(if (onTap != null) Modifier.clickable(onClick = onTap) else Modifier),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = IdeDim,
            fontSize = 11.sp,
            modifier = Modifier.width(META_LABEL_WIDTH),
        )
        Text(
            text = value,
            // §7 fingerprint uses MonoFontFamily (bundled JetBrains Mono) per §1/§10.
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = MonoFontFamily),
            color = IdeText,
            fontSize = 11.sp,
            modifier = Modifier.weight(1f),
        )
    }
}

/**
 * Copy [text] to the Android system clipboard using [ClipboardManager].
 * Used by fingerprint rows (§7 "tap-to-copy").
 */
private fun copyToSystemClipboard(ctx: Context, text: String) {
    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager ?: return
    cm.setPrimaryClip(ClipData.newPlainText("Fingerprint", text))
}

@Composable
private fun DeviceField(label: String, value: String) {
    Column {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = IdeDim,
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
            color = IdeText,
            fontSize = 11.sp,
        )
    }
}

private const val TAG = "DevicesActivity"

/**
 * Format a Unix epoch-millisecond timestamp as a short locale date+time string
 * for device-info fields. Returns "—" for zero / negative values (unknown).
 * Mirrors macOS formatEpochSecs (which uses toLocaleString()).
 */
private fun formatEpochMs(ms: Long): String {
    if (ms <= 0L) return "—"
    return DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT)
        .format(Date(ms))
}

/** Extract the host part from a "host:port" sync address, or return the full string. */
private fun syncAddrToIp(syncAddr: String): String? {
    if (syncAddr.isBlank()) return null
    // IPv6: [::1]:4242 → ::1; IPv4: 192.168.1.2:4242 → 192.168.1.2
    val v6 = Regex("""^\[(.+)]:\d+$""").find(syncAddr)
    if (v6 != null) return v6.groupValues[1]
    val colon = syncAddr.lastIndexOf(':')
    return if (colon > 0) syncAddr.substring(0, colon) else syncAddr
}

/** Poll cadence for refreshing peer state on the Devices screen. */
private const val PEER_POLL_MS = 10_000L

/** Poll cadence for refreshing the LAN-discovered peer list (~2 s). */
private const val DISCOVERED_POLL_MS = 2_000L

/** Poll cadence for the SAS pairing state machine (~500 ms). */
private const val SAS_POLL_MS = 500L

/**
 * Fixed bootstrap (SAS-pairing) listener port this device advertises in its mDNS
 * TXT record so peers can dial back to pair. A non-zero bport marks this device
 * SAS-pairing-capable (v2); the native discovery service binds/owns this port.
 */
// `internal` so the always-on [ClipboardService] FGS owns the discovery
// lifecycle with the SAME well-known bport (HB-2).
internal const val SAS_BPORT = 47_654
