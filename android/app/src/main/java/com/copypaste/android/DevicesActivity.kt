package com.copypaste.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
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
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp

import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.LocalLiquidTokens
import com.copypaste.android.ui.theme.LocalSkin
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.SectionLabel
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.SkinRowTreatment
import com.copypaste.android.ui.theme.LocalPalette
import com.copypaste.android.ui.theme.RadiusChip
import com.copypaste.android.ui.theme.auroraCanvas
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.paletteAurora
import com.copypaste.android.ui.theme.rememberTranslucency
import com.copypaste.android.ui.theme.skinTokens
import com.copypaste.android.ui.theme.tintBlobCanvas
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
 * True when the QR is in the warning zone (≤20 s remaining).
 * Matches [DEVICES_QR_URGENT_THRESHOLD_SECONDS] (PARITY-SPEC §10 / audit #26).
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
 * True when the aurora animated canvas should be painted as the screen backdrop.
 *
 * Gating rules (A-C2):
 *  - [background] must be [SkinBackground.AURORA] (Classic only).
 *  - [translucent] must be true (user pref; same gate as before).
 *  - [paintCanvasBackdrop] must be true (standalone vs. embedded gate; same as before).
 *
 * Classic keeps the SAME condition as before — byte-identical output.
 * Quiet (FLAT) and Vapor (TINT_BLOB) return false here.
 *
 * Extracted so it can be unit-tested without the Compose runtime.
 */
internal fun shouldPaintAurora(
    background: SkinBackground,
    translucent: Boolean,
    paintCanvasBackdrop: Boolean,
): Boolean = background == SkinBackground.AURORA && translucent && paintCanvasBackdrop

/**
 * True when a static tinted blob should be painted as the screen backdrop.
 *
 * Gating rules (A-C2):
 *  - [background] must be [SkinBackground.TINT_BLOB] (Vapor only).
 *  - [translucent] must be true (same pref gate as aurora).
 *  - [paintCanvasBackdrop] must be true (standalone vs. embedded gate).
 *
 * Extracted so it can be unit-tested without the Compose runtime.
 */
internal fun shouldPaintTintBlob(
    background: SkinBackground,
    translucent: Boolean,
    paintCanvasBackdrop: Boolean,
): Boolean = background == SkinBackground.TINT_BLOB && translucent && paintCanvasBackdrop

/**
 * CopyPaste-mgkr / CopyPaste-1jms.4 (NG-3): trust label for a paired peer.
 *
 * Returns "Verified" only when [PairedPeer.sasVerified] is true — meaning the
 * peer was admitted through the SAS (Short Authentication String) flow that
 * proves absence of a man-in-the-middle. All historical roster entries default
 * to sasVerified=true for backward-compatibility.
 *
 * Peers admitted by any other mechanism (cloud-import, admin provisioning, etc.)
 * have sasVerified=false and receive "Unverified" so users can distinguish them.
 *
 * Extracted as a pure function for unit-testability without the Compose runtime.
 */
internal fun trustLabel(peer: PairedPeer): String =
    if (peer.sasVerified) "Verified" else "Unverified"

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
 * How recent a last_sync_ms must be to count as "connected" in the badge
 * (PG-11). Mirrors macOS [SyncStatusChip.tsx] `RECENT_SYNC_MS = 5 * 60 * 1000`.
 * A peer that has not synced within this window is considered stale even if it
 * is still technically in the ONLINE_WINDOW_MS bracket — the badge should only
 * show green when we have evidence of a recent successful exchange.
 *
 * [SyncStatusBadge] should gate its "connected" colour on this threshold when
 * falling back to the configured-count path (PG-41 / PG-11 follow-up):
 * `lastActivityMs.value > 0 && (now - lastActivityMs.value) <= RECENT_SYNC_MS`.
 */
// c4q2.5: This value mirrors copypaste_ipc::SYNC_BADGE_RECENT_MS (crates/copypaste-ipc/src/methods.rs:208).
// Both must stay equal — if the Rust constant changes, update this too (and vice-versa).
internal const val RECENT_SYNC_MS = 5 * 60 * 1_000L

/**
 * CopyPaste-d6z3: pure online-derivation function matching macOS daemon logic.
 *
 * A peer is "online" iff EITHER:
 *  (a) its [lastSyncMs] is within [recentSyncMs] of [nowMs] (recent successful sync), OR
 *  (b) it is currently in the mDNS discovery table ([isMdnsDiscovered]).
 *
 * This mirrors the macOS `isPeerOnline` derivation: online = recentSync || mDNSDiscovered.
 * [onlineWindowMs] is retained as a separate parameter for future use (e.g. a tighter
 * P2P-contact window gate); currently [recentSyncMs] is the sole lastSyncMs gate.
 *
 * Pure function: no Android runtime dependencies — unit-testable without an emulator.
 */
internal fun isPeerOnline(
    lastSyncMs: Long,
    isMdnsDiscovered: Boolean,
    nowMs: Long,
    onlineWindowMs: Long,
    recentSyncMs: Long,
): Boolean {
    val recentSync = lastSyncMs > 0L && (nowMs - lastSyncMs) <= recentSyncMs
    return recentSync || isMdnsDiscovered
}

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
 *
 * ## PG-11 recency gate
 * [lastActivityMs] carries the most-recent [PairedPeer.lastSyncMs] across all
 * peers. [SyncStatusBadge] should show "connected" (green) only when this value
 * is within [RECENT_SYNC_MS] of the current wall time. A link idle for >5 min
 * should show the grey idle dot even if count > 0 (parity with macOS chip).
 */
object DevicesOnlineState {
    private val _onlineCount = MutableStateFlow(-1)
    private val _lastActivityMs = MutableStateFlow(0L)

    /** -1 = not yet computed (badge may fall back to its own logic). */
    val onlineCount: StateFlow<Int> = _onlineCount.asStateFlow()

    /**
     * Wall-clock ms of the most-recent successful peer sync across all peers,
     * or 0 when no sync has ever occurred. Published alongside [onlineCount] so
     * [SyncStatusBadge] can apply the [RECENT_SYNC_MS] recency gate (PG-11)
     * without re-reading Settings.
     */
    val lastActivityMs: StateFlow<Long> = _lastActivityMs.asStateFlow()

    /**
     * CopyPaste-lwnz: true while a sync operation (cloud poll or P2P dial) is
     * actively in flight inside [FgsSyncLoop]. Consumed by [SyncStatusBadge] to
     * drive the SYNCING badge state (green with distinct label) so the badge is
     * no longer a dead state. Set via [setSyncing]; cleared automatically when
     * the operation completes.
     *
     * Thread-safe: [MutableStateFlow.value] assignments are atomic.
     */
    private val _isSyncing = MutableStateFlow(false)
    val isSyncing: StateFlow<Boolean> = _isSyncing.asStateFlow()

    /**
     * Called by [FgsSyncLoop] immediately before starting a sync operation and
     * again (with [active]=false) when the operation finishes (success or error).
     * Safe to call from any thread.
     */
    fun setSyncing(active: Boolean) {
        _isSyncing.value = active
    }

    /**
     * CopyPaste-5917.52: true when the last sync attempt failed with a hard error
     * (backend auth failure, relay unreachable, persistent P2P dial failure) and
     * the daemon has not recovered since. Set by [FgsSyncLoop] via [setSyncError].
     *
     * When true AND the OS has internet, [resolveSyncBadgeState] returns
     * [SyncBadgeState.DaemonUnreachable] (red dot) — making [DaemonUnreachable]
     * reachable via the production code path for the first time. Previously the
     * state was only reachable via the IPC path that does not yet exist on Android.
     *
     * Thread-safe: [MutableStateFlow.value] assignments are atomic.
     */
    private val _isSyncError = MutableStateFlow(false)
    val isSyncError: StateFlow<Boolean> = _isSyncError.asStateFlow()

    /**
     * Called by [FgsSyncLoop] when a sync operation ends in a hard error
     * ([error]=true) or recovers ([error]=false). Safe to call from any thread.
     */
    fun setSyncError(error: Boolean) {
        _isSyncError.value = error
    }

    internal fun publish(count: Int, maxLastSyncMs: Long = 0L) {
        _onlineCount.value = count
        if (maxLastSyncMs > _lastActivityMs.value) {
            _lastActivityMs.value = maxLastSyncMs
        }
    }

    /**
     * PG-41: start a background polling loop that publishes [onlineCount] /
     * [lastActivityMs] every [BACKGROUND_POLL_MS] using [Settings.pairedPeers]
     * and [isPeerOnline]. Intended to be called once from
     * [CopyPasteApplication.onCreate] (or a long-lived coroutine scope) so the
     * footer badge shows the real peer count BEFORE [DevicesScreen] is ever shown,
     * removing the binary fallback in [SyncStatusBadge].
     *
     * CopyPaste-d6z3: uses [isPeerOnline] with [RECENT_SYNC_MS] so the background
     * badge count matches macOS parity (online = recentSync OR mDNS-discovered).
     * The mDNS signal is not available in this context (it lives in ClipboardService),
     * so isMdnsDiscovered=false is passed; [DevicesScreen] provides the full composite
     * signal via [onlineByFingerprint] while the screen is visible.
     *
     * Safe to call from any coroutine scope; the loop exits when the scope is
     * cancelled. Does NOT use mDNS (that lives in ClipboardService).
     *
     * Note: caller must ensure [isNativeLibraryLoaded] before starting, or wrap
     * the body in a guard, to avoid crashing on devices where the .so failed.
     */
    suspend fun startBackgroundPolling(settings: Settings) {
        while (true) {
            val peers = settings.pairedPeers
            val nowMs = System.currentTimeMillis()
            // CopyPaste-d6z3: use isPeerOnline with RECENT_SYNC_MS (5 min, macOS parity)
            // instead of the old isOnline() which used the 60 s ONLINE_WINDOW_MS gate.
            // isMdnsDiscovered=false: mDNS lives in ClipboardService, unavailable here;
            // DevicesScreen overrides with the full composite signal while visible.
            val count = peers.count { peer ->
                isPeerOnline(
                    lastSyncMs = peer.lastSyncMs,
                    isMdnsDiscovered = false,
                    nowMs = nowMs,
                    onlineWindowMs = ONLINE_WINDOW_MS,
                    recentSyncMs = RECENT_SYNC_MS,
                )
            }
            val maxLastSyncMs = peers.maxOfOrNull { it.lastSyncMs } ?: 0L
            publish(count = count, maxLastSyncMs = maxLastSyncMs)
            delay(BACKGROUND_POLL_MS)
        }
    }

    /** Poll cadence for [startBackgroundPolling] — 30 s (parity with macOS chip). */
    private const val BACKGROUND_POLL_MS = 30_000L
}

/**
 * Forget a single paired peer locally: remove its roster entry (fingerprint,
 * sync address, KEK-wrapped session key).
 *
 * CopyPaste-1jms.8: Android cannot send a mutual unpair signal to the peer
 * because the Android app has no live mTLS channel management equivalent to the
 * macOS daemon's `send_unpair_signal_if_connected()` / `queue_unpair_for_offline_delivery()`
 * (crates/copypaste-daemon/src/ipc.rs:998-1052). A `ControlMsg::Unpair` would
 * need:
 *   (a) a persistent mTLS connection handle to the peer, OR
 *   (b) a durable pending-unpair queue flushed on next P2P dial.
 * Neither exists on Android yet — the P2P dialer (syncWithPeer FFI) is
 * one-shot pull, not a live connection.
 *
 * As a result the revoked peer continues trying to sync until it is also
 * unpaired on its side (or times out). This is tracked as a known limitation.
 * Backend support required: expose a "queue_unpair" IPC call or durable
 * pending-action table that FgsSyncLoop can flush on next dial.
 *
 * Does NOT touch this device's P2P identity (cert/key) — we keep our own
 * identity so our OTHER pairings keep working and re-pairing needs no new cert.
 */
fun unpairPeer(settings: Settings, fingerprint: String) {
    settings.removePeer(fingerprint)
    // CopyPaste-1jms.8: local removal is all we can do on Android today.
    // Log the limitation so it is visible in diagnostics; the revoked peer
    // will continue dialling until it is also unpaired on its own side.
    Log.w(
        "DevicesActivity",
        "unpairPeer: peer ${fingerprint.take(16)}… removed locally. " +
            "No unpair signal sent — Android lacks a durable pending-unpair queue " +
            "(backend support needed: see CopyPaste-1jms.8).",
    )
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
    /** §1: paint the aurora backdrop here (standalone) vs. via MainShell (embedded). */
    paintCanvasBackdrop: Boolean = true,
) {
    val ctx = LocalContext.current
    val c = LocalIdeColors.current
    val settings = remember { Settings(ctx) }
    val deviceKeyStore = remember { DeviceKeyStore(ctx) }
    val scope = rememberCoroutineScope()
    // §1 aurora canvas backdrop (glass surfaces frost over real colour).
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()
    // A-C2: skin token bundle drives background mode and row layout.
    val tok = skinTokens(LocalSkin.current)

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
                TextButton(onClick = {
                    unpairTarget = null
                    unpairPeer(settings, target.fingerprint)
                    refresh()
                }) { Text("Unpair", color = c.danger) }
            },
            dismissButton = {
                TextButton(onClick = { unpairTarget = null }) { Text("Cancel") }
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
                TextButton(onClick = {
                    val t = revokeTarget
                    revokeTarget = null
                    if (t != null) {
                        revokePassphrase = ""
                        revokeRotateTarget = t
                    }
                }) {
                    Text("Revoke & rotate key", color = c.danger)
                }
            },
            dismissButton = {
                Row(horizontalArrangement = Arrangement.spacedBy(0.dp)) {
                    // "Revoke only" — left; performs the plain audit+remove path.
                    TextButton(onClick = {
                        val t = revokeTarget ?: return@TextButton
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
                    }) { Text("Revoke only", color = c.danger) }

                    TextButton(onClick = { revokeTarget = null }) { Text("Cancel") }
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
                TextButton(
                    enabled = isValidRotatePassphrase(revokePassphrase) && !revokeRotateInFlight,
                    onClick = {
                        val t = revokeRotateTarget ?: return@TextButton
                        val passphrase = revokePassphrase
                        if (!isValidRotatePassphrase(passphrase)) return@TextButton
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
                ) {
                    if (revokeRotateInFlight) {
                        CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                    } else {
                        Text("Confirm revoke & rotate", color = c.danger)
                    }
                }
            },
            dismissButton = {
                TextButton(
                    enabled = !revokeRotateInFlight,
                    onClick = {
                        revokeRotateTarget = null
                        revokePassphrase = ""
                    },
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
        GlassAlertDialog(
            onDismissRequest = { scanError = null },
            title = { Text("Scanner unavailable") },
            text = { Text(msg) },
            confirmButton = {
                TextButton(onClick = { scanError = null }) { Text("OK") }
            },
        )
    }

    // A-C2: three-way background canvas gating driven by tok.background.
    //
    // CLASSIC (AURORA, glow=.62): animated aurora canvas — same condition as before;
    //   byte-identical for Classic (shouldPaintAurora reproduces the prior guard exactly).
    // QUIET (FLAT, glow=0): no canvas — Scaffold uses plain solid c.bg; containerColor
    //   stays opaque so the FLAT surface reads clean.
    // VAPOR (TINT_BLOB, glow=.45): static tinted radial blob — a single large
    //   accent-tinted radial gradient centred on the canvas, painted at glow-modulated
    //   alpha to anchor the frosted glass panels without animated motion.
    val paintAurora = shouldPaintAurora(tok.background, translucent, paintCanvasBackdrop)
    val paintTintBlob = shouldPaintTintBlob(tok.background, translucent, paintCanvasBackdrop)
    // Hoist LocalPalette.current out of drawBehind (DrawScope is not composable).
    val currentPalette = paletteAurora(LocalPalette.current)
    val scaffoldModifier = when {
        paintAurora -> modifier.auroraCanvas(dark, currentPalette)
        paintTintBlob -> modifier.tintBlobCanvas(dark, currentPalette, tok.glow)
        else -> modifier
    }

    Scaffold(
        // CopyPaste-7em1/1a61: pass the active palette's AuroraDef so light palettes
        // render their soft aurora blobs instead of hardcoded Liquid Blue legacy blobs.
        // A-C2: replaced inline ternary with scaffoldModifier (three-way background gate).
        modifier = scaffoldModifier,
        containerColor = if (translucent && tok.background != SkinBackground.FLAT) androidx.compose.ui.graphics.Color.Transparent else c.bg,
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
                    // A-C2: row separator driven by tok.rowTreatment.
                    // CARD (Classic): hairline RowDivider — byte-identical.
                    // LINE (Quiet): same hairline divider — line treatment uses dividers.
                    // INSET (Vapor): gap spacer (tok.rowGap = 3dp) instead of a divider line.
                    deviceRows.forEachIndexed { index, row ->
                        if (index > 0) {
                            if (tok.rowTreatment == SkinRowTreatment.INSET && tok.rowGap > 0.dp) {
                                Spacer(Modifier.height(tok.rowGap))
                            } else {
                                RowDivider()
                            }
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

// ─────────────────────────────────────────────────────────────────────────────
// Own QR section (Deliverable 1)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Pixel side of the QR bitmap generated here — matches [QR_BITMAP_PX] in
 * PairActivity so both screens produce identical-quality codes.
 * CopyPaste-s6cc: raised 512→800 to prevent downscaling blur at 3× density.
 */
private const val DEVICES_QR_BITMAP_PX = 800

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

/**
 * QR countdown urgency threshold. PARITY-SPEC §10 / audit #26: the bar + label
 * switch from accent → warning at ≤20 s remaining (was 15 s, faint→warning).
 */
private const val DEVICES_QR_URGENT_THRESHOLD_SECONDS = 20

/**
 * Generates a QR [Bitmap] for [text] at [sizePx] pixels.
 *
 * CopyPaste-jkbo: delegates to the shared [encodeQrBitmap] in QrUtils.kt,
 * eliminating the former duplication with PairActivity's private copy.
 */
private fun encodeDevicesQrBitmap(text: String, sizePx: Int): Bitmap =
    encodeQrBitmap(text, sizePx)

/**
 * Shows this device's pairing QR at the top of the Devices screen.
 *
 * Privacy model — identical to [PairActivity]:
 *  - QR is blurred ([Modifier.blur] 16 dp) by default; a "Tap to reveal"
 *    overlay guides the user.
 *  - First tap → unblurred (revealed).
 *  - Second tap → regenerates; blur state is left untouched.
 *  - On expiry (2-minute TTL) the QR auto-regenerates; blur state is preserved.
 *
 * Blur persistence (CopyPaste-v5a, android half — mirrors the web fix): the
 * `qrBlurred` flag is INDEPENDENT of QR generation. Regenerating (manual second
 * tap OR the automatic TTL refresh) never flips the blur — only an explicit
 * first tap reveals, and the QR stays revealed across subsequent refreshes. This
 * removes the surprise re-blur / unexpected reveal on auto-refresh.
 *
 * The QR is generated on first composition via [startPairing] (same FFI call
 * as PairActivity). Failures show a muted error label so the rest of the
 * Devices screen still renders.
 *
 * FLAG_SECURE: DevicesActivity sets FLAG_SECURE in onCreate (CopyPaste-92qs), so
 * the revealed QR (and the full fingerprint) cannot be captured to a screenshot or
 * the recents thumbnail. The blur-at-rest remains as defence-in-depth.
 */
@Composable
private fun OwnQrSection(settings: Settings) {
    val c = LocalIdeColors.current
    val scope = rememberCoroutineScope()
    var qr by remember { mutableStateOf<PairingQrResult?>(null) }
    var qrBitmap by remember { mutableStateOf<Bitmap?>(null) }
    var loading by remember { mutableStateOf(false) }
    var errorMsg by remember { mutableStateOf<String?>(null) }
    var remainingSeconds by remember { mutableStateOf(0) }
    // Privacy blur — INDEPENDENT of QR generation (CopyPaste-v5a; mirrors web).
    // Blurred by default; an explicit first tap reveals; regenerating (second
    // tap OR the TTL auto-refresh) leaves this flag untouched so the user's
    // chosen reveal/blur state survives a refresh.
    var qrBlurred by remember { mutableStateOf(true) }

    val expired = qr != null && remainingSeconds <= 0

    // Scan line and progress-bar pulse removed — QR is static; progress bar is static.

    // Generate (or regenerate) the QR.
    //
    // CopyPaste-v5a / CopyPaste-5917.36: blur state is INDEPENDENT of QR generation.
    // generateQr() MUST NOT touch qrBlurred — only an explicit first tap reveals,
    // and the reveal state persists across subsequent token refreshes (both manual
    // second-tap and the automatic 120 s TTL rotation). This matches PairActivity
    // line 437-439 ("The blur is user-owned") and the macOS DevicesView policy.
    fun generateQr() {
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
                // qrBlurred intentionally NOT touched here — see CopyPaste-v5a above.
            } catch (e: Exception) {
                // CopyPaste-7yno / CopyPaste-jwga: log raw detail internally but
                // never store it in user-visible state — set a boolean sentinel
                // instead so the UI can show a sanitized fixed string.
                Log.w("OwnQrSection", "QR generation failed: ${e.javaClass.name}: ${e.message}")
                errorMsg = ErrorMessages.friendlyQrError(e)
            } finally {
                loading = false
            }
        }
    }

    // Countdown ticker — restarts whenever a fresh QR is issued. Auto-regenerates
    // on expiry WITHOUT changing the blur state (CopyPaste-v5a).
    LaunchedEffect(qr) {
        if (qr == null) return@LaunchedEffect
        remainingSeconds = DEVICES_QR_TTL_SECONDS
        while (remainingSeconds > 0) {
            delay(1_000L)
            remainingSeconds -= 1
        }
        generateQr()
    }

    // Generate QR on first composition.
    LaunchedEffect(Unit) {
        if (qr != null || loading) return@LaunchedEffect
        generateQr()
    }

    // CopyPaste-0tb0: counteract the outer column's 16dp horizontal padding so this
    // SectionLabel aligns with the card edge (SectionLabel itself adds start=16.dp,
    // but it's already inside a column with horizontal=16.dp → net 32dp without the
    // offset, vs 16dp for the card). The offset shifts the label 16dp back to the left.
    SectionLabel("Your QR code", modifier = Modifier.offset(x = (-16).dp))

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
                color = c.dim,
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
                            color = c.accent,
                            strokeWidth = 2.dp,
                        )
                    }
                    bmp != null && !expired -> {
                        // Static QR — scan line removed; QR is calm and professional.
                        Box(
                            modifier = Modifier
                                .size(DEVICES_QR_SLOT_DP.dp)
                                .clip(RoundedCornerShape(10.dp))
                                .then(
                                    if (qrBlurred) Modifier.blur(16.dp) else Modifier
                                )
                                .clickable {
                                    // First tap reveals; subsequent taps regenerate
                                    // WITHOUT re-blurring (blur is user-owned, v5a).
                                    if (qrBlurred) {
                                        qrBlurred = false
                                    } else {
                                        generateQr()
                                    }
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            // CopyPaste-sry7: ioco pattern — pad → clip → background so
                            // glass shows through at the slot corners (radius-card 10dp).
                            Box(
                                modifier = Modifier
                                    .size(DEVICES_QR_SLOT_DP.dp)
                                    .padding(DEVICES_QR_PLATE_PADDING_DP.dp)
                                    .clip(RoundedCornerShape(10.dp))
                                    .background(androidx.compose.ui.graphics.Color.White),
                                contentAlignment = Alignment.Center,
                            ) {
                                Image(
                                    bitmap = bmp.asImageBitmap(),
                                    contentDescription = stringResource(R.string.cd_own_qr_blurred),
                                    modifier = Modifier.size(DEVICES_QR_IMAGE_DP.dp),
                                )
                            }
                            // CopyPaste-5917.40: reveal overlay — accent pill matching PairActivity
                            // pattern (was bare Text with c.text, no background). Now uses
                            // accentDim container + RadiusChip shape, accent text colour.
                            if (qrBlurred) {
                                Box(
                                    modifier = Modifier
                                        .size(DEVICES_QR_SLOT_DP.dp)
                                        .background(c.accentDim, RoundedCornerShape(12.dp)),
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
                        // Expired placeholder while auto-regeneration is in flight.
                        Text(
                            text = "Refreshing…",
                            style = MaterialTheme.typography.bodySmall,
                            color = c.dim,
                        )
                    }
                }
            }

            // §10 Countdown / expiry label + drain bar.
            // CopyPaste-h59h: guard on !loading prevents a 1-frame flash of
            // remainingSeconds==0 between LaunchedEffect(qr) restarts when the
            // composable re-enters after the previous token expired on visibility-restore
            // (>105 s hidden). During regeneration the loading spinner is shown instead.
            if (qr != null && !expired && !loading) {
                val urgent = isQrWarning(remainingSeconds)
                Text(
                    text = stringResource(R.string.pair_token_expires_in_seconds, remainingSeconds),
                    style = MaterialTheme.typography.bodySmall,
                    color = if (urgent) c.warning else c.faint,
                )
                // §10 QR countdown drain bar: 2dp track, mute@35%; fill drains over TTL.
                // Static fill (no pulse) — progress-bar pulse removed for calm UI.
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(2.dp)
                        .clip(RoundedCornerShape(999.dp))
                        .background(c.mute.copy(alpha = 0.35f)),
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth(qrCountdownProgress(remainingSeconds, DEVICES_QR_TTL_SECONDS))
                            .height(2.dp)
                            .background(if (urgent) c.warning else c.accent),
                    )
                }
            }

            // CopyPaste-7yno: never show the raw exception message (may contain
            // socket paths or internal detail). errorMsg is set to a pre-sanitized
            // string from ErrorMessages.friendlyQrError(); display it directly.
            errorMsg?.let { sanitizedMsg ->
                Text(
                    text = sanitizedMsg,
                    style = MaterialTheme.typography.bodySmall,
                    color = c.danger,
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
// Grouped inset list rows (PARITY-SPEC §8)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Fixed width of the label column in the two-column metadata table.
 * Sized to fit the longest label ("Local IP" / "Public IP") at 11 sp so
 * values in all three row types (Own, Peer, Discovered) start at the same
 * horizontal position regardless of which row they appear in.
 */
private val META_LABEL_WIDTH: Dp = 72.dp

/**
 * Single 1dp hairline between rows in the grouped inset device list
 * (PARITY-SPEC §4 / §8 — kills the former 0.5dp mix). Inset on the leading edge
 * to read as an Apple grouped-list separator.
 */
@Composable
private fun RowDivider() {
    val c = LocalIdeColors.current
    HorizontalDivider(
        modifier = Modifier.padding(start = 16.dp),
        color = c.divider,
        thickness = 1.dp,
    )
}

// CopyPaste-jkbo: promoted from private to internal so future screens can reuse.
@Composable
internal fun PeerRow(
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
    val c = LocalIdeColors.current
    // PG-37 parity: offline status dot uses danger (red) to match the macOS
    // DeviceCard offline indicator (was c.faint/grey, which diverged).
    val dotColor = if (online) c.success else c.danger
    val chip = transportChipFor(peer)

    // Row content only — the enclosing CopyPasteCard provides the glass surface,
    // 12dp radius, and 1dp hairline border (PARITY-SPEC §8 grouped inset list).
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
                color = c.text,
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.weight(1f, fill = false),
            )
            Text(
                text = if (online) "Online" else "Offline",
                color = dotColor,
                style = MaterialTheme.typography.labelMedium,
            )
            // §7 transport chip: P2P (info) or Cloud (accent).
            TransportChipLabel(chip = chip)
        }

        Spacer(Modifier.height(6.dp))

        // mgkr (NG-3): Verified trust badge — all persisted peers completed SAS
        // confirmation before roster insertion. Surface this explicitly via a
        // green "Verified" chip using success token colours + RadiusChip shape
        // (4 dp — PARITY-SPEC §4 chip radius) so it adapts across skins without
        // hard-coding a value. Parity with the web DeviceCard trust badge.
        Text(
            text = trustLabel(peer),
            color = c.success,
            fontSize = 10.sp,
            letterSpacing = 0.4.sp,
            style = MaterialTheme.typography.labelSmall,
            modifier = Modifier
                .background(c.success.copy(alpha = 0.14f), RadiusChip)
                .border(
                    width = 1.dp,
                    color = c.success.copy(alpha = 0.30f),
                    shape = RadiusChip,
                )
                .padding(horizontal = 6.dp, vertical = 2.dp),
        )

        Spacer(Modifier.height(8.dp))

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
            // PG-39: show peerLocalIp when present, else fall back to the host
            // portion of syncAddr — mirrors macOS DeviceCard.tsx:215
            //   `peer.local_ip ?? extractIp(peer.address)`.
            // syncAddrToIp() strips the port (handles IPv4 and [IPv6]:port).
            val localIpDisplay = peer.peerLocalIp?.takeIf { it.isNotBlank() }
                ?: syncAddrToIp(peer.syncAddr)
            localIpDisplay?.let {
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
            // PG-45: show truncated peer fingerprint so the user can verify the
            // peer's identity inline — mirrors macOS DeviceCard which shows a
            // truncated fingerprint in the MetaGrid. Format: first16…last8.
            // formatPeerFingerprint() is the shared helper at the top of this file.
            peer.fingerprint.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = "Fingerprint", value = formatPeerFingerprint(it))
            }
        }

        // CopyPaste-g4ze: reduce divider gap (vertical 12dp → top 10 / bottom 8) to avoid
        // disproportionate spacing between the metadata table and the action buttons.
        HorizontalDivider(
            modifier = Modifier.padding(top = 10.dp, bottom = 8.dp),
            color = c.divider,
            thickness = 1.dp,
        )

        // ── Actions ─────────────────────────────────────────────────────
        // CopyPaste-jkbo: replaced raw M3 Button/ButtonDefaults with shared
        // CopyPasteButton(DANGER) which applies the styleguide bg=danger@15%,
        // fg=danger recipe automatically (matching web spec §7).
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            CopyPasteButton(
                onClick = onUnpair,
                variant = ButtonVariant.DANGER,
                modifier = Modifier.weight(1f),
            ) {
                Text("Unpair")
            }
            CopyPasteButton(
                onClick = onRevoke,
                variant = ButtonVariant.DANGER,
                modifier = Modifier.weight(1f),
            ) {
                Text("Revoke")
            }
        }
    }
}

