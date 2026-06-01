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
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
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

    private val scope = CoroutineScope(Dispatchers.IO)
    private lateinit var settings: Settings
    private lateinit var repository: ClipboardRepository
    private lateinit var clipboardManager: ClipboardManager
    private lateinit var syncManager: SyncManager
    private lateinit var fgsSyncLoop: FgsSyncLoop

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

        val text = clip.getItemAt(0)?.text?.toString()
            ?: return@OnPrimaryClipChangedListener

        scope.launch { handleClipboardChange(text) }
    }

    override fun onCreate() {
        super.onCreate()
        settings = Settings(this)
        repository = ClipboardRepository(this)

        val relayClient = RelayClient(settings.relayUrl)
        syncManager = SyncManager(relayClient, settings.deviceId, token = "", settings = settings)
        fgsSyncLoop = FgsSyncLoop(settings, repository, syncManager, DeviceKeyStore(this))

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

        // Start the FGS-internal 60-second sync loop for near-real-time incoming sync.
        fgsSyncLoop.start(scope)

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
        fgsSyncLoop.stop()
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
                    notifySyncManager(storedId, text, key, settings, syncManager, lamportTs)
                }
            }
        }

        /**
         * Capture an image clipboard item from a content:// [uri].
         *
         * Stores the original image at full resolution. OOM is caught explicitly.
         * The size cap is enforced by [ClipboardRepository.storeImageBytes].
         */
        @Suppress("UNUSED_PARAMETER") // syncManager reserved for future image-sync wiring
        suspend fun captureImageClip(
            context: Context,
            uri: android.net.Uri,
            mimeType: String,
            settings: Settings,
            repository: ClipboardRepository,
            syncManager: SyncManager,
        ) {
            if (!settings.captureEnabled) {
                Log.d(TAG, "Capture paused — dropping image clipboard change")
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

            // Re-encode as PNG (lossless). bitmap.recycle() runs in finally so
            // native pixel memory is released immediately after the byte array is built.
            val pngBytes: ByteArray? = try {
                ByteArrayOutputStream().use { baos ->
                    bitmap.compress(Bitmap.CompressFormat.PNG, 100, baos)
                    baos.toByteArray()
                }
            } catch (t: Throwable) {
                Log.w(TAG, "captureImageClip: PNG encode failed: ${t.message}")
                null
            } finally {
                bitmap.recycle()
            }

            if (pngBytes == null) return

            // Persist a placeholder text blob with the image MIME type so the row
            // appears in history, then attach the image bytes under the same id.
            val placeholder = uri.toString()
            val key = settings.encryptionKey
            val storedId = repository.storeItem(placeholder, key, contentType = mimeType)
            if (storedId.isEmpty()) {
                Log.d(TAG, "captureImageClip: storeItem returned empty (dedup/sensitive) — not storing bytes")
                return
            }

            repository.storeImageBytes(storedId, pngBytes)
            Log.d(TAG, "captureImageClip: stored image $storedId (${pngBytes.size} bytes, mime=$mimeType)")

            bumpTodayCounter(context)
            refreshNotification(context)
            if (settings.notifyOnCopy) postCopyNotification(context)
            if (settings.soundOnCopy) playCopySound(context)
            // Image sync is not wired in this version — text sync only for now.
        }

        /** Path to the app-private encrypted SQLite DB used by the UniFFI live binding. */
        private fun databasePath(context: Context): String =
            context.applicationContext.getDatabasePath("copypaste.db").absolutePath

        private suspend fun notifySyncManager(
            itemId: String,
            plaintext: String,
            key: ByteArray,
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
                            plaintext = plaintext.toByteArray(Charsets.UTF_8),
                            contentType = "text",
                            overrideId = itemId,
                            deviceId = settings.deviceId,
                            lamportTs = lamportTs,
                        )
                        if (id != null) {
                            Log.d(TAG, "Supabase push ok: $id")
                        } else {
                            Log.w(TAG, "Supabase push returned null (logged above)")
                        }
                    } catch (e: Exception) {
                        Log.w(TAG, "Supabase push failed: ${e.message}")
                    }
                }
                SyncBackend.RELAY -> {
                    // RELAY cloud backend is DISABLED.
                    //
                    // The relay path encrypted items with the local per-device AES
                    // key (settings.encryptionKey) which no other device holds, so
                    // every payload it uploaded was undecryptable by macOS or any
                    // peer. The incoming poll (syncIncoming / syncItems) was also
                    // never wired into any active code path, making the relay
                    // write-only. This combination makes the relay cloud backend
                    // completely broken for cross-device sync.
                    //
                    // Decision: cloud sync = Supabase only. Switch to Supabase in
                    // Settings to enable cross-device cloud sync. The P2P/pairing
                    // LAN path (dialPairedPeer in FgsSyncLoop) is unaffected.
                    Log.w(
                        TAG,
                        "relay cloud backend is disabled — use Supabase for " +
                            "cross-device cloud sync (Settings → Use Supabase Cloud Sync)"
                    )
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
         * Bump today's capture counter. Rolls over at local midnight (uses
         * day-of-year as the bucket key so the rollover is visible the
         * morning after).
         */
        private fun bumpTodayCounter(context: Context) {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            val today = todayBucket()
            val storedBucket = prefs.getInt(KEY_DAY_BUCKET, -1)
            val current = if (storedBucket == today) prefs.getInt(KEY_TODAY_COUNT, 0) else 0
            prefs.edit()
                .putInt(KEY_DAY_BUCKET, today)
                .putInt(KEY_TODAY_COUNT, current + 1)
                .apply()
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
