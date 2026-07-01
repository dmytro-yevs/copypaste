package com.copypaste.android

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// ─────────────────────────────────────────────────────────────────────────────
// Pure decision logic (testable on a plain JVM — CopyPaste-vp63.39)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * True when [DevicesController.startPairing] should proceed: no pairing
 * request already in flight and no SAS modal already open for another peer.
 * Extracted from the inline early-return guard so it is unit-testable
 * without Settings/DeviceKeyStore/Compose.
 */
internal fun canStartPairing(pairStarting: Boolean, pairingPeer: DiscoveredPeer?): Boolean =
    !pairStarting && pairingPeer == null

/**
 * True when the "revoke & rotate key" passphrase dialog may be dismissed
 * (cancel button or scrim tap) — i.e. no revokeDeviceAndRotateKey call is
 * currently in flight. Extracted from the inline guard shared by the
 * dialog's onDismissRequest and its dismiss button.
 */
internal fun canDismissRevokeRotate(revokeRotateInFlight: Boolean): Boolean = !revokeRotateInFlight

/**
 * True when the "Revoke all" confirmation dialog may be dismissed — i.e. no
 * bulk revoke is currently in flight. Extracted from the inline
 * onDismissRequest guard.
 */
internal fun canDismissRevokeAllConfirm(revokeAllInFlight: Boolean): Boolean = !revokeAllInFlight

private const val TAG = "DevicesController"

// ─────────────────────────────────────────────────────────────────────────────
// DevicesController — discovery/pairing/unpair/revoke state + logic
// ─────────────────────────────────────────────────────────────────────────────

/**
 * State holder + business logic for the Devices screen — LAN discovery
 * polling, SAS pairing initiation, and unpair/revoke/revoke-and-rotate/
 * revoke-all actions.
 *
 * CopyPaste-vp63.39: extracted from the former `DevicesScreen` god-composable
 * in DevicesActivity.kt — every action body below was moved verbatim (local
 * `var`s became `by mutableStateOf` class properties; closures became named
 * methods). Obtain an instance via [rememberDevicesController]; DevicesScreen
 * reads the public state and wires the action functions into PeerRow /
 * DiscoveredPeerRow callbacks and the DevicesDialogs.kt dialog set.
 */
