package com.copypaste.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.PixelFormat
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import android.view.SoundEffectConstants
import android.view.View
import android.view.WindowManager
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import android.provider.OpenableColumns
import java.io.ByteArrayOutputStream
import java.util.Calendar

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
 * non-IME, non-AccessibilityService context on API 29+. This service registers
 * `OnPrimaryClipChangedListener` on the main thread (framework requirement),
 * which *fires* even from background — but `getPrimaryClip()` inside the
 * callback will return null unless the process also has an enabled
 * AccessibilityService. [ClipboardAccessibilityService] provides that binding;
 * since both services run in the same process, enabling the a11y service makes
 * getPrimaryClip() return non-null here too.
 *
 * When getPrimaryClip() returns null on API 29+ (a11y service not enabled),
 * [clipListener] issues a one-time actionable notification via
 * [maybeNotifyA11yRequired] — shown at most once per install to avoid spam —
 * so the user knows background capture is silently inactive and can fix it.
 * Without the a11y service, this FGS only captures clips copied while the
 * app is in the foreground (via this listener) or via [MainActivity]'s own
 * ClipboardManager listener; it does NOT capture clips from other apps in
 * background on API 29+. There is no workaround for this platform restriction
 * without an enabled AccessibilityService or the default IME role.
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

    /**
     * Inbound mTLS P2P listener handle (macOS→Android direction). Bound in
     * [onStartCommand] when `syncEnabled && p2pSyncEnabled`, drained by
     * [p2pListenerJob], released in [onDestroy]. Null while not running.
     *
     * The companion [activeListenerPort] mirrors [P2pListenerHandleInfo.actualPort]
     * so [PairActivity] can advertise this device's dialable address at pair time.
     */
    private var p2pListener: P2pListenerHandleInfo? = null

    /** Coroutine draining [pollP2pListener] on the dial cadence. Cancelled in [onDestroy]. */
    private var p2pListenerJob: kotlinx.coroutines.Job? = null

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

    /**
     * The 1×1 px invisible overlay view that gives this process a WindowManager
     * token, lifting the Android 10+ clipboard restriction so
     * getPrimaryClip() returns non-null from background.
     *
     * Non-null only when the overlay has been successfully added.
     * Guarded by [Settings.canDrawOverlays] before the add call.
     */
    private var captureOverlayView: View? = null

    // HIGH-7: refresh the notification whenever a UI-side write flips a flag
    // the service cares about (capture pause, sync toggle). Retained as a
    // field so SharedPreferences' weak reference does not collect it.
    private val prefsListener = SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
        when (key) {
            "capture_enabled" -> refreshNotification(this)
            // sync_enabled, relay_url etc. are read fresh on each capture
            // so no explicit re-read is needed here.
            else -> Unit
        }
    }

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        val clip = clipboardManager.primaryClip
        if (clip == null) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                if (!ClipboardAccessibilityService.isEnabled(this@ClipboardService)) {
                    maybeNotifyA11yRequired(this@ClipboardService)
                }
            }
            return@OnPrimaryClipChangedListener
        }

        // Detect image MIME before falling back to text. Check all MIME types on
        // the ClipDescription; the first image/* type wins.
        val imageMime = (0 until clip.description.mimeTypeCount)
            .map { clip.description.getMimeType(it) }
            .firstOrNull { it.startsWith("image/") }

        if (imageMime != null) {
            val uri = clip.getItemAt(0)?.uri
            if (uri != null) {
                scope.launch { captureImageClip(this@ClipboardService, uri, imageMime, settings, repository, syncManager) }
            } else {
                Log.w(TAG, "Image clip has no URI — skipping")
            }
            return@OnPrimaryClipChangedListener
        }

        // File branch: a URI whose MIME is not text/* and not image/* is a real
        // file (PDF, ZIP, DOCX, etc.). Capture bytes + metadata and store as a
        // "file" content-type item so it can sync to macOS via P2P.
        val itemUri = clip.getItemAt(0)?.uri
        if (itemUri != null) {
            val mimeTypes = (0 until clip.description.mimeTypeCount)
                .map { clip.description.getMimeType(it) }
            val fileMime = mimeTypes.firstOrNull { mime ->
                mime != null && !mime.startsWith("text/") && !mime.startsWith("image/")
            }
            if (fileMime != null) {
                scope.launch {
                    captureFileClip(this@ClipboardService, itemUri, fileMime, settings, repository, syncManager)
                }
                return@OnPrimaryClipChangedListener
            }
        }

        val text = clip.getItemAt(0)?.text?.toString()
            ?: return@OnPrimaryClipChangedListener

        scope.launch { handleClipboardChange(text) }
    }

    override fun onCreate() {
        super.onCreate()
        settings = Settings(this)
        repository = ClipboardRepository(this)

        val relayHttp = RelayClient(settings.relayUrl)
        syncManager = SyncManager(relayHttp, settings.deviceId, token = "", settings = settings)

        // P1.2/P1.4: Supabase Realtime WS client — constructed here so it can be
        // passed to FgsSyncLoop as the wsConnected gate.
        realtimeClient = SupabaseRealtimeClient(settings, syncManager, repository, scope)
        deviceKeyStore = DeviceKeyStore(this)
        fgsSyncLoop = FgsSyncLoop(settings, repository, syncManager, deviceKeyStore, realtimeClient)

        // Relay SSE subscription — the third independent receive transport.
        // Reuses the same syncManager (relay decrypt + LWW) and FGS scope.
        relayClient = RelaySubscriptionClient(settings, syncManager, repository, scope)

        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        ensureChannel(this)
        settings.observe(prefsListener)
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
                NOTIFICATION_ID,
                buildNotification(this),
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
        // addCaptureOverlay is also called here: the overlay must exist before the
        // first clipboard callback fires so that getPrimaryClip() sees the token.
        Handler(Looper.getMainLooper()).post {
            addCaptureOverlay()
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

        // Inbound mTLS P2P listener (macOS→Android direction). Gated on the same
        // toggles as the dialer so the user's P2P switch governs BOTH directions.
        // Idempotent across sticky restarts: startP2pListener is a no-op while a
        // listener is already running.
        if (settings.syncEnabled && settings.p2pSyncEnabled) {
            startInboundP2pListener()
            // HB-2: host mDNS discovery (advert + standing SAS-pairing responder)
            // in the always-on FGS, NOT on the Devices screen. The screen-scoped
            // version died the moment Devices closed, so a Mac→Android pair hit
            // "Connection refused". Started AFTER the listener so activeListenerPort
            // is known and advertised as the peer's sync port.
            startFgsDiscovery()
        }

        return START_STICKY
    }

    /**
     * HB-2: start LAN discovery for the lifetime of the foreground service.
     *
     * Advertises this device over mDNS (sync port = the live inbound listener
     * port; bootstrap port = the fixed [SAS_BPORT]) and runs the standing
     * SAS-pairing responder so a macOS peer can dial back to pair AT ANY TIME —
     * not only while the Devices screen is open. Uses the persisted device cert
     * (peek, else generate). Idempotent on the native side (a second start while
     * already running is a no-op). All failures are logged and non-fatal: the FGS
     * must never crash because discovery could not start.
     */
    private fun startFgsDiscovery() {
        scope.launch {
            try {
                val cert = withContext(Dispatchers.IO) {
                    deviceKeyStore.peek() ?: deviceKeyStore.getOrCreate()
                }
                val syncPort = activeListenerPort.coerceAtLeast(0)
                withContext(Dispatchers.IO) {
                    startDiscovery(
                        deviceId = cert.deviceId,
                        deviceName = Build.MODEL ?: "Android",
                        syncPort = syncPort,
                        bport = SAS_BPORT,
                        certDer = cert.certDer,
                        keyDer = cert.keyDer,
                        // HB-1a (ABI 14): own metadata for the standing responder.
                        deviceModel = Build.MODEL ?: "Android",
                        osVersion = "Android " + Build.VERSION.RELEASE,
                        appVersion = BuildConfig.VERSION_NAME,
                        localIp = lanIpv4Address(),
                    )
                }
                Log.i(TAG, "FGS discovery started (syncPort=$syncPort, bport=$SAS_BPORT)")
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Log.w(TAG, "FGS discovery failed to start (${e.javaClass.simpleName}: ${e.message})")
            }
        }
    }

    /**
     * Bind the inbound mTLS P2P listener and launch a coroutine that drains it on
     * the dial cadence, storing each received item via the SAME store-mapping the
     * dialer uses ([FgsSyncLoop.storeSyncedItem]) — LWW dedup on item_id makes a
     * re-receipt (already delivered by the dialer or a previous tick) a no-op.
     *
     * Idempotent: a no-op while [p2pListener] is already bound (sticky restart).
     * All failures are logged and non-fatal — a listener that cannot bind must
     * never crash the foreground service; the Android→macOS dialer still runs.
     */
    private fun startInboundP2pListener() {
        if (p2pListener != null) return // already running — idempotent

        // mTLS identity is mandatory; if pairing never generated one there is
        // nothing to listen with (a peer could not authenticate us anyway).
        val cert = deviceKeyStore.peek() ?: run {
            Log.i(TAG, "P2P listener not started: no device cert (never paired?)")
            return
        }

        val key = settings.encryptionKey
        val peers = settings.pairedPeers
        val allowed = peers.map { it.fingerprint }
        val revoked = try {
            listRevokedFingerprints(settings.dbPath, key)
        } catch (e: Exception) {
            Log.w(TAG, "P2P listener: listRevokedFingerprints failed, using empty denylist: ${e.message}")
            emptyList()
        }
        val sessionKeys = peers.map {
            PeerSessionKeyInfo(it.fingerprint, settings.sessionKeyFor(it.fingerprint))
        }
        val localItems = runBlocking {
            repository.localItemsForSync(key, limit = FgsSyncLoop.P2P_LOCAL_ITEM_LIMIT)
        }

        val handle = try {
            startP2pListener(
                listenPort = 0, // OS-assigned free port; read back from actualPort
                certDer = cert.certDer,
                keyDer = cert.keyDer,
                allowedFingerprints = allowed,
                revokedFingerprints = revoked,
                sessionKeys = sessionKeys,
                localItems = localItems,
                deviceId = settings.deviceId,
            )
        } catch (e: Exception) {
            Log.w(TAG, "P2P listener failed to start (${e.javaClass.simpleName}: ${e.message}) — macOS→Android dial-in disabled this session")
            return
        }

        p2pListener = handle
        activeListenerPort = handle.actualPort
        Log.i(TAG, "P2P listener bound on port ${handle.actualPort} (id=${handle.listenerId})")

        p2pListenerJob = scope.launch {
            val listenerId = handle.listenerId
            while (isActive) {
                // Refresh the roster/denylist/session-keys each tick so a pairing
                // change or revocation is honoured without restarting the listener.
                try {
                    val freshPeers = settings.pairedPeers
                    val freshRevoked = try {
                        listRevokedFingerprints(settings.dbPath, settings.encryptionKey)
                    } catch (e: Exception) {
                        Log.w(TAG, "P2P listener: denylist refresh failed: ${e.message}")
                        emptyList()
                    }
                    updateP2pListenerPeers(
                        listenerId = listenerId,
                        allowed = freshPeers.map { it.fingerprint },
                        revoked = freshRevoked,
                        sessionKeys = freshPeers.map {
                            PeerSessionKeyInfo(it.fingerprint, settings.sessionKeyFor(it.fingerprint))
                        },
                    )
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    Log.w(TAG, "P2P listener: updateP2pListenerPeers failed: ${e.message}")
                }

                // Drain received items; store each via the shared mapping. Per-item
                // try/catch so one malformed item does not stall the rest.
                try {
                    val items = pollP2pListener(listenerId)
                    var stored = 0
                    for (item in items) {
                        try {
                            if (fgsSyncLoop.storeSyncedItem(item)) stored += 1
                        } catch (e: CancellationException) {
                            throw e
                        } catch (e: Exception) {
                            Log.w(TAG, "P2P listener: failed to store item ${item.itemId.take(8)}: ${e.message}")
                        }
                    }
                    if (stored > 0) {
                        Log.i(TAG, "P2P listener: stored $stored inbound item(s)")
                    }
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    Log.w(TAG, "P2P listener: poll failed: ${e.message}")
                }

                delay(FgsSyncLoop.P2P_DIAL_INTERVAL_MS)
            }
        }
    }

    /**
     * Stop the inbound listener and cancel its drain coroutine. Idempotent and
     * safe to call when the listener was never started. Errors are logged, not
     * thrown — [onDestroy] must complete regardless.
     */
    private fun stopInboundP2pListener() {
        // HB-2: tear down LAN discovery (mDNS advert + standing SAS responder)
        // alongside the inbound listener. stopDiscovery() is idempotent and
        // tolerates a stop without a completed start. Called here so both the
        // P2P-toggle-off path and onDestroy stop advertising.
        try {
            stopDiscovery()
        } catch (e: Exception) {
            Log.w(TAG, "FGS discovery: stop failed (${e.javaClass.simpleName}: ${e.message})")
        }
        p2pListenerJob?.cancel()
        p2pListenerJob = null
        val handle = p2pListener ?: return
        p2pListener = null
        activeListenerPort = 0
        try {
            stopP2pListener(handle.listenerId)
            Log.i(TAG, "P2P listener stopped (id=${handle.listenerId})")
        } catch (e: Exception) {
            Log.w(TAG, "P2P listener: stop failed (${e.javaClass.simpleName}: ${e.message})")
        }
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
     * Register the [OnPrimaryClipChangedListener].
     * Must be called from the main thread.
     */
    private fun monitorClipboard() {
        clipboardManager.addPrimaryClipChangedListener(clipListener)
        Log.d(TAG, "Clipboard listener registered (API ${Build.VERSION.SDK_INT})")
    }

    /**
     * Add a 1×1 px invisible overlay window so this process holds a
     * WindowManager token. On Android 10+ (API 29+) this token counts as
     * "focused" and lifts the clipboard restriction that blocks
     * getPrimaryClip() from background — the ClipCascade trick.
     *
     * Idempotent: does nothing if the overlay is already present.
     * Guarded by Settings.canDrawOverlays — on devices without the
     * SYSTEM_ALERT_WINDOW permission the call is a no-op and the existing
     * AccessibilityService path continues to be the background capture mechanism.
     *
     * Must be called from the main thread (WindowManager.addView requirement).
     * Only TYPE_APPLICATION_OVERLAY is legal for background services on API 26+.
     *
     * FLAG_NOT_TOUCHABLE | FLAG_NOT_FOCUSABLE: the overlay is completely
     * invisible and input-transparent — it cannot steal focus or touches from
     * the user. Its sole purpose is giving the process a window token.
     */
    private fun addCaptureOverlay() {
        if (captureOverlayView != null) return  // already present — idempotent

        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) return  // canDrawOverlays needs API 23
        if (!android.provider.Settings.canDrawOverlays(this)) {
            Log.d(TAG, "addCaptureOverlay: SYSTEM_ALERT_WINDOW not granted — skipping overlay (a11y path remains active)")
            return
        }

        val wm = getSystemService(WINDOW_SERVICE) as? WindowManager ?: run {
            Log.w(TAG, "addCaptureOverlay: WindowManager unavailable")
            return
        }

        val params = WindowManager.LayoutParams(
            /* width  */ 1,
            /* height */ 1,
            /* type   */ WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            /* flags  */ WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE or
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE,
            /* format */ PixelFormat.TRANSLUCENT
        ).apply {
            alpha = 0f   // fully transparent — invisible to the user
        }

        val view = View(this)
        try {
            wm.addView(view, params)
            captureOverlayView = view
            Log.i(TAG, "addCaptureOverlay: invisible overlay added — background clipboard reads enabled")
        } catch (e: Exception) {
            // addView can throw if the permission was revoked between the
            // canDrawOverlays check and the addView call, or on some OEM ROMs
            // that return false from canDrawOverlays at add-time. Non-fatal —
            // fall back to the AccessibilityService capture path.
            Log.w(TAG, "addCaptureOverlay: addView failed (${e.javaClass.simpleName}: ${e.message}) — falling back to a11y path")
        }
    }

    /**
     * Remove the capture overlay if it was added. Idempotent.
     * Safe to call from onDestroy even if addCaptureOverlay was never called or failed.
     */
    private fun removeCaptureOverlay() {
        val view = captureOverlayView ?: return
        captureOverlayView = null
        val wm = getSystemService(WINDOW_SERVICE) as? WindowManager ?: return
        try {
            wm.removeView(view)
            Log.i(TAG, "removeCaptureOverlay: overlay removed")
        } catch (e: Exception) {
            // removeView can throw if the view was already detached (e.g. the
            // WindowManager died or the permission was revoked). Non-fatal.
            Log.w(TAG, "removeCaptureOverlay: removeView failed (${e.javaClass.simpleName}: ${e.message})")
        }
    }

    /**
     * Encrypt [text] and persist via [ClipboardRepository].
     * Falls back to local AES when the UniFFI .so is unavailable.
     * Skips storage for content flagged as sensitive.
     * Honours [Settings.captureEnabled] — toggled by the notification's
     * Pause/Resume action.
     */
    private suspend fun handleClipboardChange(text: String) {
        captureClip(this, text, settings, repository, syncManager)
    }

    override fun onDestroy() {
        // Stop the inbound listener (cancels its drain job + releases the bound
        // port) BEFORE scope.cancel() so the native accept loop is torn down
        // cleanly rather than left dangling on an orphaned coroutine.
        stopInboundP2pListener()
        fgsSyncLoop.stop()
        // P1.4: close the WS channel gracefully (sends phx_leave) before the
        // scope is cancelled — avoids an abrupt TCP close that Supabase would
        // count against the connection quota.
        realtimeClient?.close()
        // Stop the relay SSE subscription before the scope is cancelled.
        relayClient?.close()
        clipboardManager.removePrimaryClipChangedListener(clipListener)
        settings.stopObserving(prefsListener)
        removeCaptureOverlay()
        scope.cancel()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        private const val TAG = "ClipboardService"
        const val NOTIFICATION_ID = 1001
        const val CHANNEL_ID = "copypaste_service"

        /**
         * Port the inbound mTLS P2P listener is currently bound to (OS-assigned),
         * or 0 when no listener is running. Published by [startInboundP2pListener]
         * / cleared by [stopInboundP2pListener] so [PairActivity] can advertise
         * `"<lan-ip>:<port>"` to a peer at pair time (Path A: the peer persists it
         * and dials back). @Volatile — read cross-thread from the pairing flow.
         */
        @Volatile
        var activeListenerPort: Int = 0
            private set

        /** Separate notification channel for the one-time "enable Accessibility" action prompt. */
        private const val CHANNEL_A11Y_WARN = "copypaste_a11y_warn"

        /** Stable notification id for the a11y-required warning (never collides with NOTIFICATION_ID). */
        private const val NOTIF_ID_A11Y_WARN = 1002

        /**
         * SharedPreferences key used to gate the a11y-required warning to a single
         * notification per install. We show it once when clipboard data is first
         * silently dropped due to the Android 10+ restriction, then never again —
         * the user can always re-open Onboarding from Settings.
         */
        private const val KEY_A11Y_WARN_SHOWN = "a11y_warn_shown"

        /**
         * Notification channel for per-copy event toasts (A-SET-6 parity).
         * IMPORTANCE_MIN = no sound, no heads-up, no status-bar icon — just a
         * silent badge in the shade so the user can see "item captured" without
         * being disturbed. Auto-cancelled after 2 seconds.
         */
        const val CHANNEL_COPY_EVENT = "copypaste_copy_event"

        /** Stable notification id for the per-copy event notification. */
        private const val NOTIF_ID_COPY_EVENT = 1003

        /**
         * Debounce guard: timestamp (System.currentTimeMillis) of the last copy
         * notification. If another capture arrives within [COPY_NOTIF_DEBOUNCE_MS],
         * the notification is refreshed in-place (same id) rather than posting a
         * new one, preventing rapid bursts from stacking.
         */
        @Volatile
        private var lastCopyNotifMs = 0L
        private const val COPY_NOTIF_DEBOUNCE_MS = 500L

        private const val PREFS_NAME = "copypaste_notif"
        private const val KEY_DAY_BUCKET = "day_bucket"
        private const val KEY_TODAY_COUNT = "today_count"

        /**
         * Shared capture pipeline: store + count + sync. HIGH-2.
         *
         * Both the foreground [ClipboardService] and the background
         * [ClipboardAccessibilityService] funnel captures through here so that
         * a11y-captured clips (the PRIMARY background path on Android 10+, where
         * the FGS's getPrimaryClip() returns null) are stored, counted in the
         * notification, AND pushed to sync — exactly like foreground captures.
         * Previously the a11y service only called repository.storeItem, so
         * backgrounded copies were stored locally but never synced/counted.
         *
         * The native SQLite insert and the repository store mirror
         * [ClipboardService]'s original logic: the native insert is
         * fire-and-forget (the UI reads the SharedPreferences repository, not
         * the native DB), so it must not gate repository.storeItem.
         */
        suspend fun captureClip(
            context: Context,
            text: String,
            settings: Settings,
            repository: ClipboardRepository,
            syncManager: SyncManager,
        ) {
            if (text.isBlank()) return

            // Copy-from-history echo guard: when the user taps a row in
            // HistoryActivity to copy it, the UI sets the primary clip to that
            // text, which these listeners then observe as a "new" clipboard
            // change. Outside the 2 s dedup window (the original was copied long
            // ago) this would create a duplicate row AND re-push to the cloud.
            // HistoryActivity registers the expected content-hash right before
            // setPrimaryClip; consume it here and skip the re-capture once.
            if (ClipboardRepository.shouldSkipExpectedClip(text)) {
                Log.d(TAG, "Skipping copy-from-history echo (expected clip)")
                return
            }

            // Notification-driven pause: drop the change but keep listeners
            // registered so resuming is instant (no service restart).
            if (!settings.captureEnabled) {
                Log.d(TAG, "Capture paused — dropping clipboard change")
                return
            }

            // Private mode: when enabled, do NOT persist or sync clipboard items.
            // privateMode=true → suppress capture; privateMode=false (default) → allow capture.
            if (settings.privateMode) {
                Log.d(TAG, "Private mode enabled — dropping clipboard change")
                return
            }

            val sensitive = try { isSensitive(text) } catch (_: UnsatisfiedLinkError) { false }
            if (sensitive) {
                Log.d(TAG, "Sensitive clip detected — skipping storage")
                return
            }

            val key = settings.encryptionKey

            // Native SQLite insert (sync subsystem only) — fire-and-forget.
            try {
                val nativeId = addClipboardItem(databasePath(context), key, text)
                if (nativeId.isNotEmpty()) {
                    Log.d(TAG, "Native insert ok: $nativeId")
                }
            } catch (e: UnsatisfiedLinkError) {
                Log.d(TAG, "Native addClipboardItem unavailable (no live .so)")
            } catch (e: CopypasteException) {
                Log.w(TAG, "Native addClipboardItem failed (${e.message})")
            }

            // Generate ONE lamport tick at capture time and thread the SAME value
            // into both the stored local row AND the cloud push. Previously the
            // stored row defaulted to lamport_ts=0 while the push minted a fresh
            // tick, so the two disagreed and LWW reconciliation broke on a later
            // poll (the synced-back row always looked "newer" than the local one).
            val lamportTs = settings.lamportClock.tick()

            // Persist to the SharedPreferences repository — the single source the
            // UI reads. storeItem performs cross-listener dedup (HIGH-3) so a
            // single copy seen by multiple owners is stored (and counted) once.
            val storedId = repository.storeItem(text, key, lamportTs = lamportTs)
            if (storedId.isNotEmpty()) {
                Log.d(TAG, "Clipboard item stored successfully")
                bumpTodayCounter(context)
                refreshNotification(context)
                if (settings.notifyOnCopy) postCopyNotification(context)
                if (settings.soundOnCopy) playCopySound(context)
                if (settings.syncEnabled) {
                    notifySyncManager(
                        itemId = storedId,
                        payload = text.toByteArray(Charsets.UTF_8),
                        contentType = "text",
                        settings = settings,
                        syncManager = syncManager,
                        lamportTs = lamportTs,
                    )
                }
            }
        }

        /**
         * Capture an image clipboard item from a content:// [uri].
         *
         * Stores the original image at full resolution AND generates a downscaled
         * thumbnail (max ~680 px, WebP LOSSY 80 on API 30+, PNG fallback) stored
         * under a separate "item_thumb_<id>" key. The history list displays the
         * thumbnail for lower memory pressure; copy/open still uses full-res.
         *
         * OOM is caught explicitly. The full-res size cap is enforced by
         * [ClipboardRepository.storeImageBytes].
         *
         * TODO(synced-images): when a synced image arrives via FgsSyncLoop
         *   (off-limits file), call storeThumbnailBytes there too so synced image
         *   rows also benefit from thumbnail display.
         */
        suspend fun captureImageClip(
            context: Context,
            uri: android.net.Uri,
            mimeType: String,
            settings: Settings,
            repository: ClipboardRepository,
            syncManager: SyncManager,
        ) {
            // Copy-from-history echo guard (parity with text path in captureClip).
            // When HistoryActivity copies an image back to the clipboard it calls
            // ClipboardRepository.expectImageUri(uri) RIGHT BEFORE setPrimaryClip.
            // Without this check the capture listener fires, decodes the same bytes,
            // and creates a duplicate history row.  The text path has an identical
            // guard (shouldSkipExpectedClip); this is the image equivalent.
            if (ClipboardRepository.shouldSkipExpectedImageUri(uri)) {
                Log.d(TAG, "Skipping copy-from-history echo for image URI: $uri")
                return
            }

            if (!settings.captureEnabled) {
                Log.d(TAG, "Capture paused — dropping image clipboard change")
                return
            }

            // Private mode: mirror the text-path check in captureClip (~417).
            // Images must also be suppressed in private mode (privacy parity).
            if (settings.privateMode) {
                Log.d(TAG, "Private mode enabled — dropping image clipboard change")
                return
            }

            // Decode at full resolution (inSampleSize=1 = no sub-sampling).
            val decodeOpts = BitmapFactory.Options().apply {
                inSampleSize = 1
                inPreferredConfig = Bitmap.Config.ARGB_8888
            }
            val bitmap: Bitmap? = try {
                context.contentResolver.openInputStream(uri)?.use { stream ->
                    BitmapFactory.decodeStream(stream, null, decodeOpts)
                }
            } catch (t: Throwable) {
                Log.w(TAG, "captureImageClip: failed to decode image from $uri: ${t.message}")
                return
            }

            if (bitmap == null) {
                Log.w(TAG, "captureImageClip: BitmapFactory returned null for $uri — skipping")
                return
            }

            // Re-encode as PNG (lossless) for the full-res copy.
            // Also generate a thumbnail from the same Bitmap before recycling.
            // Both operations run before recycle() — bitmap stays valid for both.
            val pngBytes: ByteArray?
            val thumbBytes: ByteArray?
            try {
                pngBytes = try {
                    ByteArrayOutputStream().use { baos ->
                        bitmap.compress(Bitmap.CompressFormat.PNG, 100, baos)
                        baos.toByteArray()
                    }
                } catch (t: Throwable) {
                    Log.w(TAG, "captureImageClip: PNG encode failed: ${t.message}")
                    null
                }

                // Generate thumbnail while the Bitmap is still valid (before recycle).
                // ImageThumbnailUtils.generateThumbnail does NOT recycle bitmap.
                thumbBytes = try {
                    ImageThumbnailUtils.generateThumbnail(bitmap)
                } catch (t: Throwable) {
                    Log.w(TAG, "captureImageClip: thumbnail generation failed (non-fatal): ${t.message}")
                    null
                }
            } finally {
                bitmap.recycle()
            }

            if (pngBytes == null) return

            // Persist a placeholder text blob with the image MIME type so the row
            // appears in history, then attach the image bytes under the same id.
            // Generate ONE lamport tick and thread it into the stored row AND the
            // cloud push (parity with the text path) so LWW agrees on a later poll.
            val placeholder = uri.toString()
            val key = settings.encryptionKey
            val lamportTs = settings.lamportClock.tick()
            val storedId = repository.storeItem(
                placeholder,
                key,
                contentType = mimeType,
                lamportTs = lamportTs,
            )
            if (storedId.isEmpty()) {
                Log.d(TAG, "captureImageClip: storeItem returned empty (dedup/sensitive) — not storing bytes")
                return
            }

            repository.storeImageBytes(storedId, pngBytes)
            Log.d(TAG, "captureImageClip: stored full-res image $storedId (${pngBytes.size} bytes, mime=$mimeType)")

            if (thumbBytes != null) {
                repository.storeThumbnailBytes(storedId, thumbBytes)
                Log.d(TAG, "captureImageClip: stored thumbnail $storedId (${thumbBytes.size} bytes)")
            } else {
                Log.d(TAG, "captureImageClip: no thumbnail generated for $storedId — history will fall back to full-res")
            }

            bumpTodayCounter(context)
            refreshNotification(context)
            if (settings.notifyOnCopy) postCopyNotification(context)
            if (settings.soundOnCopy) playCopySound(context)

            // AB-4: push the IMAGE bytes to the cloud (Supabase + relay) the same
            // way text does. content_type "image" makes the receiver store raw
            // bytes (build_local_blob_item on macOS, the image branch on Android)
            // instead of UTF-8-decoding binary. No header — images carry none.
            if (settings.syncEnabled) {
                notifySyncManager(
                    itemId = storedId,
                    payload = pngBytes,
                    contentType = "image",
                    settings = settings,
                    syncManager = syncManager,
                    lamportTs = lamportTs,
                )
            }
        }

        /**
         * Capture a file clipboard item from a content:// or file:// [uri].
         *
         * Called when the clipboard item has a non-text, non-image MIME type
         * (e.g. application/pdf, application/zip). Reads the raw bytes via
         * [contentResolver], derives the filename from [OpenableColumns.DISPLAY_NAME]
         * (falling back to the last URI path segment), and stores the item as
         * `content_type="file"` with a "[file: <name>]" label.
         *
         * The stored item is included in the next P2P sync push via
         * [ClipboardRepository.localItemsForSync], which attaches the bytes and
         * metadata through [getFileBytes]/[getFileMeta].
         *
         * Size is gated by [ClipboardRepository.storeFileBytes]'s internal cap.
         * Private-mode and capture-paused guards mirror [captureImageClip].
         */
        suspend fun captureFileClip(
            context: Context,
            uri: android.net.Uri,
            mimeType: String,
            settings: Settings,
            repository: ClipboardRepository,
            // AB-4: when supplied AND sync is enabled, the captured file is also
            // pushed to the cloud (Supabase + relay). Optional/defaulted so the
            // accessibility-service caller (which has no SyncManager wired) compiles
            // unchanged and simply skips the cloud push.
            syncManager: SyncManager? = null,
        ) {
            // Copy-from-history echo guard (mirrors text + image paths above).
            // HistoryActivity calls ClipboardRepository.expectImageUri(uri) before
            // setPrimaryClip for the file copy-back path; suppress the re-capture here.
            if (ClipboardRepository.shouldSkipExpectedImageUri(uri)) {
                Log.d(TAG, "captureFileClip: skipping copy-from-history echo for URI: $uri")
                return
            }

            if (!settings.captureEnabled) {
                Log.d(TAG, "captureFileClip: capture paused — dropping file clipboard change")
                return
            }
            if (settings.privateMode) {
                Log.d(TAG, "captureFileClip: private mode — dropping file clipboard change")
                return
            }

            // Read raw bytes from the content provider.
            val fileBytes: ByteArray = try {
                context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
                    ?: run {
                        Log.w(TAG, "captureFileClip: openInputStream returned null for $uri")
                        return
                    }
            } catch (t: Throwable) {
                Log.w(TAG, "captureFileClip: failed to read bytes from $uri: ${t.message}")
                return
            }

            if (fileBytes.isEmpty()) {
                Log.d(TAG, "captureFileClip: empty byte array for $uri — skipping")
                return
            }

            // Derive filename: prefer OpenableColumns.DISPLAY_NAME, fall back to
            // the last path segment of the URI (common for file:// URIs).
            val fileName: String? = try {
                context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                    ?.use { cursor ->
                        if (cursor.moveToFirst()) {
                            val col = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                            if (col >= 0) cursor.getString(col) else null
                        } else null
                    }
                    ?: uri.lastPathSegment
            } catch (_: Exception) {
                uri.lastPathSegment
            }

            val key = settings.encryptionKey
            val label = SyncFileHelper.buildFileLabel(fileName)
            // Generate ONE lamport tick and thread it into the stored row AND the
            // cloud push (parity with the text/image paths) so LWW agrees later.
            val lamportTs = settings.lamportClock.tick()
            val storedId = repository.storeItem(
                plaintext = label,
                key = key,
                contentType = "file",
                lamportTs = lamportTs,
            )
            if (storedId.isEmpty()) {
                Log.d(TAG, "captureFileClip: storeItem returned empty (dedup/sensitive) — skipping")
                return
            }

            repository.storeFileBytes(storedId, fileBytes)
            repository.storeFileMeta(storedId, fileName, mimeType)
            Log.d(
                TAG,
                "captureFileClip: stored $storedId (${fileBytes.size} bytes, " +
                    "name=$fileName, mime=$mimeType)",
            )

            bumpTodayCounter(context)
            refreshNotification(context)
            if (settings.notifyOnCopy) postCopyNotification(context)
            if (settings.soundOnCopy) playCopySound(context)

            // AB-4: push the FILE to the cloud (Supabase + relay) the same way text
            // does. ENCODE the cloud file-identity header (name + mime + bytes) so
            // the receiver recovers the original name/MIME (AB-3) — byte-for-byte
            // the same envelope the macOS daemon ships. Only when a SyncManager is
            // wired (the foreground-service capture path) and sync is enabled.
            if (settings.syncEnabled && syncManager != null) {
                val payload = SyncManager.encodeCloudFilePayload(
                    name = fileName ?: SyncManager.CLOUD_FILE_LEGACY_NAME,
                    mime = mimeType.ifBlank { SyncManager.CLOUD_FILE_LEGACY_MIME },
                    fileBytes = fileBytes,
                )
                notifySyncManager(
                    itemId = storedId,
                    payload = payload,
                    contentType = "file",
                    settings = settings,
                    syncManager = syncManager,
                    lamportTs = lamportTs,
                )
            }
        }

        /** Path to the app-private encrypted SQLite DB used by the UniFFI live binding. */
        private fun databasePath(context: Context): String =
            context.applicationContext.getDatabasePath("copypaste.db").absolutePath

        /**
         * Push one freshly-captured local item to the configured cloud backend.
         *
         * AB-4: routes by ACTUAL [contentType] — text/image/file — instead of the
         * old text-only path. [payload] is the EXACT byte payload the cloud blob
         * must carry:
         *   - text  → UTF-8 bytes of the clip
         *   - image → raw image bytes (PNG)
         *   - file  → the cloud file-identity header + bytes
         *             (`SyncManager.encodeCloudFilePayload(name, mime, bytes)`),
         *             so the receiver recovers the original name/MIME (AB-3).
         * The same [payload] is shipped over BOTH the Supabase and relay transports
         * under the row's STABLE [itemId].
         */
        private suspend fun notifySyncManager(
            itemId: String,
            payload: ByteArray,
            contentType: String,
            settings: Settings,
            syncManager: SyncManager,
            lamportTs: Long,
        ) {
            when (settings.syncBackend) {
                SyncBackend.SUPABASE -> {
                    // Supabase path: encrypt with cross-device SyncKey (schema v5),
                    // push to Supabase PostgREST. Interoperates with macOS daemon.
                    // STABLE identity: push under the row's persisted [itemId]
                    // (overrideId) so the cloud item_id matches the local row and
                    // is reused on every push — the daemon dedups/LWW-merges
                    // instead of seeing a new item each time (the duplicates bug).
                    try {
                        val id = syncManager.pushToSupabase(
                            plaintext = payload,
                            contentType = contentType,
                            overrideId = itemId,
                            deviceId = settings.deviceId,
                            lamportTs = lamportTs,
                        )
                        if (id != null) {
                            Log.d(TAG, "Supabase push ok: $id ($contentType)")
                        } else {
                            Log.w(TAG, "Supabase push returned null (logged above)")
                        }
                    } catch (e: Exception) {
                        Log.w(TAG, "Supabase push failed: ${e.message}")
                    }
                }
                SyncBackend.RELAY -> {
                    // Relay path: encrypt with the cross-device cloud SyncKey via
                    // cloud_encrypt (item_id bound into the AEAD AAD), wrap as a
                    // RelayEnvelope, and POST to the derived shared inbox. STABLE
                    // identity: push under the row's persisted [itemId] so the
                    // relay item_id matches the local row and is reused on every
                    // push, mirroring the Supabase branch above. pushToRelay runs
                    // on Dispatchers.IO internally and zeroes the sync key after use.
                    try {
                        val ok = syncManager.pushToRelay(
                            itemId = itemId,
                            plaintext = payload,
                            contentType = contentType,
                            lamportTs = lamportTs,
                        )
                        if (ok) {
                            Log.d(TAG, "Relay push ok: $itemId ($contentType)")
                        } else {
                            Log.w(TAG, "Relay push returned false (logged above)")
                        }
                    } catch (e: Exception) {
                        Log.w(TAG, "Relay push failed: ${e.message}")
                    }
                }
            }
        }

        /**
         * Post (or refresh) the per-copy event notification.
         *
         * Debounced: if the previous notification was posted within
         * [COPY_NOTIF_DEBOUNCE_MS], this call updates it in-place (same id)
         * rather than emitting a new one — rapid-paste bursts produce a single
         * updating notification rather than a stack.
         *
         * Requires POST_NOTIFICATIONS permission on API 33+; on older APIs the
         * permission is implicit. [NotificationManagerCompat.notify] is a no-op
         * when the permission has not been granted, so no guard is needed here.
         */
        fun postCopyNotification(context: Context) {
            val now = System.currentTimeMillis()
            // Atomic CAS-style update: read, decide, write under no lock — worst
            // case two threads both post; that is fine (same stable id, idempotent).
            lastCopyNotifMs = now
            ensureChannel(context)
            val notification = NotificationCompat.Builder(context, CHANNEL_COPY_EVENT)
                .setSmallIcon(android.R.drawable.ic_menu_edit)
                .setContentTitle(context.getString(R.string.notif_copy_event_title))
                .setContentText(context.getString(R.string.notif_copy_event_content))
                .setPriority(NotificationCompat.PRIORITY_MIN)
                .setCategory(NotificationCompat.CATEGORY_EVENT)
                .setAutoCancel(true)
                .setTimeoutAfter(2_000L)
                .setOnlyAlertOnce(true)
                .build()
            NotificationManagerCompat.from(context).notify(NOTIF_ID_COPY_EVENT, notification)
        }

        /**
         * Play a subtle UI click sound to acknowledge a clipboard capture.
         *
         * Uses [AudioManager.playSoundEffect] with [SoundEffectConstants.CLICK],
         * which respects the system "touch sounds" volume and is available on all
         * API levels. The call is intentionally non-blocking and fire-and-forget.
         * Errors are swallowed — a missing sound must never break capture.
         */
        fun playCopySound(context: Context) {
            try {
                val am = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
                am.playSoundEffect(SoundEffectConstants.CLICK, -1f)
            } catch (e: Exception) {
                Log.d(TAG, "playCopySound failed (non-fatal): ${e.message}")
            }
        }

        /**
         * Ensure all notification channels exist. Idempotent — calling twice is a
         * no-op on the framework side (createNotificationChannel is idempotent).
         *
         * [CHANNEL_ID]: IMPORTANCE_LOW = silent (no sound, no heads-up).
         *   setShowBadge(false) keeps the launcher icon clean.
         *
         * [CHANNEL_A11Y_WARN]: IMPORTANCE_DEFAULT = shows a heads-up the first
         *   time, which is intentional — the user needs to act on it to restore
         *   background clipboard capture on Android 10+.
         *
         * [CHANNEL_COPY_EVENT]: IMPORTANCE_MIN = silent badge only, no heads-up.
         */
        fun ensureChannel(context: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            val nm = context.getSystemService(NotificationManager::class.java) ?: return

            if (nm.getNotificationChannel(CHANNEL_ID) == null) {
                nm.createNotificationChannel(
                    NotificationChannel(
                        CHANNEL_ID,
                        context.getString(R.string.notif_channel_service_name),
                        NotificationManager.IMPORTANCE_LOW
                    ).apply {
                        description = context.getString(R.string.notif_channel_service_description)
                        setShowBadge(false)
                        enableVibration(false)
                        setSound(null, null)
                    }
                )
            }

            if (nm.getNotificationChannel(CHANNEL_A11Y_WARN) == null) {
                nm.createNotificationChannel(
                    NotificationChannel(
                        CHANNEL_A11Y_WARN,
                        context.getString(R.string.notif_channel_a11y_warn_name),
                        NotificationManager.IMPORTANCE_DEFAULT
                    ).apply {
                        description = context.getString(R.string.notif_channel_a11y_warn_description)
                        setShowBadge(true)
                    }
                )
            }

            if (nm.getNotificationChannel(CHANNEL_COPY_EVENT) == null) {
                nm.createNotificationChannel(
                    NotificationChannel(
                        CHANNEL_COPY_EVENT,
                        context.getString(R.string.notif_channel_copy_event_name),
                        NotificationManager.IMPORTANCE_MIN
                    ).apply {
                        description = context.getString(R.string.notif_channel_copy_event_description)
                        setShowBadge(false)
                        enableVibration(false)
                        setSound(null, null)
                    }
                )
            }
        }

        fun maybeNotifyA11yRequired(context: Context) {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            if (prefs.getBoolean(KEY_A11Y_WARN_SHOWN, false)) return
            prefs.edit().putBoolean(KEY_A11Y_WARN_SHOWN, true).apply()

            Log.w(
                TAG,
                "Android 10+ clipboard restriction: getPrimaryClip() returned null from " +
                    "background FGS — background capture disabled until Accessibility " +
                    "Service is enabled. Issuing one-time setup notification."
            )

            ensureChannel(context)
            val nm = context.getSystemService(NotificationManager::class.java) ?: return

            val piFlags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            val onboardingIntent = Intent(context, OnboardingActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            }
            val onboardingPi = PendingIntent.getActivity(context, 10, onboardingIntent, piFlags)

            val notification = NotificationCompat.Builder(context, CHANNEL_A11Y_WARN)
                .setContentTitle(context.getString(R.string.notif_a11y_warn_title))
                .setContentText(context.getString(R.string.notif_a11y_warn_content))
                .setSmallIcon(android.R.drawable.ic_dialog_info)
                .setAutoCancel(true)
                .setPriority(NotificationCompat.PRIORITY_DEFAULT)
                .setContentIntent(onboardingPi)
                .addAction(0, context.getString(R.string.notif_a11y_warn_action), onboardingPi)
                .setStyle(
                    NotificationCompat.BigTextStyle()
                        .bigText(context.getString(R.string.notif_a11y_warn_content_long))
                )
                .build()

            nm.notify(NOTIF_ID_A11Y_WARN, notification)
        }

        /**
         * Re-issue the foreground notification using current [Settings] state.
         * Called by [CaptureControlReceiver] after toggling pause/resume, and
         * by this service after each successful capture so the count updates.
         */
        fun refreshNotification(context: Context) {
            val nm = context.getSystemService(NotificationManager::class.java) ?: return
            ensureChannel(context)
            nm.notify(NOTIFICATION_ID, buildNotification(context))
        }

        /**
         * Guards [bumpTodayCounter]'s read-modify-write against concurrent callers
         * (ClipboardService + ClipboardAccessibilityService both call captureClip/
         * captureImageClip on the IO dispatcher and can race on the same prefs file).
         */
        private val counterLock = Any()

        /**
         * Bump today's capture counter. Rolls over at local midnight (uses
         * day-of-year as the bucket key so the rollover is visible the
         * morning after).
         *
         * Guarded by [counterLock] to prevent a lost-update between the read of
         * KEY_TODAY_COUNT and the write of KEY_TODAY_COUNT + 1.
         */
        private fun bumpTodayCounter(context: Context) {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            synchronized(counterLock) {
                val today = todayBucket()
                val storedBucket = prefs.getInt(KEY_DAY_BUCKET, -1)
                val current = if (storedBucket == today) prefs.getInt(KEY_TODAY_COUNT, 0) else 0
                prefs.edit()
                    .putInt(KEY_DAY_BUCKET, today)
                    .putInt(KEY_TODAY_COUNT, current + 1)
                    .apply()
            }
        }

        /**
         * Reconcile the "captured today" counter after the user removes clips.
         * Decrements by [count] (floored at 0) and re-issues the notification so
         * the shown number reflects the store after a delete/clear. The counter
         * is otherwise monotonic-on-capture, so without this a deletion left the
         * notification reporting a stale, too-high total. Safe to call from any
         * thread — SharedPreferences and NotificationManager are both
         * thread-safe.
         */
        fun onItemsDeleted(context: Context, count: Int) {
            if (count <= 0) return
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            val today = todayBucket()
            val storedBucket = prefs.getInt(KEY_DAY_BUCKET, -1)
            // Only adjust when the stored bucket is today's — a delete of an
            // older clip must not resurrect/zero a fresh day's bucket.
            if (storedBucket != today) {
                refreshNotification(context)
                return
            }
            val current = prefs.getInt(KEY_TODAY_COUNT, 0)
            val next = (current - count).coerceAtLeast(0)
            prefs.edit()
                .putInt(KEY_DAY_BUCKET, today)
                .putInt(KEY_TODAY_COUNT, next)
                .apply()
            refreshNotification(context)
        }

        private fun readTodayCount(context: Context): Int {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            val today = todayBucket()
            val storedBucket = prefs.getInt(KEY_DAY_BUCKET, -1)
            return if (storedBucket == today) prefs.getInt(KEY_TODAY_COUNT, 0) else 0
        }

        private fun todayBucket(): Int {
            val cal = Calendar.getInstance()
            // YYYY * 1000 + DOY — unique per local day, monotonically increasing
            // across year boundaries.
            return cal.get(Calendar.YEAR) * 1000 + cal.get(Calendar.DAY_OF_YEAR)
        }

        /**
         * Build the foreground-service notification. Visible state:
         *  - Title: "Active" or "Paused" depending on [Settings.captureEnabled]
         *  - Body: "<N> items captured today" / "Capture paused..."
         *  - Actions: Pause/Resume (toggle), Open (launch MainActivity)
         */
        fun buildNotification(context: Context): Notification {
            ensureChannel(context)
            val settings = Settings(context)
            val paused = !settings.captureEnabled
            val count = readTodayCount(context)

            val title = context.getString(
                if (paused) R.string.notif_title_paused else R.string.notif_title_active
            )
            val content = if (paused) {
                context.getString(R.string.notif_content_paused)
            } else {
                context.getString(R.string.notif_content_today, count)
            }

            // Pending-intent flag set: IMMUTABLE is required on API 31+, allowed
            // on older releases (NotificationCompat handles back-compat).
            val piFlags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE

            val openIntent = Intent(context, MainActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
            }
            val openPi = PendingIntent.getActivity(context, 0, openIntent, piFlags)

            val toggleAction = if (paused) {
                CaptureControlReceiver.ACTION_RESUME to R.string.notif_action_resume
            } else {
                CaptureControlReceiver.ACTION_PAUSE to R.string.notif_action_pause
            }
            val togglePi = PendingIntent.getBroadcast(
                context,
                if (paused) 1 else 2,
                Intent(toggleAction.first).setPackage(context.packageName),
                piFlags
            )

            return NotificationCompat.Builder(context, CHANNEL_ID)
                .setContentTitle(title)
                .setContentText(content)
                .setSmallIcon(android.R.drawable.ic_menu_edit)
                .setColor(0xFF3D8BFF.toInt())
                .setOngoing(true)
                .setShowWhen(false)
                .setOnlyAlertOnce(true)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setCategory(NotificationCompat.CATEGORY_SERVICE)
                .setVisibility(NotificationCompat.VISIBILITY_SECRET)
                .setContentIntent(openPi)
                .addAction(0, context.getString(toggleAction.second), togglePi)
                .addAction(0, context.getString(R.string.notif_action_open), openPi)
                .setStyle(NotificationCompat.BigTextStyle().bigText(content))
                .build()
        }
    }
}
