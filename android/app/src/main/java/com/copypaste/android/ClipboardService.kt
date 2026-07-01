package com.copypaste.android

import android.app.Notification
import android.app.Service
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import androidx.core.app.ServiceCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * Foreground service for clipboard monitoring.
 *
 * ## Foreground service type (Android 14+)
 * Declared as `specialUse` in the manifest with the required
 * `PROPERTY_SPECIAL_USE_FGS_SUBTYPE` property. We use `specialUse` rather than
 * `dataSync` because clipboard monitoring is not "data sync" per Google Play
 * policy — it is a niche, user-facing clipboard utility that does not fit any
 * of the 12 named FGS types. Play policy explicitly directs developers to use
 * `specialUse` for clipboard managers. The `dataSync` type is for
 * upload/download/backup operations, not for event-driven clipboard capture.
 * At runtime we pass `ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE` via
 * ServiceCompat so API 34+ correctly tracks the type.
 *
 * ## Background clipboard access (Android 10+)
 * `ClipboardManager.getPrimaryClip()` is blocked from any non-foreground,
 * non-IME context on API 29+. This service registers
 * `OnPrimaryClipChangedListener` on the main thread (framework requirement),
 * which *fires* even from background — but `getPrimaryClip()` inside the
 * callback will return null unless the process has a focused window token.
 * [CaptureOverlayController.add] adds a 1×1 invisible TYPE_APPLICATION_OVERLAY
 * window that grants this token, lifting the restriction on Android 10+.
 *
 * Background capture via the logcat+ClipboardFloatingActivity path requires
 * READ_LOGS (adb grant) and SYSTEM_ALERT_WINDOW. See [LogcatCaptureService].
 * When getPrimaryClip() returns null (overlay not yet added), this FGS only
 * captures clips copied while the app is in the foreground.
 *
 * ## Restart on swipe-away ([onTaskRemoved])
 * When the user swipes the app from the recents list, Android calls
 * [onTaskRemoved] before killing the service. We post a JobScheduler-backed
 * delayed restart via WorkManager. We do NOT call startForegroundService from
 * onTaskRemoved because that is not an exempt background-start case on API 31+
 * and would throw ForegroundServiceStartNotAllowedException. WorkManager
 * schedules a one-time expedited job instead, which is the correct mechanism.
 *
 * ## Background sync loop
 * [FgsSyncLoop] runs a 60-second Supabase poll inside this service (faster
 * than the 15-min WorkManager fallback). See [FgsSyncLoop] for the rationale.
 *
 * ## CopyPaste-vp63.32 — composition over inheritance
 * This class is now a THIN Android [Service] lifecycle shell. The crypto-
 * sensitive capture pipeline, notification surface, inbound P2P listener +
 * discovery, capture-overlay token trick, and outbound mutation-drain bridge
 * were extracted VERBATIM into dedicated collaborators wired here:
 *  - [ClipboardCapturePipeline] — capture/encrypt/store/sync-push.
 *  - [ServiceNotifications] — channels, notification builders, today-counter.
 *  - [P2pListenerController] — inbound mTLS listener + mDNS discovery/pollers.
 *  - [CaptureOverlayController] — overlay add/remove + suppress/restore protocol.
 *  - [ServiceMutationBridge] — outbound mutation-queue drain hook.
 * Public companion API (constants + forwarding functions) is kept byte-for-byte
 * name/signature compatible so external callers (MainActivity, HistoryActivity,
 * ShareReceiverActivity, ClipboardFloatingActivity, LogcatCaptureService,
 * PairActivity, CaptureControlReceiver, ServiceRestartWorker, ClipboardRepository)
 * are unaffected by the split.
 */
class ClipboardService : Service() {

    // SupervisorJob: one failing child coroutine (e.g. a bad image decode) does
    // not cancel sibling coroutines — all capture paths remain active.
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    private lateinit var settings: Settings
    private lateinit var repository: ClipboardRepository
    private lateinit var clipboardManager: ClipboardManager
    private lateinit var syncManager: SyncManager
    private lateinit var fgsSyncLoop: FgsSyncLoop
    private lateinit var deviceKeyStore: DeviceKeyStore

    /** CopyPaste-vp63.32: inbound P2P listener + discovery + pollers, extracted. */
    private lateinit var p2pController: P2pListenerController