class DevicesController(
    private val ctx: Context,
    private val settings: Settings,
    private val deviceKeyStore: DeviceKeyStore,
    private val scope: CoroutineScope,
) {
    // ── Scan / camera permission (Deliverable 2) ──────────────────────────
    // The scan button on this screen launches the ZXing scanner directly — no
    // PairActivity intermediary. The scan result (a CPPAIR1.… payload) is
    // forwarded to PairActivity as a cppair:// deep-link so the full pair &
    // sync flow (PAKE bootstrap, key persistence, provisioning apply) still
    // runs there unmodified.
    var scanError by mutableStateOf<String?>(null)
        private set

    // Bound once by [rememberDevicesController] — Compose activity-result
    // launchers can only be created from composable context, so the launch
    // functions are handed in rather than created here.
    private var scanLauncherLaunch: ((ScanOptions) -> Unit)? = null
    private var cameraPermissionLauncherLaunch: ((String) -> Unit)? = null

    internal fun bindLaunchers(
        scanLauncherLaunch: (ScanOptions) -> Unit,
        cameraPermissionLauncherLaunch: (String) -> Unit,
    ) {
        this.scanLauncherLaunch = scanLauncherLaunch
        this.cameraPermissionLauncherLaunch = cameraPermissionLauncherLaunch
    }

    /** Forwards a successful scan to PairActivity via a cppair:// deep-link. */
    internal fun onScanResult(contents: String?) {
        contents ?: return
        val intent = Intent(ctx, PairActivity::class.java).apply {
            action = Intent.ACTION_VIEW
            data = android.net.Uri.parse("cppair://pair?p=${android.net.Uri.encode(contents)}")
        }
        ctx.startActivity(intent)
    }

    private fun launchScanner() {
        val opts = ScanOptions()
            .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
            .setPrompt("Scan the pairing QR on the other device")
            .setBeepEnabled(false)
            .setOrientationLocked(true)
            .setCaptureActivity(PortraitCaptureActivity::class.java)
        try {
            scanLauncherLaunch?.invoke(opts)
        } catch (e: Exception) {
            // CopyPaste-jwga: never surface raw exception detail to users.
            scanError = ErrorMessages.friendlyCameraError(e)
        }
    }

    internal fun onCameraPermissionResult(granted: Boolean) {
        if (granted) {
            launchScanner()
        } else {
            // CopyPaste-jwga: use sanitized, user-friendly permission message.
            scanError = ctx.getString(R.string.error_camera_permission_denied)
        }
    }

    fun startScanFlow() {
        val hasCamera = ContextCompat.checkSelfPermission(
            ctx, android.Manifest.permission.CAMERA
        ) == PackageManager.PERMISSION_GRANTED
        if (hasCamera) launchScanner() else cameraPermissionLauncherLaunch?.invoke(android.Manifest.permission.CAMERA)
    }

    fun dismissScanError() {
        scanError = null
    }

    // ── Roster / identity ──────────────────────────────────────────────────
    // Refreshed every [PEER_POLL_MS] so the online dots and last-sync labels
    // update as FgsSyncLoop stamps presence.
    var peers by mutableStateOf(settings.pairedPeers)
        private set
    var ownIdentity by mutableStateOf(settings.p2pIdentity)
        private set

    // CopyPaste-6qq1: own public IP from a one-shot STUN query (StunUtils.queryPublicIp).
    // Null until the coroutine resolves or when collectPublicIp is disabled.
    var ownPublicIp by mutableStateOf<String?>(null)
        internal set

    // 1-second clock tick — drives smooth "Xm ago" / "Xs ago" updates and the
    // online-dot recomputation without a separate per-card timer.
    var nowMs by mutableLongStateOf(System.currentTimeMillis())
        internal set

    fun refresh() {
        peers = settings.pairedPeers
        ownIdentity = settings.p2pIdentity
    }

    // ── LAN discovery + SAS pairing state ──────────────────────────────────
    // P2P must be enabled for discovery (parity with the daemon gating
    // discovery behind start_p2p). When disabled we neither advertise nor browse.
    val p2pEnabled: Boolean = settings.p2pSyncEnabled

    // Non-paired, SAS-capable peers discovered on the LAN (refreshed by the
    // poll loop below). Paired peers are filtered out natively via `paired`.
    var discovered by mutableStateOf<List<DiscoveredPeer>>(emptyList())
        private set

    // The peer a SAS pairing modal is currently open for, or null. Setting it
    // non-null opens the modal (which begins polling pair_get_sas).
    var pairingPeer by mutableStateOf<DiscoveredPeer?>(null)
        private set

    // True while pair_with_discovered is in flight (before the modal opens).
    var pairStarting by mutableStateOf(false)
        private set

    // Inline error shown beneath the discovered list (e.g. another pairing busy).
    var discoverError by mutableStateOf<String?>(null)
        private set

    /**
     * SINGLE SOURCE OF TRUTH: online map keyed by fingerprint.
     *
     * A paired peer is ONLINE iff:
     *   (a) its IP host (from syncAddr or peerLocalIp) appears in the live mDNS
     *       `discovered` set (IP-correlation — mDNS device_id is a UUID, NOT a
     *       cert fingerprint, so we match on IP only), OR
     *   (b) its lastSyncMs falls within ONLINE_WINDOW_MS of nowMs (fallback for
     *       peers not currently advertising on mDNS, e.g. were online recently).
     *
     * Threaded to every site that shows an online indicator: PeerCard dot AND
     * the footer count via [DevicesOnlineState]. Removes the prior divergence
     * where the footer counted configured targets while each card independently
     * called peer.isOnline().
     */
    val onlineByFingerprint: Map<String, Boolean>
        get() {
            val discoveredIps: Set<String> = discovered.flatMap { it.ipAddrs }.toHashSet()
            return peers.associate { peer ->
                val peerIpHosts = listOfNotNull(
                    // host part of "host:port" (substringBeforeLast tolerates bare host).
                    peer.syncAddr.takeIf { it.isNotEmpty() }?.substringBeforeLast(':'),
                    peer.peerLocalIp?.takeIf { it.isNotEmpty() },
                )
                val viaMdns = peerIpHosts.any { host -> discoveredIps.contains(host) }
                // CopyPaste-d6z3: use isPeerOnline (recentSync OR mDNS-discovered)
                // instead of the old peer.isOnline() which only checked the 60 s
                // ONLINE_WINDOW_MS gate. isPeerOnline uses RECENT_SYNC_MS (5 min)
                // matching macOS parity.
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

    /**
     * Publish live count + most-recent peer activity so SyncStatusBadge
     * (footer) reads the SAME values as the peer cards — single source, zero
     * divergence. Called once per composition from [rememberDevicesController]
     * (matches the original inline call-site placement).
     */
    fun publishOnlineState() {
        val maxLastSyncMs = peers.maxOfOrNull { it.lastSyncMs } ?: 0L
        DevicesOnlineState.publish(
            count = onlineByFingerprint.count { it.value },
            maxLastSyncMs = maxLastSyncMs,
        )
    }

    /**
     * Deliverable 1: auto-open SAS modal on screen entry. Triggered when:
     * (a) the user tapped the incoming-pair notification
     * ([autoOpenSasOnEntry] = true), OR (b) general entry — poll once to catch
     * awaiting_sas for EITHER role so the modal appears regardless of who
     * initiated. Uses a sentinel [DiscoveredPeer] with the state machine's peer
     * info; if the native library is absent this is a safe no-op.
     */
    internal suspend fun checkAutoOpenSas(autoOpenSasOnEntry: Boolean) {
        if (!isNativeLibraryLoaded) return
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

    /**
     * mDNS discovery lifecycle lives in ClipboardService (HB-2). Discovery
     * (the mDNS advert + the standing SAS-pairing responder on [SAS_BPORT]) is
     * started/stopped by the always-on [ClipboardService] FGS, NOT here.
     * Hosting it on this screen meant the responder died the moment the
     * Devices screen closed, so a Mac→Android pair got "Connection refused".
     * The FGS keeps it alive for the lifetime of the service; this method only
     * polls the resulting peer snapshot.
     *
     * HB-4: listDiscovered marks `paired` by IP-correlation now (the mDNS
     * device_id is a UUID, not a cert fingerprint, so the old fingerprint-compare
     * never matched). We pass the set of IP hosts we have paired with — each
     * peer's syncAddr host plus its peerLocalIp — and drop the matched entries.
     */
    internal suspend fun pollDiscoveredOnce() {
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
    }

    /** Clears the discovered list when P2P is (re)disabled. */
    internal fun clearDiscovered() {
        discovered = emptyList()
    }

    /** Begin a discovery-initiated SAS pairing as initiator, then open the modal. */
    fun startPairing(peer: DiscoveredPeer) {
        if (!canStartPairing(pairStarting, pairingPeer)) return
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
                        // screen entry (see [rememberDevicesController]). Null when
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

    /** Closes the SAS pairing modal (does not cancel an in-flight pairing). */
    fun closePairing() {
        pairingPeer = null
    }

    // ── Unpair / revoke ─────────────────────────────────────────────────────
    // CopyPaste-vp63.39: unpair/revoke/revoke-rotate/revoke-all state + actions
    // live in [DevicesRevokeActions] (a separate file, to keep both classes
    // under the 500-line budget). [refresh] is threaded through so a
    // successful mutation re-reads the roster.
    val revoke: DevicesRevokeActions = DevicesRevokeActions(
        settings = settings,
        scope = scope,
        onRefresh = ::refresh,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// rememberDevicesController — wires Compose effects/launchers into DevicesController
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Creates (and remembers) a [DevicesController] and wires every LaunchedEffect
 * / activity-result launcher it needs. Moved verbatim from the former
 * `DevicesScreen` composable (CopyPaste-vp63.39) — same poll cadences, same
 * effect keys, same call-site ordering.
 */
@Composable
fun rememberDevicesController(
    settings: Settings,
    deviceKeyStore: DeviceKeyStore,
    autoOpenSasOnEntry: Boolean,
): DevicesController {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val controller = remember(settings, deviceKeyStore) {
        DevicesController(ctx = ctx, settings = settings, deviceKeyStore = deviceKeyStore, scope = scope)
    }

    val scanLauncher = rememberLauncherForActivityResult(ScanContract()) { result ->
        controller.onScanResult(result.contents)
    }
    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted -> controller.onCameraPermissionResult(granted) }
    controller.bindLaunchers(
        scanLauncherLaunch = { opts -> scanLauncher.launch(opts) },
        cameraPermissionLauncherLaunch = { perm -> cameraPermissionLauncher.launch(perm) },
    )

    // CopyPaste-6qq1: own public IP from a one-shot STUN query (StunUtils.queryPublicIp).
    LaunchedEffect(Unit) {
        controller.ownPublicIp = withContext(Dispatchers.IO) {
            StunUtils.queryPublicIp(settings.collectPublicIp)
        }
    }

    // 1-second clock tick.
    LaunchedEffect(Unit) {
        while (true) {
            delay(1_000L)
            controller.nowMs = System.currentTimeMillis()
        }
    }

    // Refresh the roster every poll interval so the online dots and last-sync
    // labels update as FgsSyncLoop stamps presence.
    LaunchedEffect(Unit) {
        while (true) {
            delay(PEER_POLL_MS)
            controller.refresh()
        }
    }

    // Deliverable 1: auto-open SAS modal on screen entry.
    LaunchedEffect(Unit) {
        controller.checkAutoOpenSas(autoOpenSasOnEntry)
    }

    // Publish the footer online-count/recency — recomputed every recomposition,
    // same call-site semantics as the original inline
    // DevicesOnlineState.publish(...) call.
    controller.publishOnlineState()

    // Poll the discovered peer list every ~2 s while P2P is enabled.
    LaunchedEffect(controller.p2pEnabled) {
        if (!controller.p2pEnabled) {
            controller.clearDiscovered()
        } else {
            while (true) {
                controller.pollDiscoveredOnce()
                delay(DISCOVERED_POLL_MS)
            }
        }
    }

    return controller
}