@Composable
private fun NoPeerCard(onPair: () -> Unit) {
    val c = LocalIdeColors.current
    CopyPasteCard(accent = c.border) {
        Row(
            modifier = Modifier.padding(16.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Discovery rings icon — concentric ping rings around the network symbol.
            // Mirrors styleguide `networkRing` keyframe: scale .78→1.35, opacity .5→0,
            // 2.7 s × motionScale loop; second ring delayed by 1.1 s × motionScale.
            DiscoveryRingsIcon(size = 52.dp)

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = "No device paired",
                    color = c.dim,
                    style = MaterialTheme.typography.bodyLarge,
                )
                Text(
                    text = "Pair with a Mac running CopyPaste to enable P2P clipboard sync over your local network.",
                    color = c.faint,
                    style = MaterialTheme.typography.bodySmall,
                )
                // CopyPaste-jkbo: replaced raw M3 Button with CopyPasteButton(PRIMARY).
                CopyPasteButton(onClick = onPair, variant = ButtonVariant.PRIMARY) {
                    Text("Pair a device")
                }
            }
        }
    }
}

/**
 * Network icon with two concentric discovery-ping rings animated outward.
 * Mirrors the styleguide `.empty-icon::before/::after` + `networkRing` keyframe:
 *   scale 0.78 → 1.35, opacity 0.5 → 0, ease-out, 2.7 s × motionScale loop.
 * The second ring is delayed by 1.1 s × motionScale to stagger the pulses.
 * Both rings are tinted [accent2] (styleguide `.empty-icon::before` uses accent-2).
 * Gated on system reduced-motion.
 */