    /** CopyPaste-vp63.32: capture-overlay token trick, extracted. */
    private lateinit var overlayController: CaptureOverlayController

    /**
     * Supabase Realtime WS client — primary push-receive channel (~1 s latency).
     * Owned by this FGS: started in [onStartCommand], closed in [onDestroy].
     * Null when the WS client cannot be constructed (should not happen in practice).
     */
    private var realtimeClient: SupabaseRealtimeClient? = null

    /**
     * Relay SSE subscription — the THIRD independent receive transport (alongside
     * P2P and Supabase WS). Gated only on a configured relayUrl + sync enabled,
     * NOT on Supabase. Owned by this FGS: started in [onStartCommand], closed in
     * [onDestroy]. See [RelaySubscriptionClient].
     */
    private var relayClient: RelaySubscriptionClient? = null

    // HIGH-7: refresh the notification whenever a UI-side write flips a flag
    // the service cares about (capture pause, sync toggle). Retained as a
    // field so SharedPreferences' weak reference does not collect it.
    private val prefsListener = SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
        when (key) {
            "capture_enabled" -> ServiceNotifications.refreshNotification(this)
            // sync_enabled, relay_url etc. are read fresh on each capture
            // so no explicit re-read is needed here.
            else -> Unit
        }
    }

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        val clip: ClipData? = clipboardManager.primaryClip
        if (clip == null) {
            return@OnPrimaryClipChangedListener
        }

        // CopyPaste-x8a8: resolve the foreground package so dispatchClipData can
        // check it against Settings.excludedAppBundleIds. Uses
        // ActivityManager.getRunningAppProcesses() filtered to IMPORTANCE_FOREGROUND —
        // the only process-level API available to third-party apps on API 26+ without
        // special permissions. The pkgList[0] of the foreground process is the app
        // that currently has the clipboard focus. May be null when the AM list is
        // empty or unavailable; dispatchClipData skips the exclusion check in that case.
        val sourcePackage: String? = ClipboardCapturePipeline.resolveSourcePackage(this@ClipboardService)

        // BUG 1 fix: delegate to the shared MIME-dispatch helper so the
        // foreground-service path and the background-overlay path are identical.
        // CopyPaste-mip2: pass the P2P wake signal so a fresh capture immediately
        // triggers an opportunistic dial to the LAN peer.
        ClipboardCapturePipeline.dispatchClipData(
            clip, this@ClipboardService, settings, repository, syncManager, scope,
            sourcePackage,
            onStored = { fgsSyncLoop.signalP2pWake() },
        )
    }

    override fun onCreate() {
        super.onCreate()
        settings = Settings(this)
        repository = ClipboardRepository(this)

        val relayHttp = RelayClient(settings.relayUrl)
        // CopyPaste-crh3.102: seed with the persisted relay token (re-enabled relay
        // upload). pushToRelay → ensureRelayToken refreshes it from Settings.relayToken
        // on a miss / 401, so a stale or empty seed is transparently re-registered.
        syncManager = SyncManager(relayHttp, settings.deviceId, token = settings.relayToken, settings = settings)
        // CopyPaste-3ox2: bind the FGS scope so thumbnail generation tasks in
        // SyncManager are tied to the service lifecycle and cancelled on destroy.
        syncManager.bindScope(scope)

        // P1.2/P1.4: Supabase Realtime WS client — constructed here so it can be
        // passed to FgsSyncLoop as the wsConnected gate.
        realtimeClient = SupabaseRealtimeClient(
            settings = settings,
            syncManager = syncManager,
            repository = repository,
            scope = scope,
            onSyncedTextClip = { text -> applyTextToClipboard(text) },
        )
        deviceKeyStore = DeviceKeyStore(this)
        fgsSyncLoop = FgsSyncLoop(
            settings = settings,
            repository = repository,
            syncManager = syncManager,
            deviceKeyStore = deviceKeyStore,
            wsClient = realtimeClient,
            onSyncedTextClip = { text -> applyTextToClipboard(text) },
            // CopyPaste-yaip: supply application context so dialPairedPeer can read
            // the OutboundMutationQueue and include pin/reorder/delete mutations in
            // the P2P outbound set even when they have an old wallTimeMs.
            context = applicationContext,
        )

        // Relay SSE subscription — the third independent receive transport.
        // Reuses the same syncManager (relay decrypt + LWW) and FGS scope.
        relayClient = RelaySubscriptionClient(settings, syncManager, repository, scope)

        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        ServiceNotifications.ensureChannel(this)
        settings.observe(prefsListener)

        // CopyPaste-vp63.32: construct + register the capture-overlay controller
        // so the static suppress/restore helpers can reach it via a weak reference
        // (parity with the original instance = WeakReference(this) pattern).
        overlayController = CaptureOverlayController(this)
        CaptureOverlayController.register(overlayController)

        // CopyPaste-vp63.32: construct the P2P listener controller. applicationContext
        // is passed (not `this`) since it is only used for notification calls that
        // must not outlive the service.
        p2pController = P2pListenerController(
            context = applicationContext,
            scope = scope,
            settings = settings,
            deviceKeyStore = deviceKeyStore,
            repository = repository,
            fgsSyncLoop = fgsSyncLoop,
        )

        // CopyPaste-0qpn: register the mutation drain hook so ClipboardViewModel
        // can trigger drainOutboundMutationQueue via requestMutationQueueDrain.
        // Captures references by value (no lambda leaks after onDestroy clears it).
        val drainSyncManager = syncManager
        val drainRepo = repository
        val drainContext: android.content.Context = applicationContext
        ServiceMutationBridge.setHook {
            drainSyncManager.drainOutboundMutationQueue(drainContext, drainRepo)
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // HIGH-6: API 34+ throws ForegroundServiceStartNotAllowedException when
        // the app is in a state that disallows promotion (e.g. started from a
        // disallowed background context). API 31+ can also throw
        // SecurityException under stricter battery-optimisation profiles.
        // A failure here is non-fatal — log it, stop the service, and let
        // the UI's in-activity clipListener (MainActivity) keep working while
        // the user is foregrounded. Crashing here would kill the JVM and
        // break the app immediately on devices with strict policies.
        try {
            // ServiceCompat.startForeground correctly passes the type constant
            // on API 29+ (required on API 34+) while remaining compatible with
            // older APIs. FOREGROUND_SERVICE_TYPE_SPECIAL_USE = 0x40000000.
            ServiceCompat.startForeground(
                this,
                ServiceNotifications.NOTIFICATION_ID,
                ServiceNotifications.buildNotification(this),
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    android.content.pm.ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE
                } else {
                    0
                }
            )
        } catch (e: Exception) {
            Log.w(
                TAG,
                "startForeground rejected (${e.javaClass.simpleName}: ${e.message}) — stopping service"
            )
            stopSelf()
            return START_NOT_STICKY
        }

        // Listener must be registered on the main thread (framework requirement).
        // overlayController.add() is also called here: the overlay must exist before
        // the first clipboard callback fires so that getPrimaryClip() sees the token.
        Handler(Looper.getMainLooper()).post {
            overlayController.add()
            monitorClipboard()
        }

        // P1.4: start Supabase Realtime WS push channel (~1 s latency).
        // The WS client owns its own reconnect loop inside `scope`.
        realtimeClient?.start()

        // Start the relay SSE subscription (3rd transport). Owns its own reconnect
        // loop inside `scope`; no-ops while relayUrl is unconfigured.
        relayClient?.start()

        // Start the catch-up poll loop (WS-aware intervals: 120s connected / 60s down).
        fgsSyncLoop.start(scope)

        // CopyPaste-yaip (startup drain): drain the outbound mutation queue on every
        // service (re)start. Without this, mutations queued while the service was dead
        // (e.g. process killed, device restarted, or service swipe-away) sit in
        // SharedPreferences indefinitely — they only drain on the NEXT UI mutation
        // (which calls requestMutationQueueDrain via ClipboardViewModel.onMutationSync).
        //
        // The drain is fire-and-forget on the IO scope: it must not block startForeground
        // (which would ANR) and must not hold a lock across the FGS start sequence.
        // The queue is durable; if the drain fails the records stay for the next tick.
        val startupDrainContext = applicationContext
        val startupDrainManager = syncManager
        val startupDrainRepo = repository
        scope.launch(Dispatchers.IO) {
            try {
                val n = startupDrainManager.drainOutboundMutationQueue(startupDrainContext, startupDrainRepo)
                if (n > 0) {
                    Log.i(TAG, "Startup drain: flushed $n queued mutation(s) accumulated while service was dead")
                }
            } catch (e: Exception) {
                Log.w(TAG, "Startup drain: drainOutboundMutationQueue failed: ${e.message}")
            }
        }

        // Inbound mTLS P2P listener (macOS→Android direction). Gated on the same
        // toggles as the dialer so the user's P2P switch governs BOTH directions.
        // Idempotent across sticky restarts: startInboundP2pListener is a no-op while a
        // listener is already running.
        if (settings.syncEnabled && settings.p2pSyncEnabled) {
            p2pController.startInboundP2pListener()
            // HB-2: host mDNS discovery (advert + standing SAS-pairing responder)
            // in the always-on FGS, NOT on the Devices screen. The screen-scoped
            // version died the moment Devices closed, so a Mac→Android pair hit
            // "Connection refused". Started AFTER the listener so activeListenerPort
            // is known and advertised as the peer's sync port.
            //
            // CopyPaste-plgt: gate discovery on lanVisibility so a user who disabled
            // LAN visibility ("Visible on LAN" toggle off) is not advertised over mDNS.
            // p2pSyncEnabled=true is required for P2P dial-in; lanVisibility=true is
            // separately required to advertise this device so peers can discover it.
            if (settings.lanVisibility) {
                p2pController.startFgsDiscovery()
            } else {
                Log.i(TAG, "LAN visibility disabled — skipping mDNS discovery advertisement")
            }
        }

        // Deliverable 1: poll for incoming (responder-role) pairing requests so
        // the user is notified even when DevicesActivity is not open.
        p2pController.startPairResponderPoller()

        // CopyPaste-mip2: watch for mDNS-discovered peers; signals an opportunistic
        // P2P dial when a new peer appears on the LAN.  Gated on P2P being enabled
        // (pointless to watch if we never dial) and the native library being loaded.
        if (settings.syncEnabled && settings.p2pSyncEnabled) {
            p2pController.startMdnsPeerWatcher()
        }

        return START_STICKY
    }

    /**
     * Called when the user swipes this app from the Recents list.
     *
     * We schedule a one-time expedited WorkManager request to restart the service.
     * We do NOT call startForegroundService here directly because [onTaskRemoved]
     * runs in a background-disallowed context on API 31+ and would throw
     * [android.app.ForegroundServiceStartNotAllowedException].
     *
     * WorkManager's expedited jobs bypass most battery-optimisation restrictions
     * and will fire as soon as the system allows it (typically within a few seconds).
     */
    override fun onTaskRemoved(rootIntent: Intent?) {
        super.onTaskRemoved(rootIntent)
        Log.i(TAG, "onTaskRemoved — scheduling WorkManager restart")
        ServiceRestartWorker.scheduleOnce(applicationContext)
    }

    /**
     * Register the [ClipboardManager.OnPrimaryClipChangedListener].
     * Must be called from the main thread.
     */
    private fun monitorClipboard() {
        clipboardManager.addPrimaryClipChangedListener(clipListener)
        Log.d(TAG, "Clipboard listener registered (API ${Build.VERSION.SDK_INT})")
    }

    /**
     * Encrypt [text] and persist via [ClipboardRepository].
     * Falls back to local AES when the UniFFI .so is unavailable.
     * Skips storage for content flagged as sensitive.
     * Honours [Settings.captureEnabled] — toggled by the notification's
     * Pause/Resume action.
     */
    private suspend fun handleClipboardChange(text: String) {
        ClipboardCapturePipeline.captureClip(this, text, settings, repository, syncManager)
    }

    /**
     * Write [text] to the system clipboard as the result of an inbound sync.
     *
     * Called ONCE per catch-up drain or P2P batch with only the NEWEST text
     * clip — never called per-item during a bulk sync, so the clipboard is
     * never spammed and the capture loop is not re-triggered for intermediate
     * items.
     *
     * Uses [ClipboardRepository.expectClip] to register the content-hash so
     * that the capture listeners ([ClipboardService] / [ClipboardAccessibilityService])
     * recognise the upcoming setPrimaryClip as an internal write and skip it —
     * preventing a capture → re-push → re-sync round-trip.
     */
    private fun applyTextToClipboard(text: String) {
        ClipboardRepository.expectClip(text)
        val clip = ClipData.newPlainText("CopyPaste sync", text)
        clipboardManager.setPrimaryClip(clip)
        Log.d(TAG, "Auto-applied newest synced text clip (${text.length} chars)")
    }

    override fun onDestroy() {
        // CopyPaste-44rq.1: clear the overlay-controller registration so suppress/
        // restore helpers know the service is no longer running and do not attempt
        // to call methods on a destroyed service.
        CaptureOverlayController.clear()
        // Stop the inbound listener (cancels its drain job + releases the bound
        // port) BEFORE scope.cancel() so the native accept loop is torn down
        // cleanly rather than left dangling on an orphaned coroutine.
        p2pController.stopInboundP2pListener()
        fgsSyncLoop.stop()
        // P1.4: close the WS channel gracefully (sends phx_leave) before the
        // scope is cancelled — avoids an abrupt TCP close that Supabase would
        // count against the connection quota.
        realtimeClient?.close()
        // Stop the relay SSE subscription before the scope is cancelled.
        relayClient?.close()
        // Stop the responder poller before the scope is cancelled.
        p2pController.cancelPairResponderPoller()
        // CopyPaste-mip2: stop the mDNS peer watcher before the scope is cancelled.
        p2pController.cancelMdnsPeerWatcher()
        clipboardManager.removePrimaryClipChangedListener(clipListener)
        settings.stopObserving(prefsListener)
        overlayController.remove()
        // CopyPaste-0qpn: clear the mutation drain hook so it does not hold a
        // reference to a destroyed service's scope/resources.
        ServiceMutationBridge.clearHook()
        scope.cancel()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        private const val TAG = "ClipboardService"

        // ── CopyPaste-vp63.32: forwarding constants (kept for call-site stability) ──

        /** @see ServiceNotifications.NOTIFICATION_ID */
        const val NOTIFICATION_ID = ServiceNotifications.NOTIFICATION_ID

        /** @see ServiceNotifications.CHANNEL_ID */
        const val CHANNEL_ID = ServiceNotifications.CHANNEL_ID

        /** @see ServiceNotifications.CHANNEL_PAIR_REQUEST */
        const val CHANNEL_PAIR_REQUEST = ServiceNotifications.CHANNEL_PAIR_REQUEST

        /** @see ServiceNotifications.NOTIF_ID_PAIR_REQUEST */
        const val NOTIF_ID_PAIR_REQUEST = ServiceNotifications.NOTIF_ID_PAIR_REQUEST

        /** @see ServiceNotifications.CHANNEL_COPY_EVENT */
        const val CHANNEL_COPY_EVENT = ServiceNotifications.CHANNEL_COPY_EVENT

        /**
         * Port the inbound mTLS P2P listener is currently bound to (OS-assigned),
         * or 0 when no listener is running. @see [P2pListenerController.activeListenerPort].
         * [PairActivity] reads this to advertise `"<lan-ip>:<port>"` to a peer at pair time.
         */
        val activeListenerPort: Int
            get() = P2pListenerController.activeListenerPort

        // ── CopyPaste-vp63.32: forwarding stubs to ClipboardCapturePipeline ──────
        // Kept so MainActivity / ShareReceiverActivity / HistoryActivity /
        // ClipboardFloatingActivity / LogcatCaptureService call sites are unaffected.

        /** @see ClipboardCapturePipeline.captureClip */
        suspend fun captureClip(
            context: Context,
            text: String,
            settings: Settings,
            repository: ClipboardRepository,
            syncManager: SyncManager,
            sourceApp: String? = null,
            onStored: (() -> Unit)? = null,
        ) = ClipboardCapturePipeline.captureClip(context, text, settings, repository, syncManager, sourceApp, onStored)

        /** @see ClipboardCapturePipeline.captureImageClip */
        suspend fun captureImageClip(
            context: Context,
            uri: android.net.Uri,
            mimeType: String,
            settings: Settings,
            repository: ClipboardRepository,
            syncManager: SyncManager,
            onStored: (() -> Unit)? = null,
        ) = ClipboardCapturePipeline.captureImageClip(context, uri, mimeType, settings, repository, syncManager, onStored)

        /** @see ClipboardCapturePipeline.captureFileClip */
        suspend fun captureFileClip(
            context: Context,
            uri: android.net.Uri,
            mimeType: String,
            settings: Settings,
            repository: ClipboardRepository,
            syncManager: SyncManager? = null,
            onStored: (() -> Unit)? = null,
        ) = ClipboardCapturePipeline.captureFileClip(context, uri, mimeType, settings, repository, syncManager, onStored)

        /** @see ClipboardCapturePipeline.resolveSourcePackage */
        fun resolveSourcePackage(context: Context): String? =
            ClipboardCapturePipeline.resolveSourcePackage(context)

        // ── CopyPaste-vp63.32: forwarding stubs to CaptureOverlayController ──────

        /**
         * @see CaptureOverlayController.suppressCaptureOverlay
         *
         * [context] is unused (kept for call-site compatibility — the original
         * implementation also ignored it, delegating entirely to the live
         * service instance via a weak reference).
         */
        fun suppressCaptureOverlay(context: Context) = CaptureOverlayController.suppressCaptureOverlay()

        /** @see CaptureOverlayController.restoreCaptureOverlay */
        fun restoreCaptureOverlay() = CaptureOverlayController.restoreCaptureOverlay()

        // ── CopyPaste-vp63.32: forwarding stubs to ServiceNotifications ──────────

        /** @see ServiceNotifications.postCopyNotification */
        fun postCopyNotification(context: Context) = ServiceNotifications.postCopyNotification(context)

        /** @see ServiceNotifications.playCopySound */
        fun playCopySound(context: Context) = ServiceNotifications.playCopySound(context)

        /** @see ServiceNotifications.ensureChannel */
        fun ensureChannel(context: Context) = ServiceNotifications.ensureChannel(context)

        /** @see ServiceNotifications.postIncomingPairNotification */
        fun postIncomingPairNotification(context: Context, peerName: String) =
            ServiceNotifications.postIncomingPairNotification(context, peerName)

        /** @see ServiceNotifications.refreshNotification */
        fun refreshNotification(context: Context) = ServiceNotifications.refreshNotification(context)

        /** @see ServiceNotifications.onItemsDeleted */
        fun onItemsDeleted(context: Context, count: Int) = ServiceNotifications.onItemsDeleted(context, count)

        /** @see ServiceNotifications.buildNotification */
        fun buildNotification(context: Context): Notification = ServiceNotifications.buildNotification(context)

        // ── CopyPaste-vp63.32: forwarding stub to ServiceMutationBridge ──────────

        /** @see ServiceMutationBridge.requestMutationQueueDrain */
        fun requestMutationQueueDrain() = ServiceMutationBridge.requestMutationQueueDrain()

        // ── CopyPaste-j2vf: port-poll pure helpers ───────────────────────────────
        //
        // Extracted to FgsDiscoveryPortPoll.kt (CopyPaste-vp63.32) — PURE,
        // JVM-testable, no Android runtime. Forwarding stubs kept below so
        // FgsDiscoveryPortPollTest and any other caller of
        // ClipboardService.portPollNextBackoffMs/.shouldAdvertisePort/the
        // PORT_POLL_* constants are unaffected.

        /** Maximum total wait for the inbound listener to bind (safety timeout). */
        internal const val PORT_POLL_TIMEOUT_MS = FgsDiscoveryPortPoll.PORT_POLL_TIMEOUT_MS

        /** Initial backoff (ms) between port-poll retries. */
        internal const val PORT_POLL_INITIAL_BACKOFF_MS = FgsDiscoveryPortPoll.PORT_POLL_INITIAL_BACKOFF_MS

        /** Maximum backoff (ms) between port-poll retries. */
        internal const val PORT_POLL_MAX_BACKOFF_MS = FgsDiscoveryPortPoll.PORT_POLL_MAX_BACKOFF_MS

        /**
         * Compute the next exponential backoff delay (capped at [maxMs]).
         * Delegates to [FgsDiscoveryPortPoll.portPollNextBackoffMs] (CopyPaste-vp63.32).
         */
        internal fun portPollNextBackoffMs(currentMs: Long, maxMs: Long): Long =
            FgsDiscoveryPortPoll.portPollNextBackoffMs(currentMs, maxMs)

        /**
         * True when [port] is non-zero and it is safe to advertise over mDNS.
         * Delegates to [FgsDiscoveryPortPoll.shouldAdvertisePort] (CopyPaste-vp63.32).
         */
        internal fun shouldAdvertisePort(port: Int): Boolean =
            FgsDiscoveryPortPoll.shouldAdvertisePort(port)
    }
}
