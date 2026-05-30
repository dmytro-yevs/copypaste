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
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import androidx.core.app.NotificationCompat
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
 * the `OnPrimaryClipChangedListener` on the main thread (framework requirement),
 * which *fires* even from background — but `getPrimaryClip()` inside the
 * callback will return null unless the process also has an enabled
 * AccessibilityService. [ClipboardAccessibilityService] provides that binding.
 * Without it this service still functions as a fallback on API 26-28 and while
 * the activity is in the foreground.
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
        // primaryClip is non-null from background only on API 26-28 or when
        // ClipboardAccessibilityService is enabled. On API 29+ without the
        // accessibility service, this callback fires but getPrimaryClip() returns
        // null — the early-return below handles that silently.
        val clip = clipboardManager.primaryClip ?: return@OnPrimaryClipChangedListener

        // Detect image MIME before falling back to text. Check all MIME types on
        // the ClipDescription; the first image/* type wins.
        val imageMime = (0 until clip.description.mimeTypeCount)
            .map { clip.description.getMimeType(it) }
            .firstOrNull { it.startsWith("image/") }

        if (imageMime != null) {
            // Image clip: read URI from the first item and dispatch to image path.
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
        Handler(Looper.getMainLooper()).post {
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
        scope.cancel()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        private const val TAG = "ClipboardService"
        const val NOTIFICATION_ID = 1001
        const val CHANNEL_ID = "copypaste_service"

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

            // Notification-driven pause: drop the change but keep listeners
            // registered so resuming is instant (no service restart).
            if (!settings.captureEnabled) {
                Log.d(TAG, "Capture paused — dropping clipboard change")
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

            // Persist to the SharedPreferences repository — the single source the
            // UI reads. storeItem performs cross-listener dedup (HIGH-3) so a
            // single copy seen by multiple owners is stored (and counted) once.
            val stored = repository.storeItem(text, key)
            if (stored) {
                Log.d(TAG, "Clipboard item stored successfully")
                bumpTodayCounter(context)
                refreshNotification(context)
                if (settings.syncEnabled) {
                    notifySyncManager(text, key, settings, syncManager)
                }
            }
        }

        /**
         * Capture an image clipboard item from a content:// [uri].
         *
         * Pipeline:
         *  1. Open an InputStream via ContentResolver and decode into a Bitmap.
         *  2. Downscale to fit within [IMAGE_MAX_PX] on the longest side using
         *     BitmapFactory.Options.inSampleSize so large images never bloat
         *     SharedPreferences. The sampling pass also caps memory usage.
         *  3. Re-encode as PNG (lossless, good for screenshots/text images) into
         *     a ByteArray and check the [IMAGE_MAX_BYTES] size cap before storing.
         *  4. Store a placeholder text blob via [ClipboardRepository.storeItem]
         *     with the real MIME content type so the row appears in the history
         *     list with the correct icon/thumbnail slot.
         *  5. Persist the scaled bytes via [ClipboardRepository.storeImageBytes]
         *     under the same id returned by storeItem.
         *
         * Errors are logged with Log.w and never thrown — a failed image capture
         * must not crash the service or suppress subsequent text captures.
         *
         * Android 10+ background restriction: the same API-29+ limitation that
         * affects text capture applies here. The URI is readable when:
         *   (a) the app is in the foreground (this FGS + activity listener), or
         *   (b) [ClipboardAccessibilityService] is enabled (background path).
         * Without the a11y service, getPrimaryClip() already returned null before
         * we reach this function — so no special guard is needed here.
         */
        // syncManager is accepted for API symmetry with captureClip and future image-sync
        // wiring; it is not yet used because image sync requires a separate Supabase
        // upload path (binary blobs) that is not implemented in this version.
        @Suppress("UNUSED_PARAMETER")
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

            // ── Step 1: decode bounds only (no pixel allocation) to compute inSampleSize.
            val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            try {
                context.contentResolver.openInputStream(uri)?.use { stream ->
                    BitmapFactory.decodeStream(stream, null, opts)
                }
            } catch (e: Exception) {
                Log.w(TAG, "captureImageClip: failed to read image bounds from $uri: ${e.message}")
                return
            }

            val rawW = opts.outWidth
            val rawH = opts.outHeight
            if (rawW <= 0 || rawH <= 0) {
                Log.w(TAG, "captureImageClip: invalid image dimensions ($rawW×$rawH) — skipping")
                return
            }

            // ── Step 2: compute inSampleSize so the longest edge fits IMAGE_MAX_PX.
            val longest = maxOf(rawW, rawH)
            var sampleSize = 1
            while (longest / (sampleSize * 2) > IMAGE_MAX_PX) {
                sampleSize *= 2
            }

            // ── Step 3: decode the full pixels at the chosen sample size.
            val decodeOpts = BitmapFactory.Options().apply {
                inSampleSize = sampleSize
                inPreferredConfig = Bitmap.Config.ARGB_8888
            }
            val bitmap: Bitmap? = try {
                context.contentResolver.openInputStream(uri)?.use { stream ->
                    BitmapFactory.decodeStream(stream, null, decodeOpts)
                }
            } catch (e: Exception) {
                Log.w(TAG, "captureImageClip: failed to decode image from $uri: ${e.message}")
                return
            }

            if (bitmap == null) {
                Log.w(TAG, "captureImageClip: BitmapFactory returned null for $uri — skipping")
                return
            }

            // ── Step 4: re-encode as PNG into a ByteArray and check size cap.
            // bitmap.recycle() is called in both success and failure paths so
            // native pixel memory is released as soon as we have the byte array.
            val pngBytes: ByteArray? = try {
                ByteArrayOutputStream().use { baos ->
                    bitmap.compress(Bitmap.CompressFormat.PNG, 100, baos)
                    baos.toByteArray()
                }
            } catch (e: Exception) {
                Log.w(TAG, "captureImageClip: PNG encode failed: ${e.message}")
                null
            } finally {
                bitmap.recycle()
            }

            if (pngBytes == null) return

            if (pngBytes.size > IMAGE_MAX_BYTES) {
                Log.w(
                    TAG,
                    "captureImageClip: encoded PNG is ${pngBytes.size} bytes (cap $IMAGE_MAX_BYTES) — skipping"
                )
                return
            }

            // ── Step 5: persist a text-blob placeholder with the image MIME type so
            // the row appears in history, then attach the image bytes under the same id.
            // Use the URI string as placeholder text (non-blank, informational, safe).
            val placeholder = uri.toString()
            val key = settings.encryptionKey
            val stored = repository.storeItem(placeholder, key, contentType = mimeType)
            if (!stored) {
                Log.d(TAG, "captureImageClip: storeItem returned false (dedup or sensitive) — not storing bytes")
                return
            }

            // Retrieve the id that storeItem just created. storeItem returns true/false in
            // this version of the repository, so we must look up the most recently inserted
            // id from the stored index. This is safe under idsWriteLock semantics because
            // storeItem already completed its synchronized write before returning.
            //
            // NOTE: storeItem in this worktree's version returns Boolean, not String.
            // We resolve the id by reading the last entry in the item_ids index, which is
            // always the id just written (appended at the end by storeItem under idsWriteLock).
            val storedId = repository.lastStoredId()
            if (storedId.isNullOrBlank()) {
                Log.w(TAG, "captureImageClip: could not resolve stored id — image bytes not persisted")
                return
            }

            repository.storeImageBytes(storedId, pngBytes)
            Log.d(TAG, "captureImageClip: stored image $storedId (${pngBytes.size} bytes, mime=$mimeType)")

            bumpTodayCounter(context)
            refreshNotification(context)
            // Image sync is not wired in this version — text sync only for now.
        }

        /**
         * Maximum pixel dimension (longest edge) for stored image thumbnails.
         * A 1024 px cap keeps PNG size well under [IMAGE_MAX_BYTES] for typical
         * screenshots while still being large enough to render a useful thumbnail.
         */
        private const val IMAGE_MAX_PX = 1024

        /**
         * Hard cap on stored image bytes (2 MB). SharedPreferences strings are
         * stored in the app's private XML file; very large values slow down
         * the prefs commit and inflate memory on load. 2 MB is conservative —
         * a 1024×1024 PNG is typically 50–300 KB for screenshots.
         */
        private const val IMAGE_MAX_BYTES = 2 * 1024 * 1024

        /** Path to the app-private encrypted SQLite DB used by the UniFFI live binding. */
        private fun databasePath(context: Context): String =
            context.applicationContext.getDatabasePath("copypaste.db").absolutePath

        private suspend fun notifySyncManager(
            plaintext: String,
            key: ByteArray,
            settings: Settings,
            syncManager: SyncManager,
        ) {
            when (settings.syncBackend) {
                SyncBackend.SUPABASE -> {
                    // Supabase path: encrypt with cross-device SyncKey (schema v5),
                    // push to Supabase PostgREST. Interoperates with macOS daemon.
                    try {
                        val id = syncManager.pushToSupabase(
                            plaintext = plaintext.toByteArray(Charsets.UTF_8),
                            contentType = "text",
                            deviceId = settings.deviceId,
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
                    // Relay path: encrypt with local device key + v3/v4 AAD,
                    // upload to custom relay server. Local-network only.
                    try {
                        // Generate the item id BEFORE encrypting so the same id can
                        // be bound into the AEAD AAD and forwarded to the relay. A
                        // mismatch would fail decryption on the receiver silently.
                        val itemId = java.util.UUID.randomUUID().toString()
                        val blob = try {
                            encryptText(itemId, plaintext.toByteArray(Charsets.UTF_8), key)
                        } catch (e: IllegalStateException) {
                            Log.d(TAG, "Native encryptText unavailable (${e.message}) — local AES")
                            ClipboardRepository.localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
                        } catch (_: UnsatisfiedLinkError) {
                            ClipboardRepository.localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
                        }
                        val lamportTs = System.currentTimeMillis()
                        syncManager.uploadItem(itemId, blob.ciphertext, blob.nonce, "text/plain", lamportTs)
                    } catch (e: Exception) {
                        Log.w(TAG, "Relay upload failed: ${e.message}")
                    }
                }
            }
        }

        /**
         * Ensure the foreground service channel exists. Idempotent — calling
         * twice is a no-op on the framework side.
         *
         * IMPORTANCE_LOW = silent (no sound, no heads-up). setShowBadge(false)
         * keeps the launcher icon clean.
         */
        fun ensureChannel(context: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            val nm = context.getSystemService(NotificationManager::class.java) ?: return
            val existing = nm.getNotificationChannel(CHANNEL_ID)
            if (existing != null) return
            val channel = NotificationChannel(
                CHANNEL_ID,
                context.getString(R.string.notif_channel_service_name),
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = context.getString(R.string.notif_channel_service_description)
                setShowBadge(false)
                enableVibration(false)
                setSound(null, null)
            }
            nm.createNotificationChannel(channel)
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
                .setColor(0xFF0066CC.toInt())
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
