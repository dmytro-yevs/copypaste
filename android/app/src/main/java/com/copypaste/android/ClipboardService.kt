package com.copypaste.android

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * Foreground service for clipboard monitoring on Android 8.0+ (API 26-28).
 * On Android 10+ (API 29+), clipboard access from background requires
 * the DEFAULT_INPUT_METHOD permission — not feasible for regular apps.
 * This service runs on API 26-28 where background clipboard access is allowed.
 *
 * Uses [ClipboardManager.OnPrimaryClipChangedListener] registered on the main
 * thread (required by the framework), then dispatches encrypt + store work to
 * a coroutine on [Dispatchers.IO].
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
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(NOTIFICATION_ID, buildNotification())

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
     */
    private suspend fun handleClipboardChange(text: String) {
        if (text.isBlank()) return

        val sensitive = try { isSensitive(text) } catch (_: UnsatisfiedLinkError) { false }
        if (sensitive) {
            Log.d(TAG, "Sensitive clip detected — skipping storage")
            return
        }

        val key = settings.encryptionKey
        val stored = repository.storeItem(text, key)
        if (stored) {
            Log.d(TAG, "Clipboard item stored successfully")
            if (settings.syncEnabled) {
                notifySyncManager(text, key)
            }
        }
    }

    private suspend fun notifySyncManager(plaintext: String, key: ByteArray) {
        try {
            val blob = try {
                encryptText(plaintext.toByteArray(Charsets.UTF_8), key)
            } catch (_: UnsatisfiedLinkError) {
                ClipboardRepository.localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
            }
            val lamportTs = System.currentTimeMillis()
            syncManager.uploadItem(blob.ciphertext, blob.nonce, "text/plain", lamportTs)
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

    private fun buildNotification(): Notification {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID, "CopyPaste Service",
                NotificationManager.IMPORTANCE_LOW
            )
            getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        }

        return android.app.Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("CopyPaste")
            .setContentText("Monitoring clipboard...")
            .setSmallIcon(android.R.drawable.ic_menu_clipboard)
            .build()
    }

    companion object {
        private const val TAG = "ClipboardService"
        private const val NOTIFICATION_ID = 1001
        private const val CHANNEL_ID = "copypaste_service"
    }
}