@Composable
private fun DiscoveryRingsIcon(size: Dp = 58.dp) {
    val c = LocalIdeColors.current
    // Discovery rings removed — static icon is calmer (no idle loop animation).
    Box(
        modifier = Modifier.size(size),
        contentAlignment = Alignment.Center,
    ) {
        // Icon surface — glass-tinted rounded square with network symbol (text).
        Box(
            modifier = Modifier
                .size(size)
                .clip(RoundedCornerShape(size / 3.5f))
                .background(c.accentDim),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = "⊕",
                color = c.accent,
                fontSize = (size.value * 0.45f).sp,
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Own-device row
// ─────────────────────────────────────────────────────────────────────────────

// CopyPaste-jkbo: promoted from private to internal so future screens can reuse.
@Composable
internal fun OwnDeviceRow(
    identity: P2pIdentity,
    /** Current epoch millis from the 1-second ticker — drives live IP refresh. */
    nowMs: Long,
    /** Public IP from a STUN lookup (CopyPaste-6qq1), null when not yet resolved. */
    ownPublicIp: String? = null,
) {
    // HB-1c: render THIS device's info at parity with the macOS "This Mac" card.
    // ABI 14 sends these same fields to peers (own gather in PairActivity /
    // DevicesActivity startPairing); we surface them locally too. Gathered live —
    // P2pIdentity only carries the id/fingerprint, the rest comes from the
    // platform (Build/BuildConfig) and a LAN-IPv4 enumeration. No synchronous
    // public-IP source on-device, so that row is omitted (matches the bootstrap
    // path, which sends public_ip = None for this device).
    val c = LocalIdeColors.current
    val model = Build.MODEL.orEmpty().ifBlank { "Android" }
    val osVersion = "Android " + Build.VERSION.RELEASE
    val appVersion = BuildConfig.VERSION_NAME

    // Live local IP — re-read every ~5 s (keyed on nowMs / 5000) so a network
    // change (Wi-Fi handoff, VPN connect) is reflected promptly.
    // The bare `remember { lanIpv4Address() }` snapshot was stale on network
    // change because it was only evaluated once at first composition.
    val localIp = remember(nowMs / 5_000L) { lanIpv4Address() }

    // Badge float removed — static badge is calmer and more professional.

    // Row content only — the enclosing CopyPasteCard provides the glass surface
    // (PARITY-SPEC §8 grouped inset list).
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
                color = c.text,
                style = MaterialTheme.typography.titleSmall,
                modifier = Modifier.weight(1f, fill = false),
            )
            Text(
                text = "Online",
                color = c.success,
                style = MaterialTheme.typography.labelMedium,
            )
            // §7 "This Device" accent badge — static (float animation removed).
            // CopyPaste-5917.44: was RoundedCornerShape(4.dp); canonical chip token is RadiusChip (7dp).
            Text(
                text = "This Device",
                color = c.accent,
                fontSize = 10.sp,
                letterSpacing = 0.4.sp,
                style = MaterialTheme.typography.labelSmall,
                modifier = Modifier
                    .background(c.accentDim, RadiusChip)
                    .padding(horizontal = 6.dp, vertical = 2.dp),
            )
        }

        Spacer(Modifier.height(10.dp))

        // Two-column aligned table — same [META_LABEL_WIDTH] as PeerRow.
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            MetaRow(label = "Model", value = model)
            MetaRow(label = "OS", value = osVersion)
            MetaRow(label = "Version", value = appVersion)
            localIp?.let { MetaRow(label = "Local IP", value = it) }
            // CopyPaste-6qq1: show Public IP from async STUN lookup when available.
            ownPublicIp?.let { MetaRow(label = "Public IP", value = it) }
            // CopyPaste-0tb0: show own fingerprint — mirrors macOS ThisDeviceCard.
            // Full fingerprint displayed (no truncation) so the user can verify identity.
            identity.fingerprint.takeIf { it.isNotBlank() }?.let {
                MetaRow(label = "Fingerprint", value = it)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Discovered-peer row (LAN, unpaired)
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
private fun DiscoveredPeerRow(
    peer: DiscoveredPeer,
    busy: Boolean,
    onPair: () -> Unit,
) {
    val c = LocalIdeColors.current
    // v1 peers (no bootstrap port) cannot do SAS pairing → disable Pair.
    val pairable = peer.bport != null
    // CopyPaste-cnmw: show ALL discovered IPs (macOS merges/shows all) instead of
    // only firstOrNull(). When multiple interfaces advertise the peer we join them
    // with ", " so the user can see every reachable address.
    val ips = peer.ipAddrs

    // Row content only — the enclosing CopyPasteCard provides the glass surface
    // (PARITY-SPEC §8 grouped inset list).
    Column {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // Discovery icon with concentric rings — signals "device nearby, tap to pair".
            DiscoveryRingsIcon(size = 40.dp)

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = peer.displayName(),
                    color = c.text,
                    style = MaterialTheme.typography.titleSmall,
                )
                Spacer(Modifier.height(4.dp))
                // CopyPaste-cnmw: show all IPs joined, matching macOS parity.
                // Each IP shown on its own MetaRow so long multi-IP lists wrap cleanly.
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (ips.isNotEmpty()) {
                        MetaRow(
                            label = stringResource(R.string.meta_label_local_ip),
                            value = ips.joinToString(", "),
                        )
                    }
                }
            }
            // CopyPaste-jkbo: replaced raw M3 Button with CopyPasteButton(PRIMARY).
            CopyPasteButton(
                onClick = onPair,
                enabled = pairable && !busy,
                variant = ButtonVariant.PRIMARY,
            ) {
                Text("Pair")
            }
        }
        if (!pairable) {
            Text(
                text = "This device does not support secure pairing.",
                color = c.faint,
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
 * CopyPaste-3vpq: peer metadata card shown inside the SAS dialog while
 * [status.state] == "awaiting_sas". Mirrors the macOS SasPairingModal which
 * displays the peer's model, OS, and IP so the user can verify they are pairing
 * with the right device before comparing the Short Authentication String.
 *
 * Only rows with non-null/non-blank values are rendered — early handshake polls
 * may not have received metadata yet (peerModel==null), so the card degrades
 * gracefully and is invisible when no fields are known.
 */
@Composable
private fun SasPeerMetadataCard(status: PairStatus) {
    val c = LocalIdeColors.current
    // Pre-resolve string resources outside buildList (stringResource is @Composable;
    // it cannot be called inside a non-@Composable lambda like buildList).
    val labelModel = stringResource(R.string.meta_label_model)
    val labelOs = stringResource(R.string.meta_label_os)
    val labelLocalIp = stringResource(R.string.meta_label_local_ip)
    val labelPublicIp = stringResource(R.string.meta_label_public_ip)

    // Collect the non-blank field pairs we have.
    val fields = buildList {
        status.peerModel?.takeIf { it.isNotBlank() }?.let { add(labelModel to it) }
        status.peerOs?.takeIf { it.isNotBlank() }?.let { add(labelOs to it) }
        status.peerLocalIp?.takeIf { it.isNotBlank() }?.let { add(labelLocalIp to it) }
        status.peerPublicIp?.takeIf { it.isNotBlank() }?.let { add(labelPublicIp to it) }
    }
    // Nothing to show yet — the card is silent (not even a placeholder).
    if (fields.isEmpty()) return

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(c.elevated, RoundedCornerShape(8.dp))
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        fields.forEach { (label, value) ->
            MetaRow(label = label, value = value)
        }
    }
}

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
    val c = LocalIdeColors.current
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
                peerDeviceId = null,
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
                        // CopyPaste-3k6m (ABI 17): persist the peer's stable device UUID so
                        // OriginDeviceFilter resolves clipboard item names by UUID.
                        peerDeviceId = st.peerDeviceId,
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
                // CopyPaste-jwga: never surface raw exception detail to users.
                error = ErrorMessages.friendlySasError(e)
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
                                // CopyPaste-3k6m: carry forward peer_device_id.
                                peerDeviceId = status.peerDeviceId,
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
                // CopyPaste-jwga: never surface raw exception detail to users.
                error = ErrorMessages.friendlySasError(e)
            } finally {
                confirmPending = false
            }
        }
    }

    val title = peer.displayName()

    // §8 glass SAS modal (audit #10, §10) — appearance only; pairing logic
    // (handleConfirm/handleClose, status machine) is untouched.
    GlassAlertDialog(
        onDismissRequest = { handleClose() },
        title = { Text("Pair “$title”") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                when {
                    ended -> {
                        Text(
                            "Pairing ended — check the other device.",
                            color = c.dim,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                    status.state == "confirmed" -> {
                        Text(
                            "Paired ✓",
                            color = c.success,
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
                            color = c.danger,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                    status.state == "awaiting_sas" && status.sas != null -> {
                        // CopyPaste-3vpq: peer metadata card — macOS shows model/OS/IP during
                        // awaiting_sas. Rendered before the SAS prompt so the user can verify
                        // they are pairing with the right device before confirming the code.
                        SasPeerMetadataCard(status = status)
                        Text(
                            stringResource(R.string.sas_confirm_prompt),
                            color = c.dim,
                            style = MaterialTheme.typography.bodySmall,
                        )
                        // §10 SAS per-digit cells — styleguide .sas: each digit in its
                        // own 38dp-wide centered mono cell, 28sp/600, letterSpacing 1.1sp
                        // (≈.04em at 28sp), gap 8dp.
                        //
                        // CopyPaste-quux: the SAS code must NOT be copyable to the system
                        // clipboard. Copying it opens a sniff window — any other app that
                        // reads the clipboard during or after pairing gets the active pairing
                        // token. The row is display-only (no clickable, no long-press copy).
                        val sasFull = status.sas ?: ""
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 8.dp),
                        ) {
                            sasFull.forEach { digit ->
                                Box(
                                    contentAlignment = Alignment.Center,
                                    modifier = Modifier.width(38.dp),
                                ) {
                                    Text(
                                        text = digit.toString(),
                                        color = c.text,
                                        fontFamily = MonoFontFamily,
                                        fontSize = 28.sp,
                                        fontWeight = FontWeight.SemiBold,
                                        letterSpacing = 1.1.sp,
                                        textAlign = TextAlign.Center,
                                    )
                                }
                            }
                        }
                    }
                    status.state == "awaiting_sas" -> {
                        // CopyPaste-3vpq: show peer metadata even while waiting for the
                        // peer to accept — same macOS parity, displayed above the spinner.
                        SasPeerMetadataCard(status = status)
                        // Accepted locally; waiting for the peer to also accept.
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            CircularProgressIndicator(modifier = Modifier.size(18.dp))
                            Text(
                                stringResource(R.string.sas_waiting_other),
                                color = c.dim,
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
                                stringResource(R.string.sas_connecting),
                                color = c.dim,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                }
                error?.let { msg ->
                    if (!terminal) {
                        Text(msg, color = c.danger, style = MaterialTheme.typography.labelSmall)
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
                    ) { Text("Doesn't match", color = c.dim) }
                }
                else -> {
                    TextButton(onClick = { handleClose() }) { Text("Cancel", color = c.faint) }
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
 * Online presence indicator: a solid success-green dot with a smooth expanding
 * ping ring when [online] is true and reduced-motion is off.
 *
 * Animation mirrors the styleguide `statusPing` keyframe:
 *   scale 0.45 → 1.8, alpha 0.7 → 0, ease-out, 2.4 s × motionScale loop.
 * Ring is drawn BEHIND the solid dot so the dot stays crisply visible.
 *
 * Gate: animated only when [online] == true and the system "remove animations"
 * scale is not 0 (matches §7 / §8 "Respect prefers-reduced-motion").
 */
@Composable
private fun PulseDot(online: Boolean, modifier: Modifier = Modifier) {
    val c = LocalIdeColors.current
    val tokens = LocalLiquidTokens.current
    val reducedMotion = rememberReducedMotion()
    // PG-37 parity: offline status dot uses danger (red) to match the macOS
    // DeviceCard offline indicator (was c.faint/grey, which diverged).
    val dotColor = if (online) c.success else c.danger
    val animate = shouldPulse(online = online, reducedMotion = reducedMotion)

    // Duration mirrors styleguide 2.4s × motionScale (cinematic = 1.3 → ~3.1 s).
    val pingDurationMs = (2400 * tokens.motionScale).toInt()

    // Always create transition unconditionally (Compose rules — no conditional @Composable).
    // Gate the visible ring via graphicsLayer alpha = 0 when not animating.
    val pulseTransition = rememberInfiniteTransition(label = "pulse")
    // Scale: 0.45 → 1.8 (styleguide statusPing scale(.45) → scale(1.8))
    val pulseScale by pulseTransition.animateFloat(
        initialValue = 0.45f,
        targetValue = 1.8f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = pingDurationMs, easing = EaseOutExpo),
            repeatMode = RepeatMode.Restart,
        ),
        label = "pulseScale",
    )
    // Alpha: 0.7 → 0 (styleguide statusPing opacity .7 → 0)
    val pulseAlpha by pulseTransition.animateFloat(
        initialValue = 0.7f,
        targetValue = 0f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = pingDurationMs, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "pulseAlpha",
    )

    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        // Expanding ring — hidden (alpha=0) when not animating so the composable
        // tree is stable and the InfiniteTransition is never conditionally created.
        Box(
            modifier = Modifier
                .size(10.dp)
                .graphicsLayer {
                    alpha = if (animate) pulseAlpha else 0f
                    scaleX = pulseScale
                    scaleY = pulseScale
                }
                .clip(CircleShape)
                .background(c.success),
        )
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
 * Transport chip pill: 10 sp label in a tinted rounded pill.
 * P2P = info teal; Cloud = accent blue (theme-adaptive via [LocalIdeColors]).
 * Label casing matches web's DevicesView ("P2P" / "Cloud" — task #5: lowercase
 * "Cloud", not all-caps "CLOUD").
 * Defensive: never crashes on absent transport info — callers derive [chip]
 * via [transportChipFor] which is always non-null.
 *
 * Styleguide `badgeFloat`: a 3.4 s ease-in-out infinite Y offset of 0 → -1 dp
 * gives the badge a living, breathing quality without distracting from content.
 */
@Composable
private fun TransportChipLabel(chip: TransportChip) {
    val c = LocalIdeColors.current
    val (text, fg, bg) = when (chip) {
        TransportChip.P2P -> Triple("P2P", c.info, c.infoDim)
        TransportChip.Cloud -> Triple("Cloud", c.accent, c.accentDim)
    }

    // Badge float animation removed — static chip is calmer.
    // CopyPaste-sry7: RadiusChip (7dp) pill + 0.5dp hairline tinted border.
    Text(
        text = text,
        color = fg,
        fontSize = 10.sp,
        letterSpacing = 0.6.sp,
        style = MaterialTheme.typography.labelSmall,
        modifier = Modifier
            .background(bg, RadiusChip)
            .border(0.5.dp, fg.copy(alpha = 0.35f), RadiusChip)
            .padding(horizontal = 6.dp, vertical = 2.dp),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Two-column aligned table row used in device rows.
 *
 * The label column is [META_LABEL_WIDTH] wide (fixed) so all labels in
 * OwnDeviceRow, PeerRow, and DiscoveredPeerRow start at the same horizontal
 * offset. Both text nodes are vertically centred within the row
 * (verticalAlignment = Alignment.CenterVertically) so multi-line values don't
 * cause the label to sit misaligned — fixing the former "Mac" misalignment in
 * the Model row.
 */
// CopyPaste-jkbo: promoted from private to internal so future screens can reuse.
@Composable
internal fun MetaRow(label: String, value: String) {
    val c = LocalIdeColors.current
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = c.dim,
            fontSize = 11.sp,
            modifier = Modifier.width(META_LABEL_WIDTH),
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = MonoFontFamily),
            color = c.text,
            fontSize = 11.sp,
            modifier = Modifier.weight(1f),
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
