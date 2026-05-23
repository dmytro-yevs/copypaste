package com.copypaste.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import androidx.core.app.NotificationCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.util.Calendar

/**
 * Foreground service for clipboard monitoring on Android 8.0+ (API 26-28).
 * On Android 10+ (API 29+), clipboard access from background requires
 * the DEFAULT_INPUT_METHOD permission — not feasible for regular apps.
 * This service runs on API 26-28 where background clipboard access is allowed.
 *
 * Uses [ClipboardManager.OnPrimaryClipChangedListener] registered on the main
 * thread (required by the framework), then dispatches encrypt + store work to
 * a coroutine on [Dispatchers.IO].
 *
 * v0.3 T4 polish: notification redesigned with dynamic title (Active/Paused),
 * today's capture count, and Pause/Resume + Open action buttons. The pause
 * toggle is wired through [CaptureControlReceiver] which flips the persisted
 * [Settings.captureEnabled] flag — checked here before each store.
 */
class ClipboardService : Service() {

    private val scope = CoroutineScope(Dispatchers.IO)
    private lateinit var settings: Settings
    private lateinit var repository: ClipboardRepository
    private lateinit var clipboardManager: ClipboardManager
    private lateinit var syncManager: SyncManager

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        // primaryClip is non-null from background only on API 26-28
        val clip = clipboardManager.primaryClip ?: return@OnPrimaryClipChangedListener
        val text = clip.getItemAt(0)?.text?.toString()
            ?: return@OnPrimaryClipChangedListener

        scope.launch { handleClipboardChange(text) }
    }

    override fun onCreate() {
        super.onCreate()
        settings = Settings(this)
        repository = ClipboardRepository(this)

        val relayClient = RelayClient(settings.relayUrl)
        syncManager = SyncManager(relayClient, settings.deviceId, token = "")

        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        ensureChannel(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(NOTIFICATION_ID, buildNotification(this))

        // Listener must be registered on the main thread (framework requirement)
        Handler(Looper.getMainLooper()).post {
            monitorClipboard()
        }

        return START_STICKY
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
        if (text.isBlank()) return

        // Notification-driven pause: drop the change on the floor but keep the
        // listener registered so resuming is instant (no service restart).
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

        // Live UniFFI path: insert directly into the encrypted SQLite DB via
        // copypaste-core. Empty id from native side also means "skipped as
        // sensitive". On UnsatisfiedLinkError (no .so) or DB failure, fall
        // through to the SharedPreferences repository so the app stays usable.
        var nativeInsertOk = false
        try {
            val nativeId = addClipboardItem(databasePath, key, text)
            if (nativeId.isNotEmpty()) {
                nativeInsertOk = true
                Log.d(TAG, "Native insert ok: $nativeId")
            }
        } catch (e: UnsatisfiedLinkError) {
            Log.d(TAG, "Native addClipboardItem unavailable — falling back to repo")
        } catch (e: CopypasteException) {
            Log.w(TAG, "Native addClipboardItem failed (${e.message}) — falling back to repo")
        }

        val stored = if (nativeInsertOk) true else repository.storeItem(text, key)
        if (stored) {
            Log.d(TAG, "Clipboard item stored successfully")
            // Bump today's counter so the next notification update shows the new
            // value; refresh the notification so users see live progress.
            bumpTodayCounter(this)
            refreshNotification(this)
            if (settings.syncEnabled) {
                notifySyncManager(text, key)
            }
        }
    }

    /** Path to the app-private encrypted SQLite DB used by the UniFFI live binding. */
    private val databasePath: String
        get() = applicationContext.getDatabasePath("copypaste.db").absolutePath

    private suspend fun notifySyncManager(plaintext: String, key: ByteArray) {
        try {
            // Generate the item id BEFORE encrypting so the same id can be
            // bound into the AEAD AAD and forwarded to the relay. A mismatch
            // would cause the receiver to fail decryption silently.
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
            Log.w(TAG, "SyncManager upload failed: ${e.message}")
        }
    }

    override fun onDestroy() {
        clipboardManager.removePrimaryClipChangedListener(clipListener)
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
                .setSmallIcon(android.R.drawable.ic_menu_clipboard)
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
